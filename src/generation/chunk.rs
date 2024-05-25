use std::{ mem, sync::Arc };

use benimator::FrameRate;
use bevy::{
    asset::Assets,
    ecs::{
        entity::Entity,
        event::EventReader,
        query::With,
        system::{ CommandQueue, Commands, Query, Res, ResMut, RunSystemOnce, SystemState },
    },
    hierarchy::{ BuildChildren, Children },
    prelude::*,
    render::texture::Image,
    tasks::{ block_on, futures_lite::future, AsyncComputeTaskPool, Task },
    transform::{ commands, components::Transform, TransformBundle },
    utils::petgraph::matrix_graph::Zero,
};
use bevy_math::{ ivec2, IVec2 };
use bevy_rapier2d::{
    dynamics::GravityScale,
    geometry::{ Collider, CollisionGroups, Group, Sensor },
};
use itertools::Itertools;

use crate::{
    actors::{
        actor::{ Actor, ActorBundle, ActorColliderBundle, MovementType },
        enemy::{ Enemy, Flipped },
        health::HealthBarOverlay,
    },
    animation::{ Animation, AnimationState },
    assets::{ ChunkLayoutAssets, SpriteSheets },
    constants::{ CHUNK_SIZE, ENEMY_Z, PLAYER_Z },
    helpers::to_index,
    registries::Registries,
    simulation::{
        chunk::{ Chunk, ChunkState },
        chunk_groups::{ self, build_chunk_group },
        chunk_manager::ChunkManager,
        colliders::{ ENEMY_MASK, HITBOX_MASK, OBJECT_MASK, PLAYER_MASK, TERRAIN_MASK },
        pixel::Pixel,
    },
};

use super::{ LevelData, Noise, PoissonEnemyPosition };

#[derive(Event, Deref)]
pub struct GenerationEvent(pub IVec2);

#[derive(Component)]
pub struct GenerationTask(pub Task<(Vec<Pixel>, Vec<u8>)>);

