use async_channel::{ Receiver, Sender };
use bevy::{
    asset::Assets,
    ecs::{
        entity::Entity,
        event::{ Event, EventReader },
        query::With,
        system::{ Commands, Query, Res, ResMut, Resource },
    },
    gizmos::gizmos::Gizmos,
    hierarchy::BuildChildren,
    log::info,
    reflect::Reflect,
    render::{ color::Color, texture::Image, view::InheritedVisibility },
    sprite::{ Anchor, Sprite, SpriteBundle },
    tasks::{ block_on, futures_lite::future, AsyncComputeTaskPool, Task },
    transform::{ components::Transform, TransformBundle },
    utils::{ HashMap, HashSet },
};
use bevy_math::{ ivec2, vec2, IVec2, Vec2, Vec3 };
use bevy_rapier2d::dynamics::RigidBody;
use itertools::Itertools;
use noise::NoiseFn;

use crate::{
    assets::TileAssets,
    constants::CHUNK_SIZE,
    generation::biome::Biome,
    helpers::to_index,
    registries::Registries,
    simulation::{
        chunk::{ Chunk, ChunkData, ChunkState },
        chunk_groups::ChunkGroupMut,
        chunk_manager::{ ChunkManager, Chunks },
        materials::MaterialInstance,
        pixel::Pixel,
        FbmNoiseRes,
        RidgedNoise1Res,
        RidgedNoise2Res,
    },
};

#[derive(Event)]
pub struct TileRequestEvent(pub IVec2);

#[derive(Default, Resource)]
pub struct TileGenerator {
    pub queue: HashSet<IVec2>,
    pub task: Option<Task<(IVec2, Vec<Pixel>, Vec<u8>)>>,
}

pub fn process_tile_requests(
    mut ev_requests: EventReader<TileRequestEvent>,
    mut tile_generator: ResMut<TileGenerator>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    registries: Res<Registries>,
    chunks_query: Query<Entity, With<Chunks>>,
    ridged_noise_1: Res<RidgedNoise1Res>,
    ridged_noise_2: Res<RidgedNoise2Res>,
    fbm_noise: Res<FbmNoiseRes>
) {
    for ev in ev_requests.read() {
        tile_generator.queue.insert(ev.0);
    }

    if tile_generator.task.is_some() {
        let result = block_on(future::poll_once(tile_generator.task.as_mut().unwrap()));

        if let Some((position, pixels, bg_texture)) = result {
            let chunks_entity = chunks_query.single();

            let chunk = ChunkData {
                pixels,
                texture: images.add(ChunkData::new_image()),
                ..Default::default()
            };

            let entity = commands
                .spawn((
                    Chunk,
                    RigidBody::Fixed,
                    InheritedVisibility::VISIBLE,
                    TransformBundle {
                        local: Transform::from_translation(position.as_vec2().extend(0.0)),
                        ..Default::default()
                    },
                ))
                .id();

            commands.entity(entity).with_children(|children| {
                children.spawn((
                    Chunk,
                    SpriteBundle {
                        texture: chunk.texture.clone(),
                        sprite: Sprite {
                            custom_size: Some(vec2(1.0, 1.0)),
                            anchor: Anchor::BottomLeft,
                            flip_y: true,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ));

                let mut bg_image = ChunkData::new_image();
                bg_image.data.copy_from_slice(&bg_texture);

                children.spawn(SpriteBundle {
                    texture: images.add(bg_image),
                    sprite: Sprite {
                        custom_size: Some(vec2(1.0, 1.0)),
                        anchor: Anchor::BottomLeft,
                        flip_y: true,
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec2::ZERO.extend(-1.0)),
                    ..Default::default()
                });

                if let Ok(colliders) = chunk.build_colliders() {
                    for collider in colliders {
                        children.spawn((
                            collider,
                            TransformBundle {
                                local: Transform::IDENTITY,
                                ..Default::default()
                            },
                        ));
                    }
                }
            });

            commands.entity(chunks_entity).add_child(entity);

            chunk.update_all(images.get_mut(&chunk.texture.clone()).unwrap());
            chunk_manager.chunks.insert(position, (entity, chunk));

            tile_generator.task = None;
        }
    }

    if tile_generator.task.is_none() && !tile_generator.queue.is_empty() {
        let position = {
            let value = tile_generator.queue.iter().next().cloned().unwrap();
            tile_generator.queue.remove(&value);

            value
        };

        if chunk_manager.chunks.contains_key(&position) {
            return;
        }

        let thread_pool = AsyncComputeTaskPool::get();
        let r_noise_1 = ridged_noise_1.0.clone();
        let r_noise_2 = ridged_noise_2.0.clone();
        let fbm_noise = fbm_noise.0.clone();
        let materials_lock = registries.materials.clone();
        tile_generator.task = Some(
            thread_pool.spawn(async move {
                let materials = materials_lock.read();
                let material_1 = materials.get("stone").unwrap();
                let material_2 = materials.get("dirt").unwrap();
                let material_3 = materials.get("grass").unwrap();

                let pixels = (0..CHUNK_SIZE.pow(2))
                    .map(|index| {
                        let point =
                            (position.as_vec2() +
                                ivec2(index % CHUNK_SIZE, index / CHUNK_SIZE).as_vec2() /
                                    (CHUNK_SIZE as f32)) /
                            48.0;

                        let offset = fbm_noise.get([point.x as f64, point.y as f64]) / 2.0;

                        r_noise_1.get([(point.x as f64) + offset, point.y as f64]) +
                            r_noise_2.get([(point.x as f64) + offset, point.y as f64]) / 4.0
                    })
                    .map(|value| {
                        if value < 0.4 {
                            Pixel::new(material_1.clone().into())
                        } else if value < 0.55 {
                            Pixel::new(material_2.clone().into())
                        } else if value < 0.6 {
                            Pixel::new(material_3.clone().into())
                        } else {
                            Pixel::default()
                        }
                    })
                    .collect_vec();

                let bg_texture = (0..CHUNK_SIZE.pow(2))
                    .map(|index| {
                        let point =
                            (position.as_vec2() +
                                ivec2(index % CHUNK_SIZE, index / CHUNK_SIZE).as_vec2() /
                                    (CHUNK_SIZE as f32)) /
                            48.0;

                        let offset = fbm_noise.get([point.x as f64, point.y as f64]) / 2.0;

                        r_noise_1.get([(point.x as f64) + offset, point.y as f64]) +
                            r_noise_2.get([(point.x as f64) + offset, point.y as f64]) / 4.0
                    })
                    .map(|value| {
                        let mut colors = if value < 0.4 {
                            MaterialInstance::from(material_1).color
                        } else if value < 0.55 {
                            MaterialInstance::from(material_2).color
                        } else if value < 0.75 {
                            MaterialInstance::from(material_3).color
                        } else {
                            [0; 4]
                        };

                        let f = 0.6;

                        let (r, g, b) = (
                            colors[0] as f32,
                            colors[1] as f32,
                            colors[2] as f32,
                        );

                        let l = 0.3 * r + 0.6 * g + 0.1 * b;

                        colors[0] = (((r + f * (l - r)) * 0.8) as u8).saturating_sub(25);
                        colors[1] = (((g + f * (l - g)) * 0.8) as u8).saturating_sub(25);
                        colors[2] = (((b + f * (l - b)) * 0.8) as u8).saturating_sub(25);

                        colors
                    })
                    .flatten()
                    .collect_vec();

                (position, pixels, bg_texture)
            })
        );
    }
}