use crate::{
    animation::{ Animation, AnimationState },
    assets::SpriteSheets,
    constants::{ CHUNK_SIZE, PLAYER_Z },
    raycast::raycast,
    simulation::chunk_manager::ChunkManager,
};
use benimator::FrameRate;
use bevy::{ ecs::query, prelude::*, window::PrimaryWindow };
use bevy_math::{ vec2, vec3 };
use bevy_rapier2d::{
    dynamics::{ GravityScale, ReadMassProperties, RigidBody, Velocity },
    geometry::{ Collider, CollisionGroups, Group, Sensor },
    pipeline::QueryFilter,
    plugin::RapierContext,
};

use super::{
    actor::{ Actor, ActorBundle, MovementType },
    effects::Death,
    health::{ DamageEvent, Health, HealthBarOverlay },
    pathfinding::Path,
    player::Player,
};

#[derive(Component)]
pub struct Enemy;

pub fn enemy_despawn(mut commands: Commands, enemy_q: Query<Entity, With<Enemy>>) {
    for entity in enemy_q.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

// since rapier automatically manages transforms it is required to manually store it
#[derive(Component, Deref, DerefMut)]
pub struct Flipped(pub bool);

pub fn enemy_update(
    player_q: Query<(Entity, &Transform), With<Player>>,
    mut enemy_q: Query<
        (Entity, &Children, &mut Velocity, &Transform, Option<&mut Path>),
        (With<Enemy>, Without<Death>)
    >,
    mut hitbox_q: Query<&Collider, With<Sensor>>,
    mut damage_ev: EventWriter<DamageEvent>,
    chunk_manager: Res<ChunkManager>,
    rapier_context: Res<RapierContext>
) {
    let (player_entity, player_transform) = player_q.single();
    
    let player_position = (player_transform.translation.xy() * (CHUNK_SIZE as f32))
        .round()
        .as_ivec2();

    for (entity, children, mut velocity, transform, path) in enemy_q.iter_mut() {
        if
            let Some(hitbox_entity) = children
                .iter()
                .find(|child_entity| hitbox_q.contains(**child_entity)).cloned()
        {
            for pair in rapier_context.intersection_pairs_with(hitbox_entity) {
                let other = if pair.0 == hitbox_entity {
                    pair.1
                }
                else {
                    pair.0
                };
                
                if hitbox_q.contains(other) {
                    damage_ev.send(DamageEvent {
                        target: rapier_context.collider_parent(other).unwrap(),
                        value: 0.5,
                        knockback: Vec2::new(
                            (transform.rotation.y + 0.5) * 2.0,
                            0.0
                        ) +
                        velocity.linvel / 2.0,
                    });
                }
            }
        }

        // rapier_context.intersection_pair(collider1, collider2)

        // rapier_context.int

        // if let  = rapier_context.intersection_with_shape(
        //     transform.translation.xy(),
        //     0.0,
        //     &collider,
        //     QueryFilter::only_dynamic().predicate(&|entity| player_q.contains(entity))
        // );

        let enemy_position = (transform.translation.xy() * (CHUNK_SIZE as f32)).round().as_ivec2();

        if let Some(mut path) = path {
            let mut closest_position = path.nodes[0];
            while path.nodes.len() > 1 && (enemy_position - path.nodes[0]).length_squared() < 8 {
                closest_position = path.nodes[1];
                path.nodes.remove(0);
            }

            velocity.linvel +=
                (closest_position - enemy_position).as_vec2().normalize_or_zero() / 16.0 +
                (fastrand::f32() - 0.5) / 8.0;
        } else {
            velocity.linvel += vec2((fastrand::f32() - 0.5) / 4.0, (fastrand::f32() - 0.5) / 8.0);
        }
    }
}

pub fn update_enemy_rotation(
    mut enemy_q: Query<(&Actor, &mut Transform, &Velocity, &mut Flipped), With<Enemy>>
) {
    for (actor, mut transform, velocity, mut flipped) in enemy_q.iter_mut() {
        if velocity.linvel.x < -0.01 {
            flipped.0 = true;
        } else if velocity.linvel.x > 0.01 {
            flipped.0 = false;
        }

        if flipped.0 {
            transform.rotation = Quat::from_rotation_y(-(180f32).to_radians());
        }
    }
}