pub fn process_chunk_generation_events(
    mut commands: Commands,
    mut ev_chunkgen: EventReader<GenerationEvent>,
    chunk_manager: Res<ChunkManager>,
    images: Res<Assets<Image>>,
    registries: Res<Registries>,
    noise: Res<Noise>,
    level_data: Res<LevelData>
) {
    if !ev_chunkgen.is_empty() {
        let thread_pool = AsyncComputeTaskPool::get();
        let image = Arc::new(images.get(level_data.1.clone()).unwrap().clone());
        
        let powder = Arc::new(registries.materials.get(&level_data.0.powder_id).unwrap().clone());
        let liquid = Arc::new(registries.materials.get(&level_data.0.liquid_id).unwrap().clone());

        let terrain_layers = level_data.0.terrain_layers.iter().map(|level| {
            (level.value, Arc::new(registries.materials.get(&level.material_id).unwrap().clone()))
        }).collect_vec();

        let background_layers = level_data.0.background_layers.iter().map(|level| {
            (level.value, Arc::new(registries.materials.get(&level.material_id).unwrap().clone()))
        }).collect_vec();

        for ev in ev_chunkgen.read() {
            let position = ev.0;
            let image = image.clone();

            let Noise {
                terrain_noise,
                sand_noise,
                liquid_noise,
            } = noise.clone();

            let powder = powder.clone();
            let liquid = liquid.clone();
            let terrain_layers = terrain_layers.clone();
            let background_layers = background_layers.clone();

            commands.entity(chunk_manager.get_chunk_id(&ev.0).unwrap()).insert(
                GenerationTask(
                    thread_pool.spawn(async move {
                        let texture_position = position * CHUNK_SIZE + image.size().as_ivec2() / 2;

                        let pixels = (0..CHUNK_SIZE.pow(2))
                            .map(|index| {
                                let point =
                                    position.as_vec2() +
                                    ivec2(index % CHUNK_SIZE, index / CHUNK_SIZE).as_vec2() /
                                        (CHUNK_SIZE as f32);

                                let texture_position = (
                                    texture_position + ivec2(index % CHUNK_SIZE, index / CHUNK_SIZE)
                                ).clamp(IVec2::ZERO, image.size().as_ivec2() - 1);

                                let texture_modifier =
                                    (
                                        image.data
                                            [

                                                    (
                                                        (texture_position.y *
                                                            (image.height() as i32) +
                                                            texture_position.x) as usize
                                                    ) * 4

                                            ] as f32
                                    ) / 255.0;

                                let value = terrain_noise(point) * texture_modifier;
                                let powder_value = sand_noise(point);
                                let liquid_value = liquid_noise(point);

                                if value < terrain_layers.last().unwrap().0 && !powder_value.is_zero() && liquid_value < 0.8 {
                                    return Pixel::from(powder.as_ref().clone());
                                }

                                if value < terrain_layers[0].0 {
                                    if value < 0.1 && liquid_value > 0.8 {
                                        return Pixel::from(liquid.as_ref().clone())
                                    } else {
                                        return Pixel::from(terrain_layers[0].1.as_ref().clone());
                                    }
                                }

                                for layer in terrain_layers.iter() {
                                    if value < layer.0 {
                                        return Pixel::from(layer.1.as_ref().clone());
                                    }
                                }

                                return Pixel::default();
                            })
                            .collect_vec();

                        let bg_texture = (0..CHUNK_SIZE.pow(2))
                            .map(|index| {
                                let point =
                                    position.as_vec2() +
                                    ivec2(index % CHUNK_SIZE, index / CHUNK_SIZE).as_vec2() /
                                        (CHUNK_SIZE as f32);

                                let texture_position = (
                                    texture_position + ivec2(index % CHUNK_SIZE, index / CHUNK_SIZE)
                                ).clamp(IVec2::ZERO, image.size().as_ivec2() - 1);

                                let texture_modifier =
                                    (
                                        image.data
                                            [

                                                    (
                                                        (texture_position.y *
                                                            (image.height() as i32) +
                                                            texture_position.x) as usize
                                                    ) * 4

                                            ] as f32
                                    ) / 255.0;

                                let value = terrain_noise(point) * texture_modifier;

                                for layer in background_layers.iter() {
                                    if value < layer.0 {
                                        return Pixel::from(layer.1.as_ref().clone());
                                    }
                                }

                                return Pixel::default();
                            })
                            .map(|pixel| {
                                let mut color = pixel.get_color();

                                let f = 0.6;

                                let (r, g, b) = (
                                    color[0] as f32,
                                    color[1] as f32,
                                    color[2] as f32,
                                );

                                let l = 0.3 * r + 0.6 * g + 0.1 * b;

                                color[0] = (((r + f * (l - r)) * 0.8) as u8).saturating_sub(25);
                                color[1] = (((g + f * (l - g)) * 0.8) as u8).saturating_sub(25);
                                color[2] = (((b + f * (l - b)) * 0.8) as u8).saturating_sub(25);

                                color
                            })
                            .flatten()
                            .collect_vec();

                        (pixels, bg_texture)
                    })
                )
            );
        }
    }
}

pub fn process_chunk_generation_tasks(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut images: ResMut<Assets<Image>>,
    mut chunk_q: Query<(Entity, &Transform, &mut GenerationTask), With<Chunk>>,
    mut awaiting: ResMut<AwaitingNearbyChunks>
) {
    for (entity, transform, mut task) in chunk_q.iter_mut() {
        let result = block_on(future::poll_once(&mut task.0));

        if let Some((pixels, bg_texture)) = result {
            let position = transform.translation.xy().round().as_ivec2();

            let chunk = chunk_manager.get_chunk_data_mut(&position).unwrap();
            chunk.pixels = pixels;

            images.get_mut(chunk.background.clone()).unwrap().data.copy_from_slice(&bg_texture);

            commands
                .entity(entity)
                .with_children(|parent| {
                    if let Ok(colliders) = chunk.build_colliders() {
                        for collider in colliders {
                            parent.spawn((
                                collider,
                                TransformBundle {
                                    local: Transform::IDENTITY,
                                    ..Default::default()
                                },
                                CollisionGroups::new(
                                    Group::from_bits_truncate(TERRAIN_MASK),
                                    Group::from_bits_truncate(OBJECT_MASK)
                                ),
                            ));
                        }
                    }
                })
                .remove::<GenerationTask>();

            chunk.update_texture(images.get_mut(&chunk.texture.clone()).unwrap());
            chunk.state = ChunkState::Populating;
            awaiting.push(position);
        }
    }
}

