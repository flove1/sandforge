use bevy::{ prelude::*, tasks::ComputeTaskPool, utils::HashMap };
use bevy_math::{ ivec2, IVec2, Rect, UVec2, Vec3Swizzles };
use itertools::{ Either, Itertools };

use crate::{
    camera::TrackingCamera,
    constants::CHUNK_SIZE,
    generation::chunk::GenerationEvent,
    registries::Registries,
};

use super::{
    chunk::{ Chunk, ChunkApi, ChunkData, ChunkState },
    chunk_groups::build_chunk_group,
    colliders::ChunkColliderEvent,
    dirty_rect::{
        update_dirty_rects,
        update_dirty_rects_3x3,
        DirtyRects,
        RenderMessage,
        UpdateMessage,
    },
    materials::{
        update_fire,
        update_gas,
        update_liquid,
        update_powder,
        update_reactions,
        Material,
        PhysicsType,
    },
    pixel::Pixel,
};

#[derive(Component)]
pub struct Terrain;

#[derive(Resource)]
pub struct ChunkManager {
    pub chunks: HashMap<IVec2, (Entity, ChunkData)>,
    clock: u8,
}

impl FromWorld for ChunkManager {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self {
            chunks: HashMap::new(),
            clock: 0,
        }
    }
}

impl std::ops::Index<IVec2> for ChunkManager {
    type Output = Pixel;

    fn index(&self, index: IVec2) -> &Self::Output {
        self.get(index).expect("pixel not exists")
    }
}

impl std::ops::IndexMut<IVec2> for ChunkManager {
    fn index_mut(&mut self, index: IVec2) -> &mut Self::Output {
        self.get_mut(index).expect("pixel not exists")
    }
}

impl ChunkManager {
    pub fn clock(&self) -> u8 {
        self.clock
    }

    pub fn get(&self, pos: IVec2) -> Result<&Pixel, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.get_chunk_data(&chunk_position)
            .filter(
                |chunk| (chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping)
            )
            .map(|chunk| &chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_mut(&mut self, pos: IVec2) -> Result<&mut Pixel, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.get_chunk_data_mut(&chunk_position)
            .filter(
                |chunk| (chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping)
            )
            .map(|chunk| &mut chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn set(&mut self, pos: IVec2, pixel: Pixel) -> Result<(), String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if
            let Some(chunk) = self
                .get_chunk_data_mut(&chunk_position)
                .filter(
                    |chunk|
                        chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping
                )
        {
            chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)] = pixel;
            Ok(())
        } else {
            Err("pixel not loaded yet".to_string())
        }
    }

    pub fn get_chunk_data(&self, chunk_position: &IVec2) -> Option<&ChunkData> {
        self.chunks.get(chunk_position).map(|chunk| &chunk.1)
    }

    pub fn get_chunk_data_mut(&mut self, chunk_position: &IVec2) -> Option<&mut ChunkData> {
        self.chunks.get_mut(chunk_position).map(|chunk| &mut chunk.1)
    }
}

pub fn manager_setup(mut commands: Commands) {
    commands.spawn((Name::new("Terrain"), SpatialBundle::INHERITED_IDENTITY, Terrain));
}

pub fn chunk_set_parent(
    mut commands: Commands,
    chunk_q: Query<Entity, Added<Chunk>>,
    terrain_q: Query<Entity, With<Terrain>>
) {
    let terrain = terrain_q.single();

    for entity in chunk_q.iter() {
        commands.entity(terrain).add_child(entity);
    }
}

