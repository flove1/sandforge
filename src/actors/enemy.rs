use crate::{
    animation::AnimationState,
    constants::CHUNK_SIZE,
    raycast::raycast,
    registries::Registries,
    simulation::{
        chunk_manager::ChunkManager,
        object::{ Projectile, Object, ObjectBundle },
        pixel::Pixel,
    },
};
use bevy::prelude::*;
use bevy_rapier2d::{
    dynamics:: Velocity ,
    geometry::{ Collider, ColliderMassProperties, Sensor },
    plugin::RapierContext,
};
use itertools::Itertools;
use seldom_state::prelude::StateMachine;

use super::{
    actor::{ Actor, ActorBundle, ActorFlags, StorredRotation },
    animation::IdleAnimation,
    effects::Death,
    health::DamageEvent,
    pathfinding::Path,
    player::Player,
};

#[derive(Component)]
pub struct Enemy;

#[derive(Default, Component, Clone)]
pub enum EnemyAI {
    #[default]
    Follow,
    Projectiles {
        base_material: String,
        cooldown: Timer,
        projectile: Projectile,
        speed: f32,
        range: f32,
    },
}

#[derive(Component)]
pub struct ScopePoints(pub i32);

#[derive(Bundle)]
pub struct EnemyBundle {
    pub score: ScopePoints,
    pub name: Name,
    pub enemy: Enemy,
    pub actor: ActorBundle,
    pub animation_state: AnimationState,
    pub state_machine: StateMachine,
    pub ai: EnemyAI,
    pub idle: IdleAnimation,
}

impl Default for EnemyBundle {
    fn default() -> Self {
        Self {
            name: Name::new("Enemy"),
            enemy: Enemy,
            actor: ActorBundle::default(),
            animation_state: AnimationState::default(),
            state_machine: StateMachine::default(),
            ai: EnemyAI::Follow,
            idle: IdleAnimation,
            score: ScopePoints(15),
        }
    }
}

