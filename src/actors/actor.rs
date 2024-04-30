use bevy::{ prelude::*, sprite::{ MaterialMesh2dBundle, Mesh2dHandle } };
use bevy_math::{ ivec2, vec2 };
use bevy_rapier2d::{
    dynamics::Velocity,
    geometry::Collider,
    na::ComplexField,
    pipeline::QueryFilter,
    plugin::RapierContext,
    rapier::geometry::ColliderBuilder,
};
use itertools::Itertools;

use crate::{
    constants::CHUNK_SIZE,
    simulation::{
        chunk_groups::{ build_chunk_group, build_chunk_group_mut },
        chunk_manager::ChunkManager,
        materials::{ Material, PhysicsType },
        particle::{ Particle, ParticleInstances },
        pixel::Pixel,
    },
};

#[derive(Component, Clone)]
pub struct Actor {
    pub hitbox: Rect,
    pub position: Vec2,
    pub velocity: Vec2,
    pub on_ground: bool,
    pub movement_type: MovementType,
}

#[derive(Clone, PartialEq, Eq)]
pub enum MovementType {
    Walking,
    Floating,
    Bouncing,
}

// #[derive(Clone, PartialEq, Eq)]
// pub enum StuckState {
//     Free,
//     Partially,
//     Full,
// }

const DEFAULT_GRAVITY: f32 = -0.98;

