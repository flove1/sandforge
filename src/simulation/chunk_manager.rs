use std::time::{SystemTime, UNIX_EPOCH};

use bevy::{
    prelude::*,
    tasks::ComputeTaskPool,
    utils::HashMap,
};
use bevy_math::{ivec2, vec2, IVec2, Rect, UVec2, Vec3Swizzles};
use itertools::{Either, Itertools};
use noise::SuperSimplex;

use crate::{
    constants::{CHUNK_SIZE, WORLD_HEIGHT, WORLD_WIDTH},
    generation::{
        chunk::ChunkGenerationEvent,
        tiles::TileGenerator,
    },
};

use super::{
    chunk::{ChunkApi, ChunkData, ChunkState},
    chunk_groups::ChunkGroup3x3,
    dirty_rect::{
        update_dirty_rects, update_dirty_rects_3x3, DirtyRects, RenderMessage, UpdateMessage,
    },
    materials::{update_gas, update_liquid, update_sand, MaterialInstance, PhysicsType},
    pixel::Pixel,
    Noise,
};

#[derive(Component)]
pub struct Chunks;

impl FromWorld for Noise {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(SuperSimplex::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .subsec_millis(),
        ))
    }
}

#[derive(Resource)]
pub struct ChunkManager {
    pub chunks: HashMap<IVec2, ChunkData>,
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

        self.chunks
            .get(&chunk_position)
            .map(|chunk| &chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_mut(&mut self, pos: IVec2) -> Result<&mut Pixel, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.chunks
            .get_mut(&chunk_position)
            .map(|chunk| &mut chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_material(&self, pos: IVec2) -> Result<&MaterialInstance, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.chunks
            .get(&chunk_position)
            .map(|chunk| &chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .map(|pixel| &pixel.material)
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_material_mut(&mut self, pos: IVec2) -> Result<&mut MaterialInstance, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.chunks
            .get_mut(&chunk_position)
            .map(|chunk| &mut chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .map(|pixel| &mut pixel.material)
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn set(&mut self, pos: IVec2, material: MaterialInstance) -> Result<(), String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if let Some(chunk) = self.chunks.get_mut(&chunk_position) {
            chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)] = Pixel::new(material, 0);
            Ok(())
        } else {
            Err("chunk is not loaded".to_string())
        }
    }

    pub fn set_with_condition<F: Fn(Pixel) -> bool>(
        &mut self,
        pos: IVec2,
        material: MaterialInstance,
        condition: F,
    ) -> Result<bool, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        match self.chunks.get_mut(&chunk_position) {
            Some(chunk) => {
                if condition(chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)].clone()) {
                    chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)] = Pixel::new(material, 0);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Err("chunk is not loaded".to_string()),
        }
    }

    // TODO: rewrite
    pub fn displace(&mut self, pos: IVec2, material: MaterialInstance) -> bool {
        let mut succeeded = false;

        let scan_w = 32;
        let scan_h = 32;
        let mut scan_pos = IVec2::ZERO;
        let mut scan_delta_pos = IVec2::new(0, -1);
        let scan_max_i = scan_w.max(scan_h) * scan_w.max(scan_h); // the max is pointless now but could change w or h later

        for _ in 0..scan_max_i {
            if (scan_pos.x >= -scan_w / 2)
                && (scan_pos.x <= scan_w / 2)
                && (scan_pos.y >= -scan_h / 2)
                && (scan_pos.y <= scan_h / 2)
            {
                if let Ok(true) = self.set_with_condition(
                    pos + IVec2::new(scan_pos.x, scan_pos.y),
                    material.clone(),
                    |pixel| (pixel.material.physics_type == PhysicsType::Air),
                ) {
                    succeeded = true;
                    break;
                }
            }

            // update scan coordinates

            if (scan_pos.x == scan_pos.y)
                || ((scan_pos.x < 0) && (scan_pos.x == -scan_pos.y))
                || ((scan_pos.x > 0) && (scan_pos.x == 1 - scan_pos.y))
            {
                let temp = scan_delta_pos.x;
                scan_delta_pos.x = -scan_delta_pos.y;
                scan_delta_pos.y = temp;
            }

            scan_pos.x += scan_delta_pos.x;
            scan_pos.y += scan_delta_pos.y;
        }

        succeeded
    }

    pub fn get_chunk(&self, chunk_position: &IVec2) -> Option<&ChunkData> {
        self.chunks.get(chunk_position)
    }

    pub fn get_chunk_mut(&mut self, chunk_position: &IVec2) -> Option<&mut ChunkData> {
        self.chunks.get_mut(chunk_position)
    }
}

