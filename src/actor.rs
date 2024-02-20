use std::mem::swap;

use bevy::prelude::*;
use bevy_math::ivec2;
use fastrand::choice;

use crate::{pixel::Pixel, constants::CHUNK_SIZE, dirty_rect::{update_dirty_rects_3x3, DirtyRects}, materials::{PhysicsType, ELEMENTS}, world::{chunks_update, ChunkManager}};

pub const UP_WALK_HEIGHT: usize = 3;
pub const DOWN_WALK_HEIGHT: usize = 4;

#[derive(Component, Clone)]
pub struct Actor {
    pub hitbox: Rect,
    pub position: Vec2,
    pub velocity: Vec2,
    pub on_ground: bool,
}

pub fn fill_actors(
    mut chunk_manager: ResMut<ChunkManager>,
    actors: Query<&Actor>,
    mut dirty_rects: ResMut<DirtyRects>,
) {
    for actor in actors.iter() {
        let size = actor.hitbox.size().as_ivec2();

        for x in 0..size.x {
            for y in 0..size.y as i32 {
                let position = actor.position.round().as_ivec2() + ivec2(x, y);

                if let Some(pixel) = chunk_manager.get_mut(position) {
                    if pixel.is_empty() {
                        *pixel = Pixel::new(&ELEMENTS.get("actor").unwrap(), 0);
                    }
                }
                update_dirty_rects_3x3(
                    &mut dirty_rects.current, 
                    position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                    position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                );
            }
        }
    }
}

pub fn unfill_actors(
    mut chunk_manager: ResMut<ChunkManager>,
    actors: Query<&Actor>,
    mut dirty_rects: ResMut<DirtyRects>,
) {

    for actor in actors.iter() {
        let size = actor.hitbox.size().as_ivec2();

        for x in 0..size.x {
            for y in 0..size.y {
                let position = actor.position.round().as_ivec2() + ivec2(x, y);

                if let Some(pixel) = chunk_manager.get_mut(position) {
                    if pixel.material_id == "actor" {
                        *pixel = Pixel::default();
                    }
                }

                update_dirty_rects_3x3(
                    &mut dirty_rects.current, 
                    position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                    position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                );
            }
        }
    }
}

// pub fn on_ground(chunk_manager: &ChunkManager, actor: &Actor) -> bool {
//     for x in 0..actor.width {
//         let position = actor.position + ivec2(x as i32, -1);

//         if let Some(pixel) = chunk_manager.get(position) {
//             if matches!(pixel.matter_type, PhysicsType::Powder | PhysicsType::Static) {
//                 return true;
//             }
//         } else {
//             return true;
//         }
//     }

//     false
// }

