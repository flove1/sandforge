use std::mem;

use bevy::{
    ecs::reflect,
    prelude::*,
    render::view::RenderLayers,
    sprite::{ MaterialMesh2dBundle, Mesh2dHandle },
};
use bevy_math::{ ivec2, vec2 };
use bevy_rapier2d::{
    control::{ KinematicCharacterController, MoveShapeOptions },
    dynamics::{
        AdditionalMassProperties,
        LockedAxes,
        MassProperties,
        ReadMassProperties,
        RigidBody,
        Velocity,
    },
    geometry::{ Collider, ColliderMassProperties, CollisionGroups, Group },
    math::Real,
    na::ComplexField,
    pipeline::QueryFilter,
    plugin::RapierContext,
    rapier::geometry::ColliderBuilder,
};
use itertools::Itertools;

use crate::{
    camera::ACTOR_RENDER_LAYER,
    constants::{ CHUNK_SIZE, ENEMY_Z, PARTICLE_Z, PLAYER_Z },
    simulation::{
        chunk_groups::build_chunk_group,
        chunk_manager::ChunkManager,
        colliders::{ ACTOR_MASK, OBJECT_MASK },
        dirty_rect::DirtyRects,
        materials::{ Material, PhysicsType },
        object::{ self, Object },
        particle::{ Particle, ParticleBundle, ParticleParent },
        pixel::Pixel,
    },
};

use bitflags::bitflags;

use super::health::Health;

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
}

#[derive(Bundle, Clone)]
pub struct ActorColliderBundle {
    pub collision_groups: CollisionGroups,
    pub collider: Collider,
    pub transform: TransformBundle,
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
        }
    }
}

