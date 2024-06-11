use std::mem;

use async_channel::Sender;
use bevy::{ prelude::*, render::view::RenderLayers, tasks::ComputeTaskPool, utils::HashMap };
use bevy_math::ivec2;
use bevy_rapier2d::dynamics::Velocity;
use serde::{ Deserialize, Serialize };

use crate::{ camera::PARTICLE_RENDER_LAYER, constants::CHUNK_SIZE, helpers::WalkGrid };

use super::{
    chunk_groups::{ build_chunk_group, ChunkGroup },
    chunk_manager::ChunkManager,
    dirty_rect::{
        update_dirty_rects,
        update_dirty_rects_3x3,
        DirtyRects,
        RenderMessage,
        UpdateMessage,
    },
    materials::PhysicsType,
    pixel::Pixel,
};

#[derive(Bundle)]
pub struct ParticleBundle {
    pub sprite: SpriteBundle,
    pub velocity: Velocity,
    pub movement: ParticleMovement,
    pub state: ParticleObjectState,
    pub particle: Particle,
    pub render_layers: RenderLayers,
}

impl Default for ParticleBundle {
    fn default() -> Self {
        Self {
            sprite: SpriteBundle::default(),
            velocity: Velocity::default(),
            movement: ParticleMovement::Fall,
            state: ParticleObjectState::FirstFrame,
            particle: Particle::default(),
            render_layers: RenderLayers::layer(PARTICLE_RENDER_LAYER),
        }
    }
}

#[derive(Component, Default)]
pub struct ParticleParent;

#[derive(Component, Reflect, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum ParticleMovement {
    Fall,
    Follow(Entity),
}

#[derive(Component, Reflect, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum ParticleObjectState {
    FirstFrame,
    Inside,
    Outside,
}

#[derive(Default, Component, Clone)]
pub struct Particle {
    pub active: bool,
    pub pixel: Pixel,
    pub place: bool,
}

impl Particle {
    pub fn new(pixel: Pixel) -> Self {
        Self {
            active: true,
            pixel,
            place: true,
        }
    }

    pub fn visual(pixel: Pixel) -> Self {
        Self {
            active: true,
            pixel,
            place: false,
        }
    }
}

pub struct ParticleApi<'a> {
    pub(super) chunk_position: IVec2,
    pub(super) chunk_group: &'a mut ChunkGroup<Pixel>,
    pub(super) update_send: &'a Sender<UpdateMessage>,
    pub(super) render_send: &'a Sender<RenderMessage>,
}

impl<'a> ParticleApi<'a> {
    pub fn get(&self, x: i32, y: i32) -> Option<Pixel> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        self.chunk_group.get(cell_position).cloned()
    }

    pub fn set(&mut self, x: i32, y: i32, pixel: Pixel) -> Result<(), String> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        match self.chunk_group.get_mut(cell_position) {
            Some(old_pixel) => {
                *old_pixel = pixel;

                self.update_send
                    .try_send(UpdateMessage {
                        cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                        chunk_position: self.chunk_position +
                        cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        awake_surrouding: true,
                    })
                    .unwrap();

                self.render_send
                    .try_send(RenderMessage {
                        cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                        chunk_position: self.chunk_position +
                        cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                    })
                    .unwrap();

                Ok(())
            }
            None => Err("out of bounds".to_string()),
        }
    }

    pub fn set_with_condition<F: Fn(Pixel) -> bool>(
        &mut self,
        x: i32,
        y: i32,
        pixel: Pixel,
        condition: F
    ) -> Result<(), String> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        match self.chunk_group.get_mut(cell_position) {
            Some(initial_pixel) => {
                if condition(initial_pixel.clone()) {
                    *initial_pixel = pixel;

                    self.update_send
                        .try_send(UpdateMessage {
                            cell_position: cell_position
                                .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                .as_uvec2(),
                            chunk_position: self.chunk_position +
                            cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                            awake_surrouding: true,
                        })
                        .unwrap();

                    self.render_send
                        .try_send(RenderMessage {
                            cell_position: cell_position
                                .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                .as_uvec2(),
                            chunk_position: self.chunk_position +
                            cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        })
                        .unwrap();

                    Ok(())
                } else {
                    Err("condition doesn't match".to_string())
                }
            }
            None => Err("out of bounds".to_string()),
        }
    }

    // TODO: rewrite
    pub fn displace(&mut self, pos: IVec2, pixel: Pixel) -> bool {
        let scan_radius = 16;
        let mut scan_pos = IVec2::ZERO;
        let mut scan_delta_pos = IVec2::new(0, -1);

        for _ in 0..scan_radius {
            let check_scan = scan_pos.abs().cmple(IVec2::splat(scan_radius)).all();

            if
                check_scan &&
                self
                    .set_with_condition(
                        pos.x + scan_pos.x,
                        pos.y + scan_pos.y,
                        pixel.clone(),
                        |pixel| pixel.is_empty()
                    )
                    .is_ok()
            {
                return true;
            }

            // update scan coordinates
            if
                scan_pos.x == scan_pos.y ||
                (scan_pos.x < 0 && scan_pos.x == -scan_pos.y) ||
                (scan_pos.x > 0 && scan_pos.x == 1 - scan_pos.y)
            {
                mem::swap(&mut scan_delta_pos.x, &mut scan_delta_pos.y);
                scan_delta_pos.x *= -1;
            }

            scan_pos += scan_delta_pos;
        }

        false
    }
}
//
pub fn particle_setup(mut commands: Commands) {
    commands.spawn((Name::new("Particles"), SpatialBundle::INHERITED_IDENTITY, ParticleParent));
}