pub fn update_actors(
    mut chunk_manager: ResMut<ChunkManager>,
    mut actors: Query<&mut Actor>,
    mut gizsmos: Gizmos,
) {
    for mut actor in actors.iter_mut() {
        actor.on_ground = false;

        let steps_x =
            ((actor.hitbox.max.x - actor.hitbox.min.x).signum() * (actor.hitbox.max.x - actor.hitbox.min.x).abs().ceil()) as u16;
        let steps_y =
            ((actor.hitbox.max.y - actor.hitbox.min.y).signum() * (actor.hitbox.max.y - actor.hitbox.min.y).abs().ceil()) as u16;

        let r: Vec<(f32, f32)> = (0..=steps_x)
            .flat_map(move |a| (0..=steps_y).map(move |b| (a, b)))
            .map(|(xs, ys)| {
                (
                    (f32::from(xs) / f32::from(steps_x)) * (actor.hitbox.max.x - actor.hitbox.min.x) + actor.hitbox.min.x,
                    (f32::from(ys) / f32::from(steps_y)) * (actor.hitbox.max.y - actor.hitbox.min.y) + actor.hitbox.min.y,
                )
            })
            .collect();

        actor.velocity.y = f32::max(actor.velocity.y - 0.25, -2.0);

        gizsmos.rect_2d((actor.position + actor.hitbox.size() / 2.0) / CHUNK_SIZE as f32, 0., actor.hitbox.size() / CHUNK_SIZE as f32, Color::Rgba { red: 0., green: 1., blue: 0., alpha: 1. });
        
        let dx = actor.velocity.x;
        let dy = actor.velocity.y;

        let mut new_pos_x = actor.position.x;
        let mut new_pos_y = actor.position.y;

        let steps = ((dx.abs() + dy.abs()) as u32 + 1).max(3);
        for _ in 0..steps {
            new_pos_x += dx / steps as f32;
            new_pos_y += dy / steps as f32;

            let mut collided_x = false;
            for &(h_dx, h_dy) in &r {
                if !matches!(
                    chunk_manager.get(
                        IVec2::new(
                            (new_pos_x + h_dx).round() as i32,
                            (actor.position.y + h_dy).round() as i32
                        )
                    ).unwrap_or(&Pixel::default()).matter_type, PhysicsType::Empty
                ) {
                    let clip_ceil = (h_dy - actor.hitbox.min.y < 4.0).then(|| {
                        ((actor.position.y + h_dy).round() + 1.0) - (actor.position.y + actor.hitbox.min.y)
                            + 0.05
                    });

                    let clip_floor = (actor.hitbox.max.y - h_dy < 4.0).then(|| {
                        (actor.position.y + h_dy).round() - (actor.position.y + actor.hitbox.max.y) - 0.05
                    });

                    if let Some(clip_y) = clip_ceil.or(clip_floor) {
                        let mut would_clip_collide = false;
                        for &(h_dx, h_dy) in &r {
                            if !matches!(
                                chunk_manager.get(
                                    IVec2::new(
                                        (new_pos_x + h_dx).round() as i32,
                                        (actor.position.y + clip_y + h_dy).round() as i32,
                                    )
                                ).unwrap_or(&Pixel::default()).matter_type, PhysicsType::Empty
                            ){
                                would_clip_collide = true;
                                break;
                            }
                        }

                        if would_clip_collide {
                            collided_x = true;
                        } else {
                            new_pos_y += clip_y;
                            actor.position.y += clip_y;
                            actor.velocity.x *= (1.0 - (clip_y.abs() / 3.0).powi(4)).clamp(0.5, 1.0);
                        }
                    } 
                    // else if mat.physics == PhysicsType::Sand
                    //     && self
                    //     .chunk_handler
                    //     .set_pixel(
                    //         (new_pos_x + f64::from(h_dx)).floor() as i64,
                    //         (pos.y + f64::from(h_dy)).floor() as i64,
                    //         MaterialInstance::air(),
                    //     )
                    //     .is_ok() 
                    // {
                    //     create_particles.push(Particle::new(
                    //         mat,
                    //         Position {
                    //             x: (new_pos_x + f64::from(h_dx)).floor(),
                    //             y: (pos.y + f64::from(h_dy)).floor().floor(),
                    //         },
                    //         Velocity {
                    //             x: rand::thread_rng().gen_range(-0.5..=0.5) + 2.0 * vel.x.signum(),
                    //             y: rand::thread_rng().gen_range(-0.5..=0.5),
                    //         },
                    //     ));

                    //     vel.x *= 0.99;
                    // }
                    else {
                        collided_x = true;
                    }
                }
            }

            if collided_x {
                actor.velocity.x = if actor.velocity.x.abs() > 0.25 { actor.velocity.x * 0.5 } else { 0.0 };
            } else {
                actor.position.x = new_pos_x;
            }

            let mut collided_y = false;
            for &(h_dx, h_dy) in &r {
                if !matches!(
                    chunk_manager.get(
                        IVec2::new(
                            (actor.position.x + h_dx).round() as i32,
                            (new_pos_y + h_dy).round() as i32,
                        )
                    ).unwrap_or(&Pixel::default()).matter_type, PhysicsType::Empty
                ) {
                    collided_y = true;
                    break;
                }
            }

            if collided_y {
                actor.velocity.x *= 0.96;
                
                if dy < 0.0 {
                    actor.on_ground = true;
                }

                actor.velocity.y = if actor.velocity.y.abs() > 0.5 { actor.velocity.y * 0.75 } else { 0.0 };

                // if let Some(c) = &mut collision_detect {
                //     c.collided = true;
                // }
            } else {
                actor.position.y = new_pos_y;
            }
        }
    }
}

