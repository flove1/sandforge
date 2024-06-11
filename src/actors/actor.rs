use std::mem;

use bevy::{ prelude::*, render::view::RenderLayers };
use bevy_math::{ ivec2, vec2 };
use bevy_rapier2d::{
    dynamics::{
        Damping,
        ExternalImpulse,
        GravityScale,
        LockedAxes,
        ReadMassProperties,
        RigidBody,
        Velocity,
    },
    geometry::{ Collider, ColliderMassProperties, CollisionGroups, Group, Sensor },
};
use itertools::Itertools;

use crate::{
    camera::ACTOR_RENDER_LAYER,
    constants::{ CHUNK_SIZE, PARTICLE_Z },
    simulation::{
        chunk_groups::build_chunk_group,
        chunk_manager::ChunkManager,
        colliders::{ ACTOR_MASK, HITBOX_MASK, OBJECT_MASK },
        dirty_rect::DirtyRects,
        materials::{ ContactEffect, PhysicsType },
        particle::{ Particle, ParticleBundle },
        pixel::Pixel,
    },
};

use bitflags::bitflags;

use super::health::{ DamageEvent, Health };

#[derive(Bundle, Clone)]
pub struct ActorBundle {
    pub actor: Actor,
    pub rb: RigidBody,
    pub velocity: Velocity,
    pub axes: LockedAxes,
    pub read_mass: ReadMassProperties,
    pub mass_properties: ColliderMassProperties,
    pub sprite: SpriteSheetBundle,
    pub health: Health,
    pub render_layers: RenderLayers,
    pub collision_groups: CollisionGroups,
    pub collider: Collider,
    pub storred_rotation: StorredRotation,
    pub damping: Damping,
    pub impulse: ExternalImpulse,
    pub gravity: GravityScale,
}

#[derive(Bundle, Clone)]
pub struct ActorHitboxBundle {
    pub collision_groups: CollisionGroups,
    pub collider: Collider,
    pub transform: TransformBundle,
    pub sensor: Sensor,
}

#[derive(Component)]
pub struct AttackParameters {
    pub value: f32,
    pub knockback_strength: f32,
}

impl Default for ActorBundle {
    fn default() -> Self {
        Self {
            axes: LockedAxes::ROTATION_LOCKED,
            actor: Actor::default(),
            rb: RigidBody::Dynamic,
            velocity: Velocity::zero(),
            read_mass: ReadMassProperties::default(),
            sprite: SpriteSheetBundle::default(),
            mass_properties: ColliderMassProperties::default(),
            render_layers: RenderLayers::layer(ACTOR_RENDER_LAYER),
            collision_groups: CollisionGroups::new(
                Group::from_bits_truncate(ACTOR_MASK),
                Group::from_bits_truncate(OBJECT_MASK)
            ),
            collider: Collider::default(),
            health: Health {
                current: 16.0,
                total: 16.0,
            },
            storred_rotation: StorredRotation::default(),
            damping: Damping::default(),
            impulse: ExternalImpulse::default(),
            gravity: GravityScale(3.0),
        }
    }
}

impl Default for ActorHitboxBundle {
    fn default() -> Self {
        Self {
            collision_groups: CollisionGroups::new(
                Group::from_bits_truncate(HITBOX_MASK),
                Group::from_bits_truncate(OBJECT_MASK)
            ),
            collider: Collider::default(),
            transform: TransformBundle::default(),
            sensor: Sensor,
        }
    }
}

bitflags! {
    #[derive(Default, Clone)]
    pub struct ActorFlags: u32 {
        const GROUNDED = 1 << 0;
        const SUBMERGED = 1 << 1;
        const INFLUENCED = 1 << 2;
    }
}

#[derive(Default, Reflect, Component, Clone)]
pub struct Actor {
    pub size: Vec2,
    pub position: Vec2,
    pub movement_type: MovementType,
    #[reflect(ignore)]
    pub flags: ActorFlags,
}

// since rapier automatically manages transforms it is required to manually store it
#[derive(Default, Clone, Component, Deref, DerefMut)]
pub struct StorredRotation(pub Quat);