impl Default for ActorColliderBundle {
    fn default() -> Self {
        Self {
            collision_groups: CollisionGroups::new(
                Group::from_bits_truncate(ACTOR_MASK),
                Group::from_bits_truncate(OBJECT_MASK)
            ),
            collider: Collider::default(),
            transform: TransformBundle::default(),
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

#[derive(Default, Reflect, Clone, PartialEq, Eq)]
pub enum MovementType {
    #[default]
    Walking,
    Floating,
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
    mut actor_q: Query<(Entity, &mut Actor, &mut Velocity, &ReadMassProperties)>,
    // mut object_q: Query<
    //     (&mut Velocity, &Collider, &ReadMassProperties, &Transform),
    //     (With<RigidBody>, Without<Actor>)
    // >,
    mut dirty_rects: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
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

    for (entity, mut actor, mut velocity, mass) in actor_q.iter_mut() {
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

        let mut n = 0;

        let mut avg_in_x = 0.0;
        let mut avg_in_y = 0.0;
        // for (x, y) in (1..(width as i32) - 1).cartesian_product(1..(height as i32) - 1) {
        //     let point = ivec2(x, y);
        //     let pixel = chunk_group.get(
        //         actor.position.ceil().as_ivec2() + point - chunk_position * CHUNK_SIZE
        //     );

        //     if
        //         pixel.map_or(false, |pixel|
        //             matches!(
        //                 pixel.material.physics_type,
        //                 PhysicsType::Static | PhysicsType::Rigidbody
        //             )
        //         )
        //     {
        //         n += 1;
        //         avg_in_x += (x as f32) - (width as f32) / 2.0;
        //         avg_in_y += (y as f32) - (height as f32) / 2.0;
        //     }
        // }

        // if n > 0 {
        //     actor.position.x +=
        //         f32::from(if avg_in_x == 0.0 { 0.0 } else { -avg_in_x.signum() }) * 0.9;
        //     actor.position.y +=
        //         f32::from(if avg_in_y == 0.0 { 0.0 } else { -avg_in_y.signum() }) * 0.9;
        // }

        if
            (0..width as i32)
                .cartesian_product(0..height as i32)
                .filter_map(|(x, y)| {
                    let point =
                        actor.position.round().as_ivec2() +
                        ivec2(x, y) -
                        chunk_position * CHUNK_SIZE;

                    if
                        let Some(pixel) = chunk_group.get(
                            actor.position.round().as_ivec2() +
                                ivec2(x, y) -
                                chunk_position * CHUNK_SIZE
                        )
                    {
                        if matches!(pixel.physics_type, PhysicsType::Powder) {
                            Some(pixel)
                        } else {
                            None
                        }
                    } else {
                        dbg!(point);
                        None
                    }
                })
                // .filter(|pixel| matches!(pixel.material.physics_type, PhysicsType::Powder))
                .map(|_| 1.0 / ((width * height) as f32))
                .sum::<f32>() > 0.9
        {
            actor.flags.insert(ActorFlags::SUBMERGED);
        } else {
            actor.flags.remove(ActorFlags::SUBMERGED);
        }

        // rapier_context.move_shape(
        //     velocity.linvel * 2.0 / CHUNK_SIZE as f32,
        //     &collider,
        //     (actor.position + actor.hitbox.size() / 2.0) / CHUNK_SIZE as f32,
        //     0.0,
        //     mass.unwrap().mass,
        //     &MoveShapeOptions::default(),
        //     QueryFilter::only_dynamic(),
        //     |collision| {
        //         dbg!("q");
        //         // collision.

        //     }
        // );

        // rapier_context.intersections_with_shape(
        //     (actor.position + actor.hitbox.size() / 2.0 + velocity.linvel * 2.0) /
        //         (CHUNK_SIZE as f32),
        //     0.0,
        //     &collider,
        //     QueryFilter::only_dynamic().exclude_collider(entity),

        //     |object_entity| {
        //         let (mut object_velocity, object_collider, object_mass, object_transform) = object_q
        //             .get_mut(object_entity)
        //             .unwrap();

        //         // rapier_context
        //         //     .contact_pair(entity, object_entity)
        //         //     .unwrap()
        //         //     .find_deepest_contact()
        //         //     .unwrap()
        //         //     .1.impulse();

        //         let sum_mass = object_mass.mass + mass.mass;
        //         let avg_vel =
        //             ((object_velocity.linvel * object_mass.mass) / sum_mass +
        //                 (velocity.linvel * mass.mass) / sum_mass) /
        //             2.0;
        //         object_velocity.linvel = avg_vel;
        //         velocity.linvel = avg_vel;

        //         true
        //     }
        // );

        // rapier_context.move_shape(
        //     velocity.linvel / (CHUNK_SIZE as f32),
        //     collider,
        //     actor.position / (CHUNK_SIZE as f32),
        //     0.0,
        //     1.0,
        //     &(MoveShapeOptions {
        //         up: todo!(),
        //         offset: todo!(),
        //         slide: todo!(),
        //         autostep: todo!(),
        //         max_slope_climb_angle: todo!(),
        //         min_slope_slide_angle: todo!(),
        //         apply_impulse_to_dynamic_bodies: todo!(),
        //         snap_to_ground: todo!(),
        //     }),
        //     QueryFilter::only_dynamic(),
        //     |event| {}
        // );

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

                        if dy.abs() > 1 {
                            velocity.linvel.x =
                                velocity.linvel.x.signum() *
                                (velocity.linvel.x.abs() - (dy.abs() as f32) * 0.25).max(0.1);
                        }
                        last_elevation += dy;
                    }
                    // else if velocity.linvel.y.abs() < 0.5 {
                    //     // try to snap to ground
                    //     let close_to_ground = (1..=4).find(|dy| {
                    //         (0..width as i32).any(|body_x| {
                    //             chunk_group
                    //                 .get(
                    //                     start_position +
                    //                         ivec2(x * direction + body_x, last_elevation - dy) -
                    //                         chunk_position * CHUNK_SIZE
                    //                 )
                    //                 .map_or(false, |pixel|
                    //                     matches!(
                    //                         pixel.material.physics_type,
                    //                         PhysicsType::Rigidbody |
                    //                             PhysicsType::Static |
                    //                             PhysicsType::Powder
                    //                     )
                    //                 )
                    //         })
                    //     });

                    //     if let Some(dy) = close_to_ground {
                    //         last_elevation -= dy - 1;
                    //     }
                    // }

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
                                    PhysicsType::Static | PhysicsType::Rigidbody => true,
                                    PhysicsType::Powder => {
                                        if
                                            (velocity.linvel.x.abs() + velocity.linvel.y.abs() <
                                                1.5 &&
                                                matches!(
                                                    actor.movement_type,
                                                    MovementType::Walking
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

                                        velocity.linvel.y *= 0.99;
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

        if actor.movement_type == MovementType::Walking {
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
                                    PhysicsType::Rigidbody |
                                        PhysicsType::Static |
                                        PhysicsType::Powder
                                )
                            )
                    })
            {
                if velocity.linvel.y.is_sign_negative() {
                    // velocity.linvel.y = -0.98;
                }
                actor.flags.insert(ActorFlags::GROUNDED);
            } else {
                actor.flags.remove(ActorFlags::GROUNDED);
            }
        }

        // if actor.movement_type == MovementType::Walking {
        //     let intersection = rapier_context.intersection_with_shape(
        //         actor.position / (CHUNK_SIZE as f32) +
        //             vec2((width as f32) / 4.0 / (CHUNK_SIZE as f32), 0.0),
        //         0.0,
        //         &Collider::cuboid(
        //             (width as f32) / 4.0 / (CHUNK_SIZE as f32),
        //             (1 as f32) / 2.0 / (CHUNK_SIZE as f32)
        //         ),
        //         QueryFilter::only_dynamic()
        //     );

        //     if let Some(object_entity) = intersection {
        //         let object_velocitry = object_q.get(object_entity).unwrap();

        //         velocity.linvel += object_velocitry.linvel * 0.8;
        //     }
        // }

        match actor.movement_type {
            MovementType::Walking => {
                if !actor.flags.contains(ActorFlags::INFLUENCED) {
                    velocity.linvel.x *= 0.85;
                } else {
                    velocity.linvel.x *= 0.975;
                }
                // velocity.linvel.y -= 0.98 * time.delta_seconds() * 6.0;
            }
            MovementType::Floating => {
                velocity.linvel *= 0.95;
            }
            // MovementType::Bouncing => todo!(),
        }

        if actor.flags.contains(ActorFlags::GROUNDED) {
            actor.flags.remove(ActorFlags::INFLUENCED);
        }
    }
}

pub fn render_actor_gizmos(mut gizmos: Gizmos, actors: Query<&Actor>) {
    for actor in actors.iter() {
        gizmos.rect_2d(
            (actor.position + actor.size) / (CHUNK_SIZE as f32),
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