// pub fn abort_stair(
//     chunk_manager: &mut ChunkManager,
//     actor: &mut Actor,
//     starting_y: i32,
//     dir: i32,
// ) {
//     for _ in 0..(starting_y - actor.position.y) {
//         move_y(chunk_manager, actor, dir);
//     }
// }

// pub fn move_x(
//     chunk_manager: &mut ChunkManager,
//     actor: &mut Actor,
//     direction: i32,
// ) -> bool {

//     for y in 0..actor.height as i32 {
//         let offset = if direction > 0 {
//             ivec2(actor.width as i32, y)
//         } else {
//             ivec2(-1, y)
//         };

//         if let Some(pixel) = chunk_manager.get_mut(actor.position + offset) {
//             if matches!(pixel.matter_type, PhysicsType::Powder | PhysicsType::Static) {
//                 actor.velocity = Vec2::ZERO;
//                 return false;
//             }
//         } else {
//             actor.velocity = Vec2::ZERO;
//             return false;
//         }
//     }

//     actor.position.x += direction;

//     true
// }

// pub fn move_y(
//     chunk_manager: &mut ChunkManager,
//     actor: &mut Actor,
//     direction: i32,
// ) -> bool {
//     for x in 0..actor.width as i32 {
//         let offset = if direction > 0 {
//             ivec2(x, actor.height as i32)
//         } else {
//             ivec2(x, -1)
//         };

//         if let Some(pixel) = chunk_manager.get_mut(actor.position + offset) {
//             if matches!(pixel.matter_type, PhysicsType::Powder | PhysicsType::Static) {
//                 actor.velocity = Vec2::ZERO;
//                 return false;
//             }
//         } else {
//             actor.velocity = Vec2::ZERO;
//             return false;
//         }
//     }

//     actor.position.y += direction;

//     true
// }abort_stair(
//     chunk_manager: &mut ChunkManager,
//     actor: &mut Actor,
//     starting_y: i32,
//     dir: i32,
// ) {
//     for _ in 0..(starting_y - actor.position.y) {
//         move_y(chunk_manager, actor, dir);
//     }
// }

// pub fn move_x(
//     chunk_manager: &mut ChunkManager,
//     actor: &mut Actor,
//     direction: i32,
// ) -> bool {

//     for y in 0..actor.height as i32 {
//         let offset = if direction > 0 {
//             ivec2(actor.width as i32, y)
//         } else {
//             ivec2(-1, y)
//         };

//         if let Some(pixel) = chunk_manager.get_mut(actor.position + offset) {
//             if matches!(pixel.matter_type, PhysicsType::Powder | PhysicsType::Static) {
//                 actor.velocity = Vec2::ZERO;
//                 return false;
//             }
//         } else {
//             actor.velocity = Vec2::ZERO;
//             return false;
//         }
//     }

//     actor.position.x += direction;

//     true
// }

// pub fn move_y(
//     chunk_manager: &mut ChunkManager,
//     actor: &mut Actor,
//     direction: i32,
// ) -> bool {
//     for x in 0..actor.width as i32 {
//         let offset = if direction > 0 {
//             ivec2(x, actor.height as i32)
//         } else {
//             ivec2(x, -1)
//         };

//         if let Some(pixel) = chunk_manager.get_mut(actor.position + offset) {
//             if matches!(pixel.matter_type, PhysicsType::Powder | PhysicsType::Static) {
//                 actor.velocity = Vec2::ZERO;
//                 return false;
//             }
//         } else {
//             actor.velocity = Vec2::ZERO;
//             return false;
//         }
//     }

//     actor.position.y += direction;

//     true
// }

pub struct ActorsPlugin;
impl Plugin for ActorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                fill_actors.before(chunks_update),
                unfill_actors.after(chunks_update),
                update_actors.after(unfill_actors),
            ),
        );
    }
}