pub fn manager_setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Chunks"),
        SpatialBundle::INHERITED_IDENTITY,
        Chunks,
    ));
}

pub fn update_loaded_chunks(
    mut ev_chunkgen: EventWriter<ChunkGenerationEvent>,
    mut chunk_manager: ResMut<ChunkManager>,
    camera_query: Query<&Transform, With<Camera>>,
    tile_generator: Res<TileGenerator>,
) {
    let camera_position = camera_query.single().translation.xy();

    let area = Rect::from_center_size(
        camera_position,
        vec2(WORLD_WIDTH as f32, WORLD_HEIGHT as f32) * 1.5,
    );

    // suspend chunks out of bounds
    chunk_manager
        .chunks.iter_mut()
        .filter(|(_, chunk)| chunk.state == ChunkState::Active)
        .for_each(|(position, chunk)| if !area.contains(position.as_vec2()) {
            chunk.state = ChunkState::Sleeping
        });

    for x in area.min.x.floor() as i32..area.max.x.ceil() as i32 {
        for y in area.min.y.floor() as i32..area.max.y.ceil() as i32 {
            let position = ivec2(x, y);

            match chunk_manager.chunks.get_mut(&position) {
                Some(chunk) => {
                    chunk.state = ChunkState::Active;
                    // if chunk.state = ChunkState::Sleeping {
                    // }
                },
                None => {
                    ev_chunkgen.send(ChunkGenerationEvent(
                        position.div_euclid(IVec2::ONE * tile_generator.scale) * tile_generator.scale,
                    ));
                },
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn chunks_update(
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
) {
    let DirtyRects {
        current: dirty_rects,
        new: new_dirty_rects,
        render: render_rects,
    } = &mut *dirty_rects_resource;

    let (update_send, update_recv) = async_channel::unbounded::<UpdateMessage>();
    let (render_send, render_recv) = async_channel::unbounded::<RenderMessage>();

    chunk_manager.clock = chunk_manager.clock.wrapping_add(1);

    ComputeTaskPool::get().scope(|scope| {
        scope.spawn(async move {
            while let Ok(update) = update_recv.recv().await {
                if update.awake_surrouding {
                    update_dirty_rects_3x3(
                        new_dirty_rects,
                        update.chunk_position,
                        update.cell_position,
                    );
                } else {
                    update_dirty_rects(new_dirty_rects, update.chunk_position, update.cell_position)
                }
            }
        });

        scope.spawn(async move {
            while let Ok(update) = render_recv.recv().await {
                update_dirty_rects(render_rects, update.chunk_position, update.cell_position);
            }
        });

        let update_send = &update_send;
        let render_send = &render_send;

        let mut groups: [Vec<IVec2>; 4] = [vec![], vec![], vec![], vec![]];

        chunk_manager
            .chunks
            .iter()
            .filter(|(_, chunk)| chunk.state == ChunkState::Active)
            .for_each(|(position, _)| {
                let index = (position.x.abs() % 2 + (position.y.abs() % 2) * 2) as usize;

                unsafe { groups.get_unchecked_mut(index) }.push(*position);
            });

        fastrand::shuffle(&mut groups);

        for group in groups {
            ComputeTaskPool::get().scope(|scope| {
                let clock = chunk_manager.clock;

                group
                    .into_iter()
                    .filter_map(|position| {
                        dirty_rects
                            .get(&position)
                            .cloned()
                            .map(|dirty_rect| (position, dirty_rect))
                    })
                    .filter_map(|(position, dirty_rect)| {
                        let center_ptr =
                            if let Some(chunk) = chunk_manager.chunks.get_mut(&position) {
                                chunk.pixels.as_mut_ptr()
                            } else {
                                return None;
                            };

                        let mut chunk_group = ChunkGroup3x3 {
                            size: CHUNK_SIZE,
                            center: center_ptr,
                            sides: [None; 4],
                            corners: [None; 4],
                        };

                        for (dx, dy) in (-1..=1).cartesian_product(-1..=1) {
                            match (dx, dy) {
                                (0, 0) => continue,
                                // UP and DOWN
                                (0, -1) | (0, 1) => {
                                    let Some(chunk) =
                                        chunk_manager.chunks.get_mut(&(position + ivec2(dx, dy)))
                                    else {
                                        continue;
                                    };

                                    if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                                        continue;
                                    }

                                    let start_ptr = chunk.pixels.as_mut_ptr();

                                    chunk_group.sides[if dy == -1 { 0 } else { 3 }] =
                                        Some(start_ptr);
                                }
                                //LEFT and RIGHT
                                (-1, 0) | (1, 0) => {
                                    let Some(chunk) =
                                        chunk_manager.chunks.get_mut(&(position + ivec2(dx, dy)))
                                    else {
                                        continue;
                                    };

                                    if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                                        continue;
                                    }

                                    let start_ptr = chunk.pixels.as_mut_ptr();

                                    chunk_group.sides[if dx == -1 { 1 } else { 2 }] =
                                        Some(start_ptr);
                                }
                                //CORNERS
                                (-1, -1) | (1, -1) | (-1, 1) | (1, 1) => {
                                    let Some(chunk) =
                                        chunk_manager.chunks.get_mut(&(position + ivec2(dx, dy)))
                                    else {
                                        continue;
                                    };

                                    if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                                        continue;
                                    }

                                    let start_ptr = chunk.pixels.as_mut_ptr();

                                    let corner_idx = match (dx, dy) {
                                        (1, 1) => 3,
                                        (-1, 1) => 2,
                                        (1, -1) => 1,
                                        (-1, -1) => 0,

                                        _ => unreachable!(),
                                    };

                                    chunk_group.corners[corner_idx] = Some(start_ptr);
                                }

                                _ => unreachable!(),
                            }
                        }

                        Some((position, dirty_rect, chunk_group))
                    })
                    .for_each(|(position, dirty_rect, mut chunk_group)| {
                        scope.spawn(async move {
                            let mut api = ChunkApi {
                                cell_position: ivec2(0, 0),
                                chunk_position: position,
                                chunk_group: &mut chunk_group,
                                update_send,
                                render_send,
                                clock,
                            };

                            let x_range = if fastrand::bool() {
                                Either::Left(dirty_rect.min.x as i32..dirty_rect.max.x as i32)
                            } else {
                                Either::Right(
                                    (dirty_rect.min.x as i32..dirty_rect.max.x as i32).rev(),
                                )
                            };

                            for x_cell in x_range {
                                for y_cell in dirty_rect.min.y as i32..dirty_rect.max.y as i32 {
                                    api.switch_position(ivec2(x_cell, y_cell));

                                    if api.get_counter(0, 0) == clock {
                                        api.keep_alive(0, 0);
                                        continue;
                                    }

                                    match api.get_physics_type(0, 0) {
                                        PhysicsType::Powder => {
                                            update_sand(&mut api);
                                        }
                                        PhysicsType::Gas => {
                                            update_gas(&mut api);
                                        }
                                        PhysicsType::Liquid(..) => {
                                            update_liquid(&mut api);
                                        }
                                        _ => {}
                                    }

                                    api.mark_updated();
                                }
                            }
                        })
                    })
            });
        }

        update_send.close();
        render_send.close();
    });

    let new_positions = dirty_rects_resource
        .new
        .keys()
        .copied()
        .collect::<Vec<IVec2>>();

    new_positions.iter().for_each(|position| {
        if !dirty_rects_resource.current.contains_key(position) {
            update_dirty_rects(&mut dirty_rects_resource.new, *position, UVec2::ZERO);
            update_dirty_rects(
                &mut dirty_rects_resource.new,
                *position,
                UVec2::ONE * (CHUNK_SIZE - 1) as u32,
            );
        }
    });

    dirty_rects_resource.current.clear();
    dirty_rects_resource.swap();
}