#[derive(Default, Resource, Deref, DerefMut)]
pub struct AwaitingNearbyChunks(Vec<IVec2>);

pub fn populate_chunk(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut awaiting: ResMut<AwaitingNearbyChunks>,
    mut enemies_queue: ResMut<PoissonEnemyPosition>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    sprites: Res<SpriteSheets>
) {
    awaiting.retain(|position| {
        let can_proceed = (-1..=1).cartesian_product(-1..=1).all(|(x, y)| {
            if x == 0 && y == 0 {
                return true;
            }

            chunk_manager
                .get_chunk_data(&(*position + ivec2(x, y)))
                .map_or(false, |chunk| chunk.state >= ChunkState::Populating)
        });

        if can_proceed {
            if let Some(mut enemy_positions) = enemies_queue.remove(position) {
                enemy_positions
                    .iter_mut()
                    .filter_map(|enemy_position| {
                        let local_enemy_position = (*enemy_position * (CHUNK_SIZE as f32))
                            .as_ivec2()
                            .rem_euclid(IVec2::splat(CHUNK_SIZE));

                        let Some(chunk_group) = build_chunk_group(
                            &mut chunk_manager,
                            *position
                        ) else {
                            panic!("wtf");
                        };

                        let scan_radius = 16;
                        let mut scan_pos = IVec2::ZERO;
                        let mut scan_delta_pos = IVec2::new(0, -1);

                        for _ in 0..scan_radius {
                            let check_scan = scan_pos.abs().cmple(IVec2::splat(scan_radius)).all();

                            let can_fit = (-3..=3).cartesian_product(-3..3).all(|(dx, dy)| {
                                if
                                    let Some(pixel) = chunk_group.get(
                                        local_enemy_position + scan_pos + ivec2(dx, dy)
                                    )
                                {
                                    pixel.is_empty()
                                } else {
                                    false
                                }
                            });

                            if check_scan && can_fit {
                                return Some(
                                    *enemy_position + scan_pos.as_vec2() / (CHUNK_SIZE as f32)
                                );
                            }

                            if
                                scan_pos.x == scan_pos.y ||
                                (scan_pos.x < 0 && scan_pos.x == -scan_pos.y) ||
                                (scan_pos.x > 0 && scan_pos.x == 1 - scan_pos.y)
                            {
                                mem::swap(&mut scan_delta_pos.x, &mut scan_delta_pos.y);
                                scan_delta_pos.x *= -1;
                            }

                            scan_pos.x += scan_delta_pos.x;
                            scan_pos.y += scan_delta_pos.y;
                        }

                        None
                    })
                    .for_each(|position| {
                        let texture_atlas_layout = texture_atlas_layouts.add(
                            TextureAtlasLayout::from_grid(Vec2::splat(17.0), 6, 1, None, None)
                        );

                        let animation = Animation(
                            benimator::Animation
                                ::from_indices(0..=5, FrameRate::from_fps(12.0))
                                .repeat()
                        );

                        commands
                            .spawn((
                                Name::new("Enemy"),
                                Enemy,
                                ActorBundle {
                                    actor: Actor {
                                        position: position * (CHUNK_SIZE as f32),
                                        size: Vec2::new(6.0, 6.0),
                                        movement_type: MovementType::Floating,
                                        ..Default::default()
                                    },
                                    collider: Collider::ball(6.0),
                                    sprite: SpriteSheetBundle {
                                        texture: sprites.bat.clone(),
                                        atlas: TextureAtlas {
                                            layout: texture_atlas_layout.clone(),
                                            ..Default::default()
                                        },
                                        transform: Transform {
                                            translation: position.extend(ENEMY_Z),
                                            scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                                            ..Default::default()
                                        },
                                        sprite: Sprite {
                                            anchor: bevy::sprite::Anchor::Center,
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                },
                                HealthBarOverlay {
                                    offset: Vec2::new(0.0, 14.0),
                                    width: 12.0,
                                },
                                AnimationState::default(),
                                animation.clone(),
                                GravityScale(0.05),
                                Flipped(false),
                            ))
                            .with_children(|parent| {
                                parent.spawn((
                                    Sensor,
                                    ActorColliderBundle {
                                        collider: Collider::ball(6.0),
                                        collision_groups: CollisionGroups::new(
                                            Group::from_bits_retain(ENEMY_MASK | HITBOX_MASK),
                                            Group::from_bits_retain(PLAYER_MASK)
                                        ),
                                        ..Default::default()
                                    },
                                ));
                            });
                    });
            }

            chunk_manager.get_chunk_data_mut(position).unwrap().state = ChunkState::Sleeping;
        }

        !can_proceed
    });
}

#[derive(Event, Deref)]
pub struct NoiseStepEvent(IVec2);

#[derive(Event)]
pub struct StructureStepEvent(IVec2);

#[derive(Event)]
pub struct ConversionStepEvent(IVec2);

// fn noise_calculation(
//     events: EventReader<NoiseStepEvent>,
//     position: In<IVec2>,
//     chunk_map: Res<ChunkMapAssets>,
//     images: Res<Assets<Image>>,
//     noise: Res<Noise>
// ) -> GenerationState {
//     for ev in events {
//         let thread_pool = AsyncComputeTaskPool::get();
//         let image = images.get(chunk_map.texture.clone()).unwrap().clone();
//         let noise = noise.0.clone();

//         let task = thread_pool.spawn(async move {
//             let texture_position = *position * CHUNK_SIZE + image.size().as_ivec2() / 2;

//             (0..CHUNK_SIZE.pow(2))
//                 .map(|index| {
//                     let point =
//                         (position.as_vec2() +
//                             ivec2(index % CHUNK_SIZE, index / CHUNK_SIZE).as_vec2() /
//                                 (CHUNK_SIZE as f32)) /
//                         48.0;

//                     let texture_position = (
//                         texture_position + ivec2(index % CHUNK_SIZE, index / CHUNK_SIZE)
//                     ).clamp(IVec2::ZERO, image.size().as_ivec2() - 1);

//                     let texture_modifier =
//                         (
//                             image.data
//                                 [

//                                         (
//                                             (texture_position.y * (image.height() as i32) +
//                                                 texture_position.x) as usize
//                                         ) * 4

//                                 ] as f32
//                         ) / 255.0;

//                     noise(point) * texture_modifier
//                 })
//                 .collect_vec()
//         });
//     }

//     GenerationState::NoiseCalculation(Some(task))
// }

// pub fn process_generation_states(
//     mut commands: Commands,
//     mut chunk_q: Query<(Entity, &mut GenerationState, &Transform), With<Chunk>>
// ) {
//     let thread_pool = AsyncComputeTaskPool::get();

//     for (entity, state, task, transform) in chunk_q.iter_mut() {
//         match state.as_ref() {
//             GenerationState::NoiseCalculation => {
//                 match task {
//                     Some(task) => {}
//                     None => {
//                         commands.run_system_with_input(
//                             noise_calculation,
//                             transform.translation.xy().round().as_ivec2()
//                         );
//                     }
//                 }
//             }
//             GenerationState::Structure => todo!(),
//             GenerationState::Conversion => todo!(),
//             GenerationState::PostProcesing => todo!(),
//         };
//     }
// }