pub fn update_loaded_chunks(
    mut ev_chunkgen: EventWriter<GenerationEvent>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    camera_q: Query<(&Transform, &OrthographicProjection), With<TrackingCamera>>
) {
    let DirtyRects { current, .. } = &mut *dirty_rects_resource;
    let (transform, projection) = camera_q.single();

    let area = Rect::from_center_size(transform.translation.xy(), projection.area.size() + 4.0);

    // suspend chunks out of bounds
    chunk_manager.chunks
        .iter_mut()
        .map(|(position, chunk)| (position, &mut chunk.1))
        .filter(|(_, chunk)| chunk.state == ChunkState::Active)
        .for_each(|(position, chunk)| {
            if !area.contains(position.as_vec2()) {
                chunk.state = ChunkState::Sleeping;
            }
        });

    for x in area.min.x.ceil() as i32..area.max.x.floor() as i32 {
        for y in area.min.y.ceil() as i32..area.max.y.floor() as i32 {
            let position = ivec2(x, y);

            match chunk_manager.get_chunk_data_mut(&position) {
                Some(chunk) => {
                    if chunk.state == ChunkState::Sleeping {
                        update_dirty_rects(current, position, UVec2::ZERO);
                        update_dirty_rects(
                            current,
                            position,
                            UVec2::splat((CHUNK_SIZE as u32) - 1)
                        );
                        chunk.state = ChunkState::Active;
                    }
                }
                None => {
                    ev_chunkgen.send(GenerationEvent(position));
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn chunks_update(
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut collider_ev: EventWriter<ChunkColliderEvent>,
    registries: Res<Registries>
) {
    let DirtyRects {
        current: dirty_rects,
        new: new_dirty_rects,
        render: render_rects,
        collider: colliders,
    } = &mut *dirty_rects_resource;

    let (update_send, update_recv) = async_channel::unbounded::<UpdateMessage>();
    let (render_send, render_recv) = async_channel::unbounded::<RenderMessage>();
    let (collider_send, collider_recv) = async_channel::unbounded::<IVec2>();

    chunk_manager.clock = chunk_manager.clock.wrapping_add(1);

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
            while let Ok(position) = collider_recv.recv().await {
                colliders.insert(position);
            }
        });

        let update_send = &update_send;
        let render_send = &render_send;
        let collider_send = &collider_send;
        let materials = &registries.materials;
        let clock = chunk_manager.clock;

        let active_chunks = chunk_manager.chunks
            .iter()
            .map(|(position, chunk)| (position, &chunk.1))
            .filter(|(_, chunk)| chunk.state == ChunkState::Active)
            .map(|(position, _)| *position)
            .collect_vec();

        let groups_by_y = active_chunks.into_iter().group_by(|position| position.y);

        let groups_by_x = groups_by_y
            .into_iter()
            .sorted_by_key(|(y, _)| *y)
            .map(|(_, group)| group.into_iter())
            .flat_map(|group| {
                group
                    .group_by(|position| position.x % 2 == 0)
                    .into_iter()
                    .map(|(_, group)| group.collect_vec())
                    .collect_vec()
            });

        for group in groups_by_x {
            ComputeTaskPool::get().scope(|scope| {
                group
                    .into_iter()
                    .filter_map(|position| {
                        dirty_rects
                            .get(&position)
                            .cloned()
                            .map(|dirty_rect| (position, dirty_rect))
                    })
                    .filter_map(|(position, dirty_rect)| {
                        build_chunk_group(&mut chunk_manager, position).map(|chunk_group| (
                            position,
                            dirty_rect,
                            chunk_group,
                        ))
                    })
                    .for_each(|(position, dirty_rect, mut chunk_group)| {
                        scope.spawn(async move {
                            let api = ChunkApi {
                                cell_position: ivec2(0, 0),
                                chunk_position: position,
                                chunk_group: &mut chunk_group,
                                update_send,
                                render_send,
                                collider_send,
                                clock,
                            };

                            update_chunk(api, dirty_rect, materials);
                        })
                    })
            });
        }

        update_send.close();
        render_send.close();
        collider_send.close();
    });

    dirty_rects_resource.collider.iter().for_each(|position| {
        collider_ev.send(ChunkColliderEvent(*position));
    });

    let new_positions = dirty_rects_resource.new.keys().copied().collect::<Vec<IVec2>>();

    new_positions.iter().for_each(|position| {
        if !dirty_rects_resource.current.contains_key(position) {
            update_dirty_rects(&mut dirty_rects_resource.new, *position, UVec2::ZERO);
            update_dirty_rects(
                &mut dirty_rects_resource.new,
                *position,
                UVec2::ONE * ((CHUNK_SIZE - 1) as u32)
            );
        }
    });

    dirty_rects_resource.current.clear();
    dirty_rects_resource.collider.clear();
    dirty_rects_resource.swap();
}

fn update_chunk(mut api: ChunkApi, dirty_rect: URect, materials: &HashMap<String, Material>) {
    let x_range = if api.clock % 2 == 0 {
        Either::Left(dirty_rect.min.x as i32..dirty_rect.max.x as i32)
    } else {
        Either::Right((dirty_rect.min.x as i32..dirty_rect.max.x as i32).rev())
    };

    for x_cell in x_range {
        for y_cell in dirty_rect.min.y as i32..dirty_rect.max.y as i32 {
            api.switch_position(ivec2(x_cell, y_cell));

            if api.get_counter(0, 0) == api.clock {
                api.keep_alive(0, 0);
                continue;
            }

            match api.get_physics_type(0, 0) {
                PhysicsType::Powder => {
                    update_powder(&mut api);
                }
                PhysicsType::Gas(..) => {
                    update_gas(&mut api);
                }
                PhysicsType::Liquid(..) => {
                    update_liquid(&mut api);
                }
                _ => {}
            }

            if update_fire(&mut api) {
                continue;
            }

            update_reactions(&mut api, materials);

            api.mark_updated();
        }
    }
}