pub fn particle_set_parent(
    mut commands: Commands,
    particle_q: Query<Entity, Added<Particle>>,
    particle_parent_q: Query<Entity, With<ParticleParent>>
) {
    let particle_parent = particle_parent_q.single();

    for entity in particle_q.iter() {
        commands.entity(particle_parent).add_child(entity);
    }
}

pub fn particle_modify_velocity(
    mut particle_q: Query<
        (&Particle, &Transform, &mut Velocity, &mut ParticleMovement),
        With<Particle>
    >,
    transform_q: Query<&GlobalTransform, Without<Particle>>,
    time: Res<Time>
) {
    for (_, transform, mut velocity, mut movement) in particle_q
        .iter_mut()
        .filter(|(particle, ..)| particle.active) {
        match *movement {
            ParticleMovement::Fall => {
                velocity.linvel.y -= (0.2 / (CHUNK_SIZE as f32)) * time.delta_seconds() * 25.0;
            }
            ParticleMovement::Follow(target_entity) => {
                let Ok(target_transform) = transform_q.get(target_entity) else {
                    *movement = ParticleMovement::Fall;
                    continue;
                };

                let distance = target_transform.translation().xy() - transform.translation.xy();

                if distance.length() * (CHUNK_SIZE as f32) > 24.0 {
                    *movement = ParticleMovement::Fall;
                    continue;
                }

                let angle = distance.normalize_or_zero().to_angle();
                let magnitude = distance.length_recip().sqrt();

                velocity.linvel =
                    (
                        (Vec2::new(angle.cos(), angle.sin()) * magnitude +
                            (Vec2::new(fastrand::f32(), fastrand::f32()) / 2.0 - 0.5)) /
                        (CHUNK_SIZE as f32)
                    ).clamp(-Vec2::ONE, Vec2::ONE) *
                    time.delta_seconds() *
                    25.0 * (fastrand::f32() * 0.05 + 0.95);
            }
        }
    }
}