pub fn enemy_update(
    mut commands: Commands,
    player_q: Query<(Entity, &Transform), With<Player>>,
    mut enemy_q: Query<
        (Entity, &Actor, &Children, &mut Velocity, &Transform, &mut EnemyAI, Option<&mut Path>),
        (With<Enemy>, Without<Death>)
    >,
    hitbox_q: Query<&Collider, With<Sensor>>,
    time: Res<Time>,
    rapier_context: Res<RapierContext>,
    registries: Res<Registries>,
    chunk_manager: Res<ChunkManager>,
    mut damage_ev: EventWriter<DamageEvent>,
) {
    let (player_entity, player_transform) = player_q.single();
    let player_position = (player_transform.translation.xy() * (CHUNK_SIZE as f32))
        .round()
        .as_ivec2();

    for (entity, actor, children, mut velocity, transform, mut ai, path) in enemy_q.iter_mut() {
        if
            let Some(hitbox_entity) = children
                .iter()
                .find(|child_entity| hitbox_q.contains(**child_entity))
                .cloned()
        {
            for pair in rapier_context.intersection_pairs_with(hitbox_entity) {
                let other = if pair.0 == hitbox_entity { pair.1 } else { pair.0 };

                let Some(parent) = rapier_context.collider_parent(other) else {
                    continue;
                };

                if hitbox_q.contains(other) && parent == player_entity {
                    damage_ev.send(DamageEvent {
                        target: rapier_context.collider_parent(other).unwrap(),
                        value: 4.0,
                        knockback: Vec2::new((transform.rotation.y + 0.5) * 2.0, 0.0) +
                        velocity.linvel / 2.0,
                        ignore_iframes: false,
                        play_sound: true,
                    });
                }
            }
        }

        let enemy_position = (transform.translation.xy() * (CHUNK_SIZE as f32)).round().as_ivec2();

        match ai.as_mut() {
            EnemyAI::Follow => {
                if let Some(mut path) = path {
                    if time.elapsed_seconds_f64() - path.created_at > 5.0 {
                        commands.entity(entity).remove::<Path>();
                    }

                    let mut closest_position = path.nodes[0];
                    while
                        path.nodes.len() > 1 &&
                        (enemy_position - path.nodes[0]).length_squared() < 8
                    {
                        closest_position = path.nodes[1];
                        path.nodes.remove(0);
                    }

                    match actor.movement_type {
                        super::actor::MovementType::Floating => {
                            velocity.linvel +=
                                (closest_position - enemy_position).as_vec2().normalize_or_zero() /
                                    16.0 +
                                (fastrand::f32() - 0.5) / 8.0;
                        }
                        super::actor::MovementType::Walking { speed, jump_height } => {
                            if
                                (
                                    path.nodes[0..(4).min(path.nodes.len() - 1)]
                                        .iter()
                                        .map(|node| *node - closest_position)
                                        .sum::<IVec2>().y as f32
                                ) > actor.size.y / 2.0 &&
                                actor.flags.contains(ActorFlags::GROUNDED) &&
                                velocity.linvel.y.is_sign_negative()
                            {
                                velocity.linvel.y += jump_height;
                            }

                            velocity.linvel.x +=
                                ((closest_position - enemy_position)
                                    .as_vec2()
                                    .normalize_or_zero().x /
                                    16.0) *
                                    speed +
                                (fastrand::f32() - 0.5) / 8.0;
                        }
                    }
                } else {
                    velocity.linvel += Vec2::new(
                        (fastrand::f32() - 0.5) / 4.0,
                        (fastrand::f32() - 0.5) / 8.0
                    );
                }
            }
            EnemyAI::Projectiles { base_material, cooldown, projectile, speed, range } => {
                cooldown.tick(time.delta());

                if cooldown.finished() {
                    if
                        (enemy_position - player_position).length_squared() >
                            (range.powi(2) as i32) ||
                        raycast(enemy_position, player_position, &chunk_manager, |pixel|
                            pixel.is_empty()
                        ).is_some()
                    {
                        continue;
                    }

                    cooldown.reset();

                    let distance = player_position - enemy_position;
                    let direction = (player_position - enemy_position)
                        .as_vec2()
                        .normalize_or_zero();

                    let Some(material) = registries.materials.get(base_material) else {
                        continue;
                    };

                    let size: i32 = 17;
                    let mut pixels = vec![None; size.pow(2) as usize];

                    for (x, y) in (0..size).cartesian_product(0..size) {
                        if
                            (IVec2::new(x, y).as_vec2() - (size as f32) / 2.0).length_squared() >
                            ((size as f32) / 2.0).powi(2)
                        {
                            continue;
                        }

                        pixels[(y * size + x) as usize] = Some(Pixel::from(material));
                    }

                    let Ok(object) = Object::from_pixels(pixels, IVec2::splat(size)) else {
                        continue;
                    };

                    let Ok(collider) = object.create_collider() else {
                        continue;
                    };

                    commands.spawn((
                        projectile.clone().with_source(entity),
                        Sensor,
                        ObjectBundle {
                            object,
                            collider,
                            transform: TransformBundle {
                                local: Transform::from_translation(
                                    transform.translation.xy().extend(0.0)
                                ),
                                ..Default::default()
                            },
                            velocity: Velocity::linear(
                                direction * 1.25 * *speed +
                                    velocity.linvel / 16.0 +
                                    distance.as_vec2() / (CHUNK_SIZE as f32) / 2.0 +
                                    (Vec2::Y * (distance.x.abs() as f32)) / (CHUNK_SIZE as f32) / 2.0
                            ),
                            mass_properties: ColliderMassProperties::Density(16.0),
                            ..Default::default()
                        },
                    ));
                }
            }
        }
    }
}

pub fn update_enemy_rotation(
    mut enemy_q: Query<(&mut Transform, &Velocity, &mut StorredRotation), With<Enemy>>
) {
    for (mut transform, velocity, mut rotation) in enemy_q.iter_mut() {
        if velocity.linvel.x < -0.1 {
            rotation.0 = Quat::from_rotation_y(-(180f32).to_radians());
        } else if velocity.linvel.x > 0.1 {
            rotation.0 = Quat::IDENTITY;
        }

        transform.rotation = rotation.0;
    }
}