/// based on this [article](http://higherorderfun.com/blog/2012/05/20/the-guide-to-implementing-2d-platformers/)
pub fn update_actors(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut actors: Query<&mut Actor>,
    particles: Query<(Entity, &Mesh2dHandle), With<ParticleInstances>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    rapier_context: Res<RapierContext>,
    time: Res<Time>,
    object_q: Query<&Velocity>
) {
    let (particles, particle_mesh) = particles.get_single().unwrap();

    for mut actor in actors.iter_mut() {
        let chunk_position = actor.position
            .round()
            .as_ivec2()
            .div_euclid(IVec2::ONE * CHUNK_SIZE);

        let Some(mut chunk_group) = build_chunk_group_mut(
            &mut chunk_manager,
            chunk_position,
            true
        ) else {
            continue;
        };

        let width = actor.hitbox.width().round() as u16;
        let height = actor.hitbox.height().round() as u16;

        // let area = (0..=width)
        //     .cartesian_product(0..height)
        //     .map(|(x, y)| vec2(x as f32, y as f32))
        //     .collect_vec();

        match actor.movement_type {
            MovementType::Walking => {
                actor.velocity.x *= 0.8;
                actor.velocity.y -= 0.98 * time.delta_seconds() * 4.0;
            }
            MovementType::Floating => {
                actor.velocity *= 0.95;
            }
            MovementType::Bouncing => todo!(),
        }

        // for (x, y) in (0..width as i32).cartesian_product(0..height as i32) {
        //     let point = ivec2(x, y);
        //     let pixel = chunk_group.get(
        //         actor.position.round().as_ivec2() + point - chunk_position * CHUNK_SIZE
        //     );

        //     if
        //         pixel.map_or(false, |pixel|
        //             matches!(
        //                 pixel.material.physics_type,
        //                 PhysicsType::Powder | PhysicsType::Liquid(..)
        //             )
        //         )
        //     {
        //         if actor.velocity.abs().x + actor.velocity.abs().y > 2.0 {
        //             let particle = Particle::new(
        //                 pixel.unwrap().material.clone(),
        //                 actor.position.round() + point.as_vec2(),
        //                 Vec2::new(
        //                     (-actor.velocity.x * fastrand::f32()) / 4.0,
        //                     fastrand::f32() * 2.0
        //                 )
        //             );

        //             let mesh = MaterialMesh2dBundle {
        //                 mesh: particle_mesh.clone(),
        //                 material: materials.add(
        //                     Color::rgba_u8(
        //                         particle.material.color[0],
        //                         particle.material.color[1],
        //                         particle.material.color[2],
        //                         particle.material.color[3]
        //                     )
        //                 ),
        //                 transform: Transform::from_translation(
        //                     (particle.pos / (CHUNK_SIZE as f32)).extend(-1.0)
        //                 ),
        //                 ..Default::default()
        //             };

        //             let particle_handle = commands.spawn((particle, mesh)).id();

        //             commands.entity(particles).add_child(particle_handle);

        //             actor.velocity.x *= 0.95;
        //             actor.velocity.y *= 0.95;
        //         }
        //     }
        // }

        if actor.velocity.x.abs() > 0.0 {
            let direction = actor.velocity.x.signum() as i32;
            let start = if direction.is_positive() { width as i32 } else { -1 };
            let start_position = actor.position.round().as_ivec2();

            let mut last_elevation = 0;

            if
                let Some(x) = (0..actor.velocity.x.abs().round() as i32).find(|x| {
                    // check whether there is pixels in the way by goind in two directions simotenously from center
                    let collision = (0..(height / 2) as i32).rev().find_map(|y| {
                        let collided_on_bottom = chunk_group
                            .get_mut(
                                start_position +
                                    ivec2(x * direction + start, y + last_elevation) -
                                    chunk_position * CHUNK_SIZE
                            )
                            .map_or(false, |pixel| {
                                match pixel.material.physics_type {
                                    PhysicsType::Static | PhysicsType::Rigidbody => true,
                                    PhysicsType::Powder => {
                                        if (actor.velocity.y - DEFAULT_GRAVITY).abs() < 0.1 && y < 3 {
                                            return true;
                                        }

                                        let particle = Particle::new(
                                            std::mem::take(pixel).material.clone(),
                                            (
                                                start_position +
                                                ivec2(x * direction + start, y + last_elevation)
                                            ).as_vec2(),
                                            Vec2::new(
                                                fastrand::f32() -
                                                    0.5 +
                                                    2.0 * actor.velocity.x.signum(),
                                                fastrand::f32() / 2.0 + 0.5
                                            )
                                        );

                                        let mesh = MaterialMesh2dBundle {
                                            mesh: particle_mesh.clone(),
                                            material: materials.add(
                                                Color::rgba_u8(
                                                    particle.material.color[0],
                                                    particle.material.color[1],
                                                    particle.material.color[2],
                                                    particle.material.color[3]
                                                )
                                            ),
                                            transform: Transform::from_translation(
                                                (particle.pos / (CHUNK_SIZE as f32)).extend(-1.0)
                                            ),
                                            ..Default::default()
                                        };

                                        commands.entity(particles).with_children(|parent| {
                                            parent.spawn((particle, mesh));
                                        });

                                        actor.velocity *= 0.975;
                                        false
                                    }
                                    _ => false,
                                }
                            });

                        let collided_on_top = chunk_group
                            .get_mut(
                                start_position +
                                    ivec2(
                                        x * direction + start,
                                        (height as i32) - y + 1 + last_elevation
                                    ) -
                                    chunk_position * CHUNK_SIZE
                            )
                            .map_or(false, |pixel| {
                                match pixel.material.physics_type {
                                    PhysicsType::Static | PhysicsType::Rigidbody => true,
                                    PhysicsType::Powder => {
                                        // if (actor.velocity.y - DEFAULT_GRAVITY).abs() < 0.1 && y < 2 {
                                        //     return true;
                                        // }

                                        let particle = Particle::new(
                                            std::mem::take(pixel).material.clone(),
                                            (
                                                start_position +
                                                (height as i32) -
                                                y +
                                                1 +
                                                last_elevation
                                            ).as_vec2(),
                                            Vec2::new(
                                                fastrand::f32() -
                                                    0.5 +
                                                    2.0 * actor.velocity.x.signum(),
                                                fastrand::f32() / 2.0 + 0.5
                                            )
                                        );

                                        let mesh = MaterialMesh2dBundle {
                                            mesh: particle_mesh.clone(),
                                            material: materials.add(
                                                Color::rgba_u8(
                                                    particle.material.color[0],
                                                    particle.material.color[1],
                                                    particle.material.color[2],
                                                    particle.material.color[3]
                                                )
                                            ),
                                            transform: Transform::from_translation(
                                                (particle.pos / (CHUNK_SIZE as f32)).extend(-1.0)
                                            ),
                                            ..Default::default()
                                        };

                                        commands.entity(particles).with_children(|parent| {
                                            parent.spawn((particle, mesh));
                                        });

                                        actor.velocity *= 0.975;
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
                        let out_of_allowed_range = dy < -2 || dy > 3;

                        if
                            out_of_allowed_range ||
                            (0..width as i32)
                                .find(|body_x|
                                    chunk_group
                                        .get(
                                            start_position +
                                                ivec2(
                                                    x * direction + body_x,
                                                    dy + last_elevation + (height as i32)
                                                ) -
                                                chunk_position * CHUNK_SIZE
                                        )
                                        .map_or(false, |pixel|
                                            matches!(
                                                pixel.material.physics_type,
                                                PhysicsType::Rigidbody | PhysicsType::Static
                                            )
                                        )
                                )
                                .is_some()
                        {
                            // encountered non-climbable obstacle
                            return true;
                        }

                        actor.velocity.x /= (dy.abs() * 2).max(1) as f32;
                        last_elevation += dy;
                    } else if actor.velocity.y.abs() < 0.5 {
                        // try to snap to ground
                        let close_to_ground = (1..=4).find(|dy| {
                            (0..width as i32).any(|body_x| {
                                chunk_group
                                    .get(
                                        start_position +
                                            ivec2(x * direction + body_x, last_elevation - dy) -
                                            chunk_position * CHUNK_SIZE
                                    )
                                    .map_or(false, |pixel|
                                        matches!(
                                            pixel.material.physics_type,
                                            PhysicsType::Rigidbody |
                                                PhysicsType::Static |
                                                PhysicsType::Powder
                                        )
                                    )
                            })
                        });

                        if let Some(dy) = close_to_ground {
                            last_elevation -= dy - 1;
                        }
                    }

                    false
                })
            {
                actor.position = (
                    start_position + ivec2((x - 1).max(0) * direction, last_elevation)
                ).as_vec2();
                actor.velocity.x *= 0.25;
            } else {
                actor.position.x += actor.velocity.x;
                actor.position.y += last_elevation as f32;
            }
        }

        if actor.velocity.y.abs() > 0.0 {
            let direction = actor.velocity.y.signum() as i32;
            let start = if direction.is_positive() { height as i32 - 1 } else { 0 };
            let start_position = actor.position.round().as_ivec2();

            if
                let Some(y) = (0..actor.velocity.y.abs().ceil() as i32).find(|y| {
                    (0..width as i32).any(|x| {
                        chunk_group
                            .get_mut(
                                start_position +
                                    ivec2(x, y * direction + start) -
                                    chunk_position * CHUNK_SIZE
                            )
                            .map_or(false, |pixel| {
                                match pixel.material.physics_type {
                                    PhysicsType::Static | PhysicsType::Rigidbody => true,
                                    PhysicsType::Powder => {
                                        if (actor.velocity.y - DEFAULT_GRAVITY).abs() < 1.0 && matches!(actor.movement_type, MovementType::Walking) {
                                            return true;
                                        }

                                        let particle = Particle::new(
                                            std::mem::take(pixel).material.clone(),
                                            (
                                                start_position + ivec2(x, y * direction + start)
                                            ).as_vec2(),
                                            Vec2::new(
                                                fastrand::f32() -
                                                    0.5 +
                                                    2.0 * actor.velocity.x.signum(),
                                                2.0 * actor.velocity.y.signum()
                                            )
                                        );

                                        let mesh = MaterialMesh2dBundle {
                                            mesh: particle_mesh.clone(),
                                            material: materials.add(
                                                Color::rgba_u8(
                                                    particle.material.color[0],
                                                    particle.material.color[1],
                                                    particle.material.color[2],
                                                    particle.material.color[3]
                                                )
                                            ),
                                            transform: Transform::from_translation(
                                                (particle.pos / (CHUNK_SIZE as f32)).extend(-1.0)
                                            ),
                                            ..Default::default()
                                        };

                                        commands.entity(particles).with_children(|parent| {
                                            parent.spawn((particle, mesh));
                                        });

                                        // actor.velocity.y *= 0.9;
                                        false
                                    }
                                    _ => false,
                                }
                            })
                    })
                })
            {
                actor.position = (start_position + ivec2(0, (y - 1).max(0) * direction)).as_vec2();
                actor.velocity.y *= 0.25;
            } else {
                actor.position.y += actor.velocity.y;
            }
        }

        if actor.movement_type == MovementType::Walking {
            let position = actor.position.floor().as_ivec2();
            if
                (0..width as i32)
                    .any(|x| {
                        chunk_group
                            .get(position + ivec2(x, -1) - chunk_position * CHUNK_SIZE)
                            .map_or(false, |pixel|
                                matches!(
                                    pixel.material.physics_type,
                                    PhysicsType::Rigidbody | PhysicsType::Static | PhysicsType::Powder
                                )
                            )
                    })
            {
                if actor.velocity.y.is_sign_negative() {
                    actor.velocity.y = -0.98;
                }
                actor.on_ground = true;
            } else {
                actor.on_ground = false;
            }
        }


        if actor.movement_type == MovementType::Walking {
            let intersection = rapier_context.intersection_with_shape(
                actor.position / (CHUNK_SIZE as f32) +
                    vec2((width as f32) / 4.0 / (CHUNK_SIZE as f32), 0.0),
                0.0,
                &Collider::cuboid(
                    (width as f32) / 4.0 / (CHUNK_SIZE as f32),
                    (1 as f32) / 2.0 / (CHUNK_SIZE as f32)
                ),
                QueryFilter::only_dynamic()
            );

            if let Some(object_entity) = intersection {
                let object_velocitry = object_q.get(object_entity).unwrap();

                actor.velocity += object_velocitry.linvel * 0.8;
            }
        }

        // let steps = (((actor.velocity.x.abs() + actor.velocity.y.abs()) as u32) + 1).max(3);
        // for _ in 0..steps {
        //     let movement = velocity / (steps as f32);

        //     for point in area.iter() {
        //         let pixel = chunk_group
        //             .get(
        //                 position +
        //                     ivec2()(vec2(new_pos_x, actor.position.y) + *point).as_ivec2() -
        //                     chunk_position * CHUNK_SIZE
        //             )
        //             .cloned()
        //             .unwrap_or(Pixel::default());
        //     }

        //     new_pos_x += velocity.x / (steps as f32);
        //     new_pos_y += velocity.y / (steps as f32);

        //     let mut collided_x = false;
        //     for point in area.iter() {
        //         // get pixel at position

        //         // if not empty then check
        //         if !matches!(
        //             pixel.material.physics_type,
        //             PhysicsType::Air | PhysicsType::Liquid(..)
        //         ) {
        //             // check if needs to go up
        //             let clip_round = (point.y - actor.hitbox.min.y < 3.0).then(|| {
        //                 ((actor.position.y + point.y).round() + 1.0)
        //                     - (actor.position.y + actor.hitbox.min.y)
        //             });

        //             // check if needs to go down
        //             let clip_floor = (actor.hitbox.max.y - point.y < 3.0).then(|| {
        //                 (actor.position.y + point.y).round()
        //                     - (actor.position.y + actor.hitbox.max.y)
        //             });

        //             if let Some(clip_y) = clip_round.or(clip_floor) {
        //                 let mut would_clip_collide = false;
        //                 for point in area.iter() {
        //                     if !matches!(
        //                         chunk_group
        //                             .get(
        //                                 IVec2::new(
        //                                     (new_pos_x + point.x).round() as i32,
        //                                     (actor.position.y + clip_y + point.y).round() as i32,
        //                                 ) - chunk_position * CHUNK_SIZE
        //                             )
        //                             .unwrap_or(&Pixel::default())
        //                             .material
        //                             .physics_type,
        //                         PhysicsType::Air | PhysicsType::Liquid(..)
        //                     ) {
        //                         would_clip_collide = true;
        //                         break;
        //                     }
        //                 }

        //                 if would_clip_collide {
        //                     collided_x = true;
        //                 } else {
        //                     new_pos_y += clip_y;
        //                     actor.position.y += clip_y;
        //                     actor.velocity.x *= 0.25;
        //                     // *=
        //                     //     (1.0 - (clip_y.abs() / 3.0).powi(4)).clamp(0.5, 1.0);
        //                 }
        //             // }
        //             // else if pixel.material.physics_type == PhysicsType::Powder {

        //             //     let temp = chunk_manager
        //             //         .set(
        //             //             IVec2::new(
        //             //                 (new_pos_x + point.x).round() as i32,
        //             //                 (actor.position.y + point.y).round() as i32,
        //             //             ),
        //             //             Material::default().into(),
        //             //         )
        //             //         .is_ok();

        //             //     let particle = Particle::new(
        //             //         pixel.material.clone(),
        //             //         Vec2::new(
        //             //             (new_pos_x + point.x).round(),
        //             //             (actor.position.y + point.y).round(),
        //             //         ),
        //             //         Vec2::new(
        //             //             (fastrand::f32() - 0.5) + 2.0 * actor.velocity.x.signum(),
        //             //             fastrand::f32() / 2.0 + 0.5,
        //             //         ),
        //             //     );

        //             //     let mesh = MaterialMesh2dBundle {
        //             //         mesh: particle_mesh.clone(),
        //             //         material: materials.add(Color::rgba_u8(
        //             //             particle.material.color[0],
        //             //             particle.material.color[1],
        //             //             particle.material.color[2],
        //             //             particle.material.color[3],
        //             //         )),
        //             //         transform: Transform::from_translation(
        //             //             (particle.pos / CHUNK_SIZE as f32).extend(-1.0),
        //             //         ),
        //             //         ..Default::default()
        //             //     };

        //             //     let particle_handle = commands.spawn((particle, mesh)).id();

        //             //     commands.entity(particles).add_child(particle_handle);

        //             //     actor.velocity.x *= 0.99;
        //             } else {
        //                 collided_x = true;
        //             }
        //         }
        //     }

        //     if collided_x {
        //         actor.velocity.x = if actor.velocity.x.abs() > 0.25 {
        //             actor.velocity.x * 0.5
        //         } else {
        //             0.0
        //         };
        //     } else {
        //         actor.position.x = new_pos_x;
        //     }

        //     let mut collided_y = false;
        //     for point in area.iter() {
        //         let pixel = chunk_group
        //             .get(
        //                 IVec2::new(
        //                     (actor.position.x + point.x).round() as i32,
        //                     (new_pos_y + point.y).round() as i32,
        //                 ) - chunk_position * CHUNK_SIZE,
        //             )
        //             .cloned()
        //             .unwrap_or(Pixel::default());

        //         if !matches!(
        //             pixel.material.physics_type,
        //             PhysicsType::Air | PhysicsType::Liquid(..)
        //         ) {
        //             if (actor.velocity.abs().x + actor.velocity.abs().y) > 2.0
        //                 && pixel.material.physics_type == PhysicsType::Powder
        //                 && chunk_group
        //                     .set(
        //                         IVec2::new(
        //                             (actor.position.x + point.x).round() as i32,
        //                             (new_pos_y + point.y).round() as i32,
        //                         ) - chunk_position * CHUNK_SIZE,
        //                         Pixel::new(Material::default().into()),
        //                     )
        //                     .is_ok()
        //             {
        //                 let particle = Particle::new(
        //                     pixel.material.clone(),
        //                     Vec2::new(
        //                         (actor.position.x + point.x).round(),
        //                         (new_pos_y + point.y).round(),
        //                     ),
        //                     Vec2::new(
        //                         -actor.velocity.x * fastrand::f32() / 4.0,
        //                         fastrand::f32() * 2.0,
        //                     ),
        //                 );

        //                 let mesh = MaterialMesh2dBundle {
        //                     mesh: particle_mesh.clone(),
        //                     material: materials.add(Color::rgba_u8(
        //                         particle.material.color[0],
        //                         particle.material.color[1],
        //                         particle.material.color[2],
        //                         particle.material.color[3],
        //                     )),
        //                     transform: Transform::from_translation(
        //                         (particle.pos / CHUNK_SIZE as f32).extend(-1.0),
        //                     ),
        //                     ..Default::default()
        //                 };

        //                 let particle_handle = commands.spawn((particle, mesh)).id();

        //                 commands.entity(particles).add_child(particle_handle);

        //                 if actor.velocity.y < 0.0 {
        //                     actor.velocity.y *= 0.9;
        //                 }

        //                 actor.velocity.x *= 0.95;
        //                 actor.velocity.y *= 0.95;
        //             } else {
        //                 collided_y = true;
        //                 break;
        //             }
        //         }
        //     }

        //     for point in area.iter() {
        //         if matches!(
        //             chunk_group
        //                 .get(
        //                     (vec2(new_pos_x, new_pos_y) + *point).as_ivec2()
        //                         - chunk_position * CHUNK_SIZE
        //                 )
        //                 .map(|pixel| pixel.material.physics_type)
        //                 .unwrap_or(PhysicsType::Air),
        //             PhysicsType::Liquid(..)
        //         ) {
        //             actor.velocity.x *= 0.975;
        //         }
        //     }

        //     if collided_y {
        //         actor.velocity.x *= 0.95;

        //         if actor.velocity.y < 0.0 {
        //             actor.on_ground = true;
        //         }

        //         actor.velocity.y = if actor.velocity.y.abs() > 0.5 {
        //             actor.velocity.y * 0.75
        //         } else {
        //             0.0
        //         };

        //         // if let Some(c) = &mut collision_detect {
        //         //     c.collided = true;
        //         // }
        //     } else {
        //         actor.position.y = new_pos_y;
        //     }
        // }
    }
}

pub fn render_actor_gizmos(mut gizmos: Gizmos, actors: Query<&Actor>) {
    for actor in actors.iter() {
        gizmos.rect_2d(
            (actor.position + actor.hitbox.center()) / (CHUNK_SIZE as f32),
            0.0,
            actor.hitbox.size() / (CHUNK_SIZE as f32),
            Color::Rgba {
                red: 0.0,
                green: 1.0,
                blue: 0.0,
                alpha: 1.0,
            }
        );
    }
}
