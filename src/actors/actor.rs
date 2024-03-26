use bevy::{prelude::*, sprite::{MaterialMesh2dBundle, Mesh2dHandle}};
use bevy_math::ivec2;

use crate::{constants::{CHUNK_SIZE, PARTICLE_LAYER}, registries::Registries, simulation::{dirty_rect::{update_dirty_rects_3x3, DirtyRects}, materials::{Material, PhysicsType}, particle::{Particle, ParticleInstances}, pixel::Pixel, world::{chunks_update, ChunkManager}}};

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
    actors: Query<&Actor>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects: ResMut<DirtyRects>,
    registries: Res<Registries>
) {
    for actor in actors.iter() {
        let size = actor.hitbox.size().as_ivec2();

        for x in 0..size.x {
            for y in 0..size.y {
                let position = actor.position.round().as_ivec2() + ivec2(x, y);

                if let Ok(pixel) = chunk_manager.get_mut(position) {
                    if pixel.is_empty() {
                        *pixel = Pixel::new(registries.materials.get("actor").unwrap().into(), 0);
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

                if let Ok(pixel) = chunk_manager.get_mut(position) {
                    if pixel.material.id == "actor" {
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

pub fn update_actors(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut actors: Query<&mut Actor>,
    mut gizsmos: Gizmos,
    particles: Query<(Entity, &Mesh2dHandle), With<ParticleInstances>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (particles, particle_mesh) = particles.get_single().unwrap();

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
                let pixel = chunk_manager.get(
                    IVec2::new(
                        (new_pos_x + h_dx).round() as i32,
                        (actor.position.y + h_dy).round() as i32
                    )
                ).cloned().unwrap_or(Pixel::default());

                if pixel.material.physics_type != PhysicsType::Air {
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
                                ).unwrap_or(&Pixel::default()).material.physics_type, PhysicsType::Air
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
                    else if pixel.material.physics_type == PhysicsType::Powder
                        && chunk_manager
                        .set(
                            IVec2::new(
                                (new_pos_x + h_dx).round() as i32,
                                (actor.position.y + h_dy).round() as i32,
                            ),
                            Material::default().into(),
                        )
                        .is_ok() 
                    {
                        let particle = Particle::new(
                            pixel.material.clone(), 
                            Vec2::new(
                                (new_pos_x + h_dx).round(),
                                (actor.position.y + h_dy).round(),
                            ), 
                            Vec2::new(
                                (fastrand::f32() - 0.5) + 2.0 * actor.velocity.x.signum(),
                                fastrand::f32() - 0.5,
                            )
                        );

                        let mesh = MaterialMesh2dBundle {
                            mesh: particle_mesh.clone(),
                            material: materials.add(Color::rgba_u8(
                                particle.material.color[0],
                                particle.material.color[1],
                                particle.material.color[2],
                                particle.material.color[3],
                            )),
                            transform: Transform::from_translation((particle.pos / CHUNK_SIZE as f32).extend(PARTICLE_LAYER)),
                            ..Default::default()
                        };

                        let particle_handle = commands.spawn((
                            particle,
                            mesh
                        )).id();

                        commands.entity(particles).add_child(particle_handle);

                        actor.velocity.x *= 0.99;
                    }
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
                let pixel = chunk_manager.get(
                    IVec2::new(
                        (actor.position.x + h_dx).round() as i32,
                        (new_pos_y + h_dy).round() as i32,
                    )
                ).cloned().unwrap_or(Pixel::default());

                if !matches!(pixel.material.physics_type, PhysicsType::Air) {
                    if (actor.velocity.y > -0.001 || actor.velocity.y < 1.0)
                        && pixel.material.physics_type == PhysicsType::Powder
                        && chunk_manager.set(
                            IVec2::new(
                                (actor.position.x + h_dx).round() as i32,
                                (new_pos_y + h_dy).round() as i32,
                            ),
                            Material::default().into()
                        ).is_ok()
                    {
                        let particle = Particle::new(
                            pixel.material.clone(), 
                            Vec2::new(
                                (actor.position.x + h_dx).round(),
                                (new_pos_y + h_dy).round(),
                            ), 
                            Vec2::new(
                                fastrand::f32() - 0.5,
                                -fastrand::f32(),
                            )
                        );

                        let mesh = MaterialMesh2dBundle {
                            mesh: particle_mesh.clone(),
                            material: materials.add(Color::rgba_u8(
                                particle.material.color[0],
                                particle.material.color[1],
                                particle.material.color[2],
                                particle.material.color[3],
                            )),
                            transform: Transform::from_translation((particle.pos / CHUNK_SIZE as f32).extend(PARTICLE_LAYER)),
                            ..Default::default()
                        };

                        let particle_handle = commands.spawn((
                            particle,
                            mesh
                        )).id();

                        commands.entity(particles).add_child(particle_handle);

                        if actor.velocity.y < 0.0 {
                            actor.velocity.y *= 0.9;
                        }

                        actor.velocity.y *= 0.99;
                    }
                    else {
                        collided_y = true;
                        break;
                    }
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