pub fn particles_update(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut particles: Query<
        (
            Entity,
            &mut Transform,
            &mut Velocity,
            &mut Particle,
            &mut ParticleObjectState,
            &mut ParticleMovement,
        )
    >,
    transform_q: Query<&GlobalTransform, Without<Particle>>,
    particles_instances: Query<Entity, With<ParticleParent>>
) {
    let particles_instances = particles_instances.single();

    let DirtyRects { new: new_dirty_rects, render: render_rects, .. } = &mut *dirty_rects_resource;

    let (update_send, update_recv) = async_channel::unbounded::<UpdateMessage>();
    let (render_send, render_recv) = async_channel::unbounded::<RenderMessage>();
    let (particle_send, particle_recv) = async_channel::unbounded::<Entity>();

    ComputeTaskPool::get().scope(|scope| {
        scope.spawn(async move {
            while let Ok(update) = update_recv.recv().await {
                if update.awake_surrouding {
                    update_dirty_rects_3x3(
                        new_dirty_rects,
                        update.chunk_position,
                        update.cell_position
                    );
                } else {
                    update_dirty_rects(
                        new_dirty_rects,
                        update.chunk_position,
                        update.cell_position
                    );
                }
            }
        });

        scope.spawn(async move {
            while let Ok(update) = render_recv.recv().await {
                update_dirty_rects(render_rects, update.chunk_position, update.cell_position);
            }
        });

        scope.spawn(async move {
            while let Ok(entity) = particle_recv.recv().await {
                commands.entity(particles_instances).remove_children(&[entity]);
                commands.entity(entity).despawn();
            }
        });

        let update_send = &update_send;
        let render_send = &render_send;
        let particle_send = &particle_send;
        let transform_q = &transform_q;

        let mut particles_maps = [
            HashMap::default(),
            HashMap::default(),
            HashMap::default(),
            HashMap::default(),
        ];

        for (entity, transform, velocity, particle, object_state, movement) in particles
            .iter_mut()
            .filter(|(_, _, _, particle, ..)| particle.active) {
            let chunk_position = transform.translation.xy().as_ivec2();

            (
                unsafe {
                    particles_maps.get_unchecked_mut(
                        ((chunk_position.x.abs() % 2) + (chunk_position.y.abs() % 2) * 2) as usize
                    )
                }
            )
                .entry(chunk_position)
                .or_insert_with(Vec::new)
                .push((entity, transform, velocity, particle, object_state, movement));
        }

        particles_maps.into_iter().for_each(|map| {
            ComputeTaskPool::get().scope(|scope| {
                map.into_iter()
                    .filter_map(|(position, particles)|
                        build_chunk_group(&mut chunk_manager, position).map(|chunk_group| (
                            position,
                            particles,
                            chunk_group,
                        ))
                    )
                    .for_each(|(position, particles, mut chunk_group)| {
                        scope.spawn(async move {
                            let mut api = ParticleApi {
                                chunk_position: position,
                                chunk_group: &mut chunk_group,
                                update_send,
                                render_send,
                            };

                            particles
                                .into_iter()
                                .filter_map(
                                    |(
                                        entity,
                                        mut transform,
                                        mut velocity,
                                        particle,
                                        mut object_state,
                                        mut movement,
                                    )| {
                                        let initial = transform.translation.xy();
                                        let delta = velocity.linvel;

                                        for position in WalkGrid::new(
                                            (initial * (CHUNK_SIZE as f32)).as_ivec2(),
                                            ((initial + delta) * (CHUNK_SIZE as f32)).as_ivec2()
                                        ) {
                                            let Some(pixel) = api.get(
                                                position.x as i32,
                                                position.y as i32
                                            ) else {
                                                continue;
                                            };

                                            match *movement {
                                                ParticleMovement::Fall => {
                                                    if pixel.is_empty() {
                                                        *object_state =
                                                            ParticleObjectState::Outside;
                                                        continue;
                                                    }

                                                    let is_object = matches!(
                                                        pixel.physics_type,
                                                        PhysicsType::Rigidbody { .. }
                                                    );

                                                    match *object_state {
                                                        ParticleObjectState::FirstFrame => {
                                                            if is_object {
                                                                *object_state =
                                                                    ParticleObjectState::Inside;
                                                            } else {
                                                                *object_state =
                                                                    ParticleObjectState::Outside;
                                                            }
                                                        }
                                                        ParticleObjectState::Inside => {
                                                            if !is_object {
                                                                *object_state =
                                                                    ParticleObjectState::Outside;
                                                            }
                                                        }
                                                        ParticleObjectState::Outside => {}
                                                    }

                                                    if
                                                        !is_object ||
                                                        *object_state ==
                                                            ParticleObjectState::Outside
                                                    {
                                                        match pixel.physics_type {
                                                            | PhysicsType::Air
                                                            | PhysicsType::Gas(..) => {
                                                                if !particle.place ||
                                                                        api
                                                                            .set(
                                                                                position.x as i32,
                                                                                position.y as i32,
                                                                                particle.pixel.clone()
                                                                            )
                                                                            .is_ok()
                                                                    {
                                                                        return Some(entity);
                                                                    }
                                                            }
                                                            _ => {
                                                                let succeeded = !particle.place || api.displace(
                                                                    IVec2::new(
                                                                        position.x as i32,
                                                                        position.y as i32
                                                                    ),
                                                                    particle.pixel.clone()
                                                                );

                                                                if succeeded {
                                                                    return Some(entity);
                                                                }

                                                                // upwarp if completely blocked
                                                                velocity.linvel.y =
                                                                    1.0 / (CHUNK_SIZE as f32);
                                                                transform.translation.y +=
                                                                    4.0 / (CHUNK_SIZE as f32);

                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                                ParticleMovement::Follow(target_entity) => {
                                                    let Ok(target_transform) =
                                                        transform_q.get(target_entity) else {
                                                        *movement = ParticleMovement::Fall;
                                                        break;
                                                    };

                                                    if
                                                        (
                                                            position.as_vec2() / (CHUNK_SIZE as f32)
                                                        ).distance(
                                                            target_transform.translation().xy()
                                                        ) < 4.0 / (CHUNK_SIZE as f32)
                                                    {
                                                        return Some(entity);
                                                    }
                                                }
                                            }
                                        }

                                        transform.translation.x += delta.x;
                                        transform.translation.y += delta.y;

                                        None
                                    }
                                )
                                .for_each(|entity| {
                                    particle_send.try_send(entity).unwrap();
                                });
                        });
                    });
            });
        });

        update_send.close();
        render_send.close();
        particle_send.close();
    });
}