#[derive(Default, Reflect, Clone)]
pub enum MovementType {
    #[default]
    Floating,
    Walking {
        speed: f32,
        jump_height: f32,
    },
}

pub fn update_actor_translation(mut actor_q: Query<(&mut Transform, &Actor)>) {
    for (mut transform, actor) in actor_q.iter_mut() {
        transform.translation.x = ((actor.position + actor.size / 2.0) / (CHUNK_SIZE as f32)).x;
        transform.translation.y = ((actor.position + actor.size / 2.0) / (CHUNK_SIZE as f32)).y;
    }
}

/// based on this [article](http://higherorderfun.com/blog/2012/05/20/the-guide-to-implementing-2d-platformers/)
pub fn update_actors(
    mut commands: Commands,
    mut actor_q: Query<(Entity, &mut Actor, &mut Velocity, &mut Health, &mut ExternalImpulse)>,
    mut dirty_rects: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut damage_ev: EventWriter<DamageEvent>,
    time: Res<Time>
) {
    let mut spawn_particle = |pixel: Pixel, position: Vec2, transferred_velocity: Vec2| {
        commands.spawn(ParticleBundle {
            sprite: SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba_u8(
                        pixel.color[0],
                        pixel.color[1],
                        pixel.color[2],
                        pixel.color[3]
                    ),
                    custom_size: Some(Vec2::ONE / (CHUNK_SIZE as f32)),
                    ..Default::default()
                },
                transform: Transform::from_translation(
                    (position / (CHUNK_SIZE as f32)).extend(PARTICLE_Z)
                ),
                ..Default::default()
            },
            velocity: Velocity::linear(
                (vec2(fastrand::f32() - 0.5, fastrand::f32() / 2.0 + 0.5) + transferred_velocity) /
                    (CHUNK_SIZE as f32)
            ),
            particle: Particle::new(pixel),
            ..Default::default()
        });

        dirty_rects.request_render(position.as_ivec2());
    };

    for (entity, mut actor, mut velocity, mut health, mut impulse) in actor_q.iter_mut() {
        let chunk_position = actor.position
            .round()
            .as_ivec2()
            .div_euclid(IVec2::ONE * CHUNK_SIZE);

        let Some(mut chunk_group) = build_chunk_group(&mut chunk_manager, chunk_position) else {
            continue;
        };

        let width = actor.size.x as u16;
        let height = actor.size.y as u16;

        let delta = time.delta_seconds() * 60.0;

        let mut in_liquid = false;
        if
            (0..width as i32)
                .cartesian_product(0..height as i32)
                .map(
                    |(x, y)|
                        actor.position.round().as_ivec2() +
                        ivec2(x, y) -
                        chunk_position * CHUNK_SIZE
                )
                .filter(|position| {
                    let Some(pixel) = chunk_group.get_mut(*position) else {
                        return false;
                    };

                    if let Some(ContactEffect::Heal(value)) = pixel.material.contact {
                        if health.total > health.current {
                            health.current = (health.current + value).min(health.total);
                            *pixel = Pixel::default();
                        }
                    }

                    if let Some(ContactEffect::Damage(value)) = pixel.material.contact {
                        if health.current > 0.0 {
                            damage_ev.send(DamageEvent {
                                value,
                                target: entity,
                                knockback: Vec2::ZERO,
                                ignore_iframes: true,
                                play_sound: false,
                            });
                            *pixel = Pixel::default();
                        }
                    }

                    if matches!(pixel.physics_type, PhysicsType::Liquid(..)) {
                        in_liquid = true;
                    }

                    matches!(pixel.physics_type, PhysicsType::Powder | PhysicsType::Static)
                })
                .map(|_| 1.0 / ((width * height) as f32))
                .sum::<f32>() > 0.9
        {
            actor.flags.insert(ActorFlags::SUBMERGED);
        } else {
            actor.flags.remove(ActorFlags::SUBMERGED);
        }

        if actor.flags.contains(ActorFlags::SUBMERGED) {
            damage_ev.send(DamageEvent {
                value: 1.0,
                target: entity,
                knockback: Vec2::ZERO,
                ignore_iframes: false,
                play_sound: true,
            });
        }

        if in_liquid {
            let change = velocity.linvel / (CHUNK_SIZE as f32) / 16.0;

            impulse.impulse.x -= change.x * 4.0;
            impulse.impulse.y -= change.y;
        }

        {
            let direction = velocity.linvel.x.signum() as i32;
            let initial_position = actor.position.round().as_ivec2();
            let velocity_offset = ivec2(if direction.is_positive() { width as i32 } else { -1 }, 0);

            let mut last_elevation = 0;

            if
                let Some(x) = (0..(velocity.linvel.x * delta).abs().ceil() as i32).find(|x| {
                    // check whether there is pixels in the way by goind in two directions simotenously from center
                    let collision = (0..=(height / 2) as i32).rev().find_map(|y| {
                        let offset_bottom = ivec2(x * direction, y + last_elevation);
                        let offset_top = ivec2(
                            x * direction,
                            (height as i32) - y + 1 + last_elevation
                        );

                        let collided_on_bottom = chunk_group
                            .get_mut(
                                initial_position +
                                    velocity_offset +
                                    offset_bottom -
                                    chunk_position * CHUNK_SIZE
                            )
                            .map_or(false, |pixel| {
                                match pixel.physics_type {
                                    PhysicsType::Static => true,
                                    PhysicsType::Powder => {
                                        if
                                            (velocity.linvel.x.abs() + velocity.linvel.y.abs() <
                                                1.5 && y <= 3) ||
                                            actor.flags.contains(ActorFlags::SUBMERGED)
                                        {
                                            return true;
                                        }

                                        spawn_particle(
                                            mem::take(pixel),
                                            (
                                                initial_position +
                                                velocity_offset +
                                                offset_bottom
                                            ).as_vec2(),
                                            velocity.linvel * 0.75
                                        );

                                        false
                                    }
                                    _ => false,
                                }
                            });

                        let collided_on_top = chunk_group
                            .get_mut(
                                initial_position +
                                    velocity_offset +
                                    offset_top -
                                    chunk_position * CHUNK_SIZE
                            )
                            .map_or(false, |pixel| {
                                match pixel.physics_type {
                                    PhysicsType::Static => true,
                                    PhysicsType::Powder => {
                                        if
                                            (velocity.linvel.x.abs() + velocity.linvel.y.abs() <
                                                1.5 && y < 3) ||
                                            actor.flags.contains(ActorFlags::SUBMERGED)
                                        {
                                            return true;
                                        }

                                        spawn_particle(
                                            mem::take(pixel),
                                            (
                                                initial_position +
                                                velocity_offset +
                                                offset_bottom
                                            ).as_vec2(),
                                            velocity.linvel * 0.75
                                        );

                                        // velocity.linvel.x *= 0.97;
                                        false
                                    }
                                    _ => false,
                                }
                            });

                        if collided_on_bottom {
                            Some(y)
                        } else if collided_on_top {
                            Some(-y)
                        } else {
                            None
                        }
                    });

                    if let Some(dy) = collision {
                        // try to fit in slopes
                        if
                            dy > 3 ||
                            dy < -2 ||
                            (0..width as i32)
                                .find(|body_x|
                                    chunk_group
                                        .get(
                                            initial_position +
                                                ivec2(
                                                    x * direction + body_x,
                                                    dy + last_elevation + (height as i32)
                                                ) -
                                                chunk_position * CHUNK_SIZE
                                        )
                                        .map_or(false, |pixel|
                                            matches!(
                                                pixel.physics_type,
                                                PhysicsType::Static | PhysicsType::Powder
                                            )
                                        )
                                )
                                .is_some()
                        {
                            // encountered non-climbable obstacle
                            return true;
                        }

                        if velocity.linvel.x.abs() > 0.0 {
                            if dy.abs() > 1 {
                                velocity.linvel.x =
                                    velocity.linvel.x.signum() *
                                    (velocity.linvel.x.abs() - (dy.abs() as f32) * 0.25).max(0.1);
                            }

                            last_elevation += dy;
                        }
                    }

                    false
                })
            {
                actor.position.x = (initial_position.x + (x - 1).max(0) * direction) as f32;
                actor.position.y += last_elevation as f32;
                velocity.linvel.x *= 0.1;
            } else {
                actor.position.x += velocity.linvel.x * delta;
                actor.position.y += last_elevation as f32;
            }
        }

        {
            let direction = velocity.linvel.y.signum() as i32;
            let initial_position = actor.position.round().as_ivec2();
            let velocity_offset = ivec2(0, if direction.is_positive() {
                height as i32
            } else {
                -1
            });

            if
                let Some(y) = (0..(velocity.linvel.y * delta).abs().ceil() as i32).find(|y| {
                    (0..width as i32).any(|x| {
                        chunk_group
                            .get_mut(
                                initial_position +
                                    velocity_offset +
                                    ivec2(x, y * direction) -
                                    chunk_position * CHUNK_SIZE
                            )
                            .map_or(false, |pixel| {
                                match pixel.physics_type {
                                    PhysicsType::Static | PhysicsType::Rigidbody { .. } => true,
                                    PhysicsType::Powder => {
                                        if
                                            (velocity.linvel.x.abs() + velocity.linvel.y.abs() <
                                                1.5 &&
                                                matches!(
                                                    actor.movement_type,
                                                    MovementType::Walking { .. }
                                                )) ||
                                            actor.flags.contains(ActorFlags::SUBMERGED)
                                        {
                                            return true;
                                        }

                                        spawn_particle(
                                            mem::take(pixel),
                                            (
                                                initial_position +
                                                velocity_offset +
                                                ivec2(x, y * direction)
                                            ).as_vec2(),
                                            velocity.linvel * 0.75
                                        );

                                        velocity.linvel.y *= 0.95;
                                        false
                                    }
                                    _ => false,
                                }
                            })
                    })
                })
            {
                actor.position.y = (initial_position.y + (y - 1).max(0) * direction) as f32;
                // actor.position = (start_position + ivec2(0, )).as_vec2();
                velocity.linvel.y *= 0.25;
            } else {
                actor.position.y += velocity.linvel.y * delta;
            }
        }

        let position = actor.position.round().as_ivec2();
        if
            (-2..=-1)
                .rev()
                .cartesian_product(-1..(width as i32) + 1)
                .any(|(y, x)| {
                    chunk_group
                        .get(position + ivec2(x, y) - chunk_position * CHUNK_SIZE)
                        .map_or(false, |pixel|
                            matches!(
                                pixel.physics_type,
                                PhysicsType::Rigidbody { .. } | PhysicsType::Static | PhysicsType::Powder
                            )
                        )
                })
        {
            actor.flags.insert(ActorFlags::GROUNDED);
        } else {
            actor.flags.remove(ActorFlags::GROUNDED);
        }

        match actor.movement_type {
            MovementType::Floating => {
                velocity.linvel *= 0.95;
            }
            MovementType::Walking { .. } => {
                if !actor.flags.contains(ActorFlags::INFLUENCED) {
                    velocity.linvel.x *= 0.85;
                } else {
                    velocity.linvel.x *= 0.975;
                }
            }
        }

        if actor.flags.contains(ActorFlags::GROUNDED) {
            actor.flags.remove(ActorFlags::INFLUENCED);
        }
    }
}

#[derive(Resource, Default, PartialEq)]
pub struct ActorDebugRender(pub bool);

pub fn toggle_actors(
    mut ctx: ResMut<ActorDebugRender>,
    keys: Res<ButtonInput<KeyCode>>
) {
    if keys.just_pressed(KeyCode::F3) {
        ctx.0 = !ctx.0;
    }
}

pub fn render_actor_gizmos(mut gizmos: Gizmos, actors: Query<&Actor>) {
    for actor in actors.iter() {
        gizmos.rect_2d(
            (actor.position + actor.size / 2.0) / (CHUNK_SIZE as f32),
            0.0,
            actor.size / (CHUNK_SIZE as f32),
            Color::Rgba {
                red: 0.0,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0,
            }
        );
    }
}
