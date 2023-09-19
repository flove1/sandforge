use std::{sync::{Arc, Mutex}, ops::{AddAssign, SubAssign}, collections::{BTreeMap, BTreeSet}, default};

use ahash::{RandomState, HashSet, HashMap};
use dashmap::{DashMap, DashSet};
use rand::Rng;
use rapier2d::na::{Matrix2, Vector2, Translation2};
use threadpool::ThreadPool;

use crate::{constants::*, renderer::Vertex, pos2, vector::Pos2};
use super::{chunk::Chunk, helpers::get_cell_index, cell::{EMPTY_CELL, Cell, SimulationType}, physics::Physics, elements::MatterType};

pub struct World {
    pub(super) chunks: DashMap<Pos2, Chunk, RandomState>,
    pub(super) active_chunks: DashSet<Pos2>,
    pub(super) suspended_chunks: DashSet<Pos2>,
    pub(super) physics_engine: Mutex<Physics>,
}

impl World {
    pub fn new() -> WorldApi {
        let world = Self {
            chunks: DashMap::with_hasher_and_shard_amount(RandomState::new(), 8),
            active_chunks: DashSet::new(),
            suspended_chunks: DashSet::new(),
            physics_engine: Mutex::new(Physics::new())
        };

        {
            let mut engine = world.physics_engine.lock().unwrap();
            
            for x in 0..WORLD_WIDTH {
                for y in 0..WORLD_HEIGHT {
                    let handler = engine.new_empty_static_object(((x as f32 + 0.5) * CHUNK_SIZE as f32) as f32 / PHYSICS_TO_WORLD, ((y as f32 + 0.5) * CHUNK_SIZE as f32) as f32 / PHYSICS_TO_WORLD);
    
                    world.chunks.insert(
                        pos2!(x, y), 
                        Chunk::new(pos2!(x, y), handler)
                    );
                }
            }
        }

        WorldApi {
            chunk_manager: Arc::new(world), 
            clock: 0, 
            previous_update_ms: 0, 
            pool: ThreadPool::new(4),
        }
    }

    pub(crate) fn place(&self, x: i32, y: i32, element: &MatterType) {
        if x < 0 || y < 0 || x >= (WORLD_WIDTH * CHUNK_SIZE) || y >= (WORLD_HEIGHT * CHUNK_SIZE) {
            return;
        }

        let chunk_position = pos2!(x / CHUNK_SIZE, y / CHUNK_SIZE);
        if let Some(chunk) = self.chunks.get(&chunk_position) {
            chunk.place(x % CHUNK_SIZE, y % CHUNK_SIZE, element);
            self.active_chunks.insert(chunk_position);
        }
    }

    pub fn place_batch(&self, positions: HashSet<(i32, i32)>, element: &MatterType) {
        let mut groups_by_chunks: HashMap<Pos2, Vec<(i32, i32)>> = HashMap::default();
        
        positions.into_iter()
            .filter(|pos| {
                pos.0 >= 0 && pos.1 >= 0 && pos.0 < (WORLD_WIDTH * CHUNK_SIZE) && pos.1 < (WORLD_HEIGHT * CHUNK_SIZE)
            })
            .for_each(|pos| {
                groups_by_chunks
                    .entry(pos2!(pos.0 / CHUNK_SIZE, pos.1 / CHUNK_SIZE))
                    .or_insert(vec![])
                    .push((pos.0 % CHUNK_SIZE, pos.1 % CHUNK_SIZE));
            });

        groups_by_chunks.into_iter()
            .for_each(|(chunk_position, cells)| {
                if let Some(chunk) = self.chunks.get(&chunk_position) {
                    chunk.place_batch(cells, &element);
                    self.active_chunks.insert(chunk_position);
                };
            });   
    }

    pub fn place_object(&self, positions: HashSet<(i32, i32)>, element: &MatterType, static_flag: bool) {
        self.physics_engine.lock().unwrap().new_object(positions, element, static_flag);
    }

    pub fn modify_object(&self, object_id: usize, cell_index: usize, cell: Cell) {
        self.physics_engine.lock().unwrap().modify_object(object_id, cell_index, cell);
    }

    pub(crate) fn update_cell(&self, chunk_position: Pos2, cell_position: Pos2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return;
        }

        let chunk = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk_data = chunk.chunk_data.write().unwrap();

        chunk_data.set_cell(cell_position, cell);
    }    

    pub(crate) fn set_cell(&self, chunk_position: Pos2, cell_position: Pos2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return;
        }

        let is_chunk_activated = self.active_chunks.insert(new_chunk_position);        
        
        let chunk = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk_data = chunk.chunk_data.write().unwrap();

        if is_chunk_activated {
            chunk_data.maximize_dirty_rect();
        }
        else {
            chunk_data.update_dirty_rect(&cell_position);
        }

        chunk_data.set_cell(cell_position, cell);
    }    

    pub(crate) fn get_cell(&self, chunk_position: Pos2, cell_position: Pos2) -> Cell {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return EMPTY_CELL.clone();
        }

        self.chunks.get(&new_chunk_position).unwrap()
            .chunk_data.read().unwrap().get_cell(cell_position).clone()
    }
        

    pub(crate) fn match_cell(&self, chunk_position: Pos2, cell_position: Pos2, element: &MatterType) -> bool {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return true;
        }

        self.chunks.get(&new_chunk_position).unwrap()
            .chunk_data.read().unwrap().match_cell(cell_position, &element)
    }

    pub(crate) fn replace_cell(&self, chunk_position: Pos2, cell_offset: Pos2, cell: Cell) -> Cell {
        let (cell_position,chunk_offset ) = cell_offset.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;
        
        if chunk_offset.is_zero() {
            panic!();
        }

        if !self.chunks.contains_key(&new_chunk_position) {
            return EMPTY_CELL.clone();
        }

        let is_chunk_activated = self.active_chunks.insert(new_chunk_position);

        let chunk = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk_data = chunk.chunk_data.write().unwrap();
    
        if is_chunk_activated {
            chunk_data.maximize_dirty_rect();
        }    
        else {
            chunk_data.update_dirty_rect(&cell_position);
        }
        
        let old_cell = chunk_data.replace_cell(cell_position, cell.clone());
        if old_cell.element != MatterType::Empty && cell.element == MatterType::Empty {
            chunk.cell_count.lock().unwrap().sub_assign(1);
        }
        else if old_cell.element == MatterType::Empty && cell.element != MatterType::Empty {
            chunk.cell_count.lock().unwrap().add_assign(1);
        }
        
        old_cell   
    }

    pub fn move_particle(&self, chunk_position: Pos2, mut particle: Cell) {
        match &mut particle.simulation {
            SimulationType::Particle { x, y, .. } => {
                let ix = x.floor() as i32;
                let iy = y.floor() as i32;
                let cell_ix = (*x * CHUNK_SIZE as f32).floor() as i32;
                let cell_iy = (*y * CHUNK_SIZE as f32).floor() as i32;

                *x -= ix as f32;
                *y -= iy as f32;

                let cell_position = pos2!(cell_ix, cell_iy);
                let new_chunk_position = chunk_position + pos2!(ix, iy);

                if let Some(chunk) = self.chunks.get(&new_chunk_position) { 
                    let mut chunk_data = chunk.chunk_data.write().unwrap();
    
                    let is_chunk_activated = self.active_chunks.insert(new_chunk_position);
        
                    if is_chunk_activated {
                        chunk_data.maximize_dirty_rect();
                    }    
                    else {
                        chunk_data.update_dirty_rect(&cell_position);
                    }
    
                    chunk.particles.lock().unwrap().push(particle);
                }
            },
            _ => panic!()
        } 
    }

    pub fn convert_objects_to_cells(&self) -> Vec<(Translation2<f32>, HashMap<Pos2, Cell>)> {
        let engine = self.physics_engine.lock().unwrap();

        engine.objects.iter()
            .map(|object| 
                (
                    object, 
                    engine.rigid_body_set.get(object.rb_handle), 
                    engine.collider_set.get(object.collider_handle)
                )
            )
            .filter(|(_, rb, collider)| rb.is_some() && collider.is_some())
            .map(|(object, rb, collider)| (object, rb.unwrap(), collider.unwrap()))
            .map(|(object, rb, _)| {
                let mut cells = HashMap::default();
                let rotation = rb.rotation();
                
                let rotation_matrix = Matrix2::new(
                    rotation.angle().cos(), 
                    -rotation.angle().sin(), 
                    rotation.angle().sin(), 
                    rotation.angle().cos()
                );

                let offset = rotation_matrix * Vector2::new(- (object.object_size as f32) / 2.0 / PHYSICS_TO_WORLD , - (object.object_size as f32) / 2.0 / PHYSICS_TO_WORLD);
                let center = rb.position().translation.vector + offset;

                for x in 0..object.object_size as i32 {
                    for y in 0..object.object_size as i32 {
                        let point = Vector2::new(x as f32, y as f32);
                        let rotated_point = rotation_matrix * point;
                        let rotated_x = (rotated_point.x as f32 / PHYSICS_TO_WORLD + center.x) * PHYSICS_TO_WORLD;
                        let rotated_y = (rotated_point.y as f32 / PHYSICS_TO_WORLD + center.y) * PHYSICS_TO_WORLD;

                        let index = (y * object.object_size as i32 + x) as usize;
                        if object.matrix[index].element != MatterType::Empty {
                            cells.insert(pos2!(rotated_x.round() as i32, rotated_y.round() as i32), object.matrix[index].clone());
                        }
                    }
                }

                (rb.position().translation, cells)
            })
            .collect::<Vec<(Translation2<f32>, HashMap<Pos2, Cell>)>>()
    }

    pub fn place_objects(&self) {
        let objects = self.convert_objects_to_cells();
        let mut groups_by_chunks: Vec<HashMap<Pos2, Vec<((i32, i32), Cell)>>> = vec![HashMap::default(); objects.len()];
        
        objects.iter().enumerate()
            .for_each(|(index, (_, cells))| {
                cells.iter()
                    .filter(|(pos, _)| {
                        pos.x >= 0 && pos.y >= 0 && pos.x < WORLD_WIDTH * CHUNK_SIZE && pos.y < WORLD_HEIGHT * CHUNK_SIZE
                    })
                    .for_each(|(pos, cell)| {
                        groups_by_chunks[index]
                            .entry(pos2!(pos.x / CHUNK_SIZE, pos.y / CHUNK_SIZE))
                            .or_insert(vec![])
                            .push(((pos.x % CHUNK_SIZE, pos.y % CHUNK_SIZE), cell.clone()));
                    });
            });

        
        groups_by_chunks.into_iter().enumerate()
            .for_each(|(index, chunks)| {
                chunks.into_iter()
                    .for_each(|(chunk_position, cells)| {
                        if let Some(chunk) = self.chunks.get(&chunk_position) {
                            chunk.place_object(
                                cells, 
                                objects[index].0.x,
                                objects[index].0.y,
                            );
                            self.active_chunks.insert(chunk_position);
                        };
                    });   
            });
            
    }

    pub fn remove_objects(&self) {
        let objects = self.convert_objects_to_cells();
        let mut groups_by_chunks: Vec<HashMap<Pos2, Vec<(i32, i32)>>> = vec![HashMap::default(); objects.len()];


        objects.iter().enumerate()
            .for_each(|(index, (_, cells))| {
                cells.iter()
                    .filter(|(pos, _)| {
                        pos.x >= 0 && pos.y >= 0 && pos.x < WORLD_WIDTH * CHUNK_SIZE && pos.y < WORLD_HEIGHT * CHUNK_SIZE
                    })
                    .for_each(|(pos, _)| {
                        groups_by_chunks[index]
                            .entry(pos2!(pos.x / CHUNK_SIZE, pos.y / CHUNK_SIZE))
                            .or_insert(vec![])
                            .push((pos.x % CHUNK_SIZE, pos.y % CHUNK_SIZE));
                    });
            });

        groups_by_chunks.into_iter().enumerate()
        .for_each(|(index, chunks)| {
            chunks.into_iter()
                .for_each(|(chunk_position, cells)| {
                    if let Some(chunk) = self.chunks.get(&chunk_position) {
                        chunk.remove_object(
                            cells, 
                            objects[index].0.x,
                            objects[index].0.y,
                        );
                        self.active_chunks.insert(chunk_position);
                    };
                });   
        });
    }

    pub fn release_chunk(&self, chunk_position: &Pos2) {
        self.active_chunks.remove(chunk_position);
        self.suspended_chunks.insert(*chunk_position);
    }

    pub fn refresh_chunk(&self, chunk_position: &Pos2, cell_position: &Pos2) {
        let chunk = {
            let result = self.chunks.get(chunk_position);

            if result.is_none() {
                return;
            }

            result.unwrap()
        };
        
        if chunk.cell_count.lock().unwrap().gt(&0) {
            if !self.active_chunks.contains(&chunk_position) {
                self.active_chunks.insert(*chunk_position);
                chunk.maximize_dirty_rect();
            }
            else {
                chunk.update_dirty_rect(cell_position);
            }
        }
    }
}

pub struct WorldApi {
    pub chunk_manager: Arc<World>,
    previous_update_ms: u128,
    clock: u8,
    pool: ThreadPool,
}

impl WorldApi {
    pub fn needs_update(&mut self, dt: u128) -> bool {
        self.previous_update_ms += dt;
        self.previous_update_ms >= DELAY_MS
    }

    pub fn update(&mut self) -> (u128, u128) {
        let mut chunks_count = 0;
        let mut pixels_count = 0;
        while self.previous_update_ms >= DELAY_MS {
            self.previous_update_ms -= DELAY_MS;
            if self.previous_update_ms > DELAY_MS * 10 {
                self.previous_update_ms = 0;
                break;
            }
            let (updated_chunk_count, updated_pixels_count) = self.update_iteration();
            chunks_count += updated_chunk_count;
            pixels_count += updated_pixels_count;
        }

        (chunks_count, pixels_count)
    }

    pub fn update_iteration(&mut self) -> (u128, u128){
        self.clock = self.clock.wrapping_add(1);

        let updated_pixels = Arc::new(Mutex::new(0));
        let positions: Vec<Pos2> = self.chunk_manager.active_chunks.iter().map(|v| *v).collect();

        let mut groups: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();

        for position in positions.iter() {
            groups.entry(position.x).or_insert(BTreeSet::new()).insert(position.y);
        }

        {
            let mut engine = self.chunk_manager.physics_engine.lock().unwrap();

            #[cfg(not(feature = "multithreading"))]
            for (x, group) in groups.iter().rev() {   
                for y in group.iter().rev() {
                    let position = &pos2!(*x, *y);
                    let chunk = self.chunk_manager.chunks.get(position).unwrap();
                    updated_pixels.lock().unwrap().add_assign(chunk.update(self.chunk_manager.clone(), self.clock));
                }
            }
        }

        self.chunk_manager.remove_objects();
        self.chunk_manager.physics_engine.lock().unwrap().step();
        self.chunk_manager.place_objects();

        {
            let mut engine = self.chunk_manager.physics_engine.lock().unwrap();
    
            for (x, group) in groups.iter().rev() {   
                for y in group.iter().rev() {
                    let position = &pos2!(*x, *y);
                    let chunk = self.chunk_manager.chunks.get(position).unwrap();
                    let rb_handle = chunk.chunk_data.read().unwrap().rb_handle.unwrap();
    
                    engine.remove_collider_from_object(rb_handle);
    
                    if chunk.cell_count.lock().unwrap().eq(&0) {
                        self.chunk_manager.release_chunk(position);
                    }
                    else {
                        chunk.create_colliders();
                        engine.replace_colliders_to_static_body(rb_handle, &chunk.chunk_data.read().unwrap().colliders);
                    }
                }
            }
        }

        let lock = Arc::try_unwrap(updated_pixels).expect("Lock still has multiple owners");
        (positions.len() as u128, lock.into_inner().unwrap())
    }

    pub fn place(&self, x: i32, y: i32, element: &MatterType) {
        self.chunk_manager.place(x, y, element);
    }

    pub fn place_batch(&self, positions: HashSet<(i32, i32)>, element: &MatterType) {
        self.chunk_manager.place_batch(positions, element);
    }

    pub fn place_object(&self, positions: HashSet<(i32, i32)>, element: &MatterType, static_flag: bool) {
        self.chunk_manager.place_object(positions, element, static_flag);
    }

    pub fn render(&self, frame: &mut [u8]) -> Vec<Vec<Vertex>> {
        let colliders: Vec<Vec<Vertex>> = vec![];

        for chunk_position in self.chunk_manager.active_chunks.clone() {
            let chunk = self.chunk_manager.chunks.get(&chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * (WORLD_WIDTH * CHUNK_SIZE);

            let chunk_data = chunk.chunk_data.read().unwrap();

            #[cfg(feature = "dirty_chunk_rendering")]
            let (dirty_rect_x, dirty_rect_y) = chunk_data.dirty_rect.get_ranges_render();

            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    let pixel_index = ((y_offset + y * (WORLD_WIDTH * CHUNK_SIZE)) + x_offset + x) * 4;
                    let cell = &chunk_data.cells[get_cell_index(x as i32, y as i32)];
                    let offset = rand::thread_rng().gen_range(0..10);
                
                    let rgba = match cell.element {
                        MatterType::Empty => [0, 0, 0, 255],
                        MatterType::Static { color, .. } | MatterType::Powder { color, .. } => [
                            color[0].saturating_add(cell.ra),
                            color[1].saturating_add(cell.ra),
                            color[2].saturating_add(cell.ra),
                            color[3].saturating_add(cell.ra),
                        ],
                        MatterType::Liquid { color, .. } => [
                            color[0].saturating_add(offset),
                            color[1].saturating_add(offset), 
                            color[2].saturating_add(offset), 
                            color[3].saturating_add(offset), 
                        ],
                        MatterType::Gas { color, .. } => [
                            color[0].saturating_add(offset * 2),
                            color[1].saturating_add(offset * 2), 
                            color[2].saturating_add(offset * 2), 
                            color[3].saturating_add(offset * 2),
                        ],
                        // MatterType::Coal | MatterType::Sand | MatterType::Wood | MatterType::Dirt => {
                        //     for color in rgba.iter_mut() {
                        //         *color = color.saturating_add(cell.ra);
                        //     }
                        // }
                        
                        // MatterType::Water => {
                        //     for color in rgba.iter_mut() {
                        //         *color = color.saturating_add(offset);
                        //     }
                        // }
                        
                        // MatterType::Gas => {
                        //     for color in rgba.iter_mut() {
                        //         *color = color.saturating_add(offset * 3);
                        //     }
                        // }
                        // _ => {}
                    };

                    frame[pixel_index as usize] = rgba[0];
                    frame[pixel_index as usize + 1] = rgba[1];
                    frame[pixel_index as usize + 2] = rgba[2];
                    frame[pixel_index as usize + 3] = rgba[3];

                    #[cfg(feature = "dirty_chunk_rendering")]
                    if dirty_rect_x.contains(&x) && dirty_rect_y.contains(&y) {
                        frame[pixel_index as usize] = frame[pixel_index as usize].saturating_add(50);
                        frame[pixel_index as usize + 1] = frame[pixel_index as usize + 1].saturating_add(25);
                        frame[pixel_index as usize + 2] = frame[pixel_index as usize + 2].saturating_add(25);
                        frame[pixel_index as usize + 3] = frame[pixel_index as usize + 3].saturating_add(25);
                    }
                }
            }

            chunk.particles.lock().unwrap().iter()
                .for_each(|particle| {
                    match particle.simulation {
                        SimulationType::Particle { x, y, .. } => {
                            let x = (x * CHUNK_SIZE as f32).floor() as i32;
                            let y = (y * CHUNK_SIZE as f32).floor() as i32;
                            let pixel_index = ((y_offset + y * (WORLD_WIDTH * CHUNK_SIZE)) + x_offset + x) * 4;

                            let offset = rand::thread_rng().gen_range(0..10);
                
                            let rgba = match particle.element {
                                MatterType::Empty => [0, 0, 0, 255],
                                MatterType::Static { color, .. } | MatterType::Powder { color, .. } => [
                                    color[0].saturating_add(particle.ra),
                                    color[1].saturating_add(particle.ra),
                                    color[2].saturating_add(particle.ra),
                                    color[3].saturating_add(particle.ra),
                                ],
                                MatterType::Liquid { color, .. } => [
                                    color[0].saturating_add(offset),
                                    color[1].saturating_add(offset), 
                                    color[2].saturating_add(offset), 
                                    color[3].saturating_add(offset), 
                                ],
                                MatterType::Gas { color, .. } => [
                                    color[0].saturating_add(offset * 2),
                                    color[1].saturating_add(offset * 2), 
                                    color[2].saturating_add(offset * 2), 
                                    color[3].saturating_add(offset * 2),
                                ],
                            };

                            frame[pixel_index as usize] = rgba[0];
                            frame[pixel_index as usize + 1] = rgba[1];
                            frame[pixel_index as usize + 2] = rgba[2];
                            frame[pixel_index as usize + 3] = rgba[3];

                            #[cfg(feature = "dirty_chunk_rendering")]
                            if dirty_rect_x.contains(&x) && dirty_rect_y.contains(&y) {
                                frame[pixel_index as usize] = frame[pixel_index as usize].saturating_add(50);
                                frame[pixel_index as usize + 1] = frame[pixel_index as usize + 1].saturating_add(25);
                                frame[pixel_index as usize + 2] = frame[pixel_index as usize + 2].saturating_add(25);
                                frame[pixel_index as usize + 3] = frame[pixel_index as usize + 3].saturating_add(25);
                            }
                        },
                        _ => panic!()
                    }
                });




            #[cfg(feature = "chunk_border_rendering")]
            for x in 0..CHUNK_SIZE {
                let start_offset = ((x + x_offset + y_offset)*4) as usize;
                let end_offset = (((CHUNK_SIZE-1) * (WORLD_WIDTH * CHUNK_SIZE) + x + x_offset + y_offset) * 4) as usize;
                frame[start_offset as usize] = frame[start_offset as usize].saturating_add(25);
                frame[start_offset+1 as usize] = frame[start_offset+1 as usize].saturating_add(25);
                frame[start_offset+2 as usize] = frame[start_offset+2 as usize].saturating_add(25);
                frame[start_offset+3 as usize] = frame[start_offset+3 as usize].saturating_add(25);

                frame[end_offset as usize] = frame[end_offset as usize].saturating_add(25);
                frame[end_offset+1 as usize] = frame[end_offset+1 as usize].saturating_add(25);
                frame[end_offset+2 as usize] = frame[end_offset+2 as usize].saturating_add(25);
                frame[end_offset+3 as usize] = frame[end_offset+3 as usize].saturating_add(25);
            }

            #[cfg(feature = "chunk_border_rendering")]
            for y in 0..CHUNK_SIZE {
                let start_offset = ((y * (WORLD_WIDTH * CHUNK_SIZE) + x_offset + y_offset)*4) as usize;
                let end_offset = ((y * (WORLD_WIDTH * CHUNK_SIZE) + CHUNK_SIZE - 1 + x_offset + y_offset)*4) as usize;
                frame[start_offset as usize] = frame[start_offset as usize].saturating_add(25);
                frame[start_offset+1 as usize] = frame[start_offset+1 as usize].saturating_add(25);
                frame[start_offset+2 as usize] = frame[start_offset+2 as usize].saturating_add(25);
                frame[start_offset+3 as usize] = frame[start_offset+3 as usize].saturating_add(25);

                frame[end_offset as usize] = frame[end_offset as usize].saturating_add(25);
                frame[end_offset+1 as usize] = frame[end_offset+1 as usize].saturating_add(25);
                frame[end_offset+2 as usize] = frame[end_offset+2 as usize].saturating_add(25);
                frame[end_offset+3 as usize] = frame[end_offset+3 as usize].saturating_add(25);
            }

            // Convert from chunk coordinates to screen coordinates
            // chunk_data.colliders.iter()
            //     .map(|collider| collider.shape().as_compound().unwrap().shapes())
            //     .for_each(|shapes| {
            //         shapes.iter()
            //             .map(|shape| shape.1.as_triangle().unwrap().vertices())
            //             .for_each(|vertices| {
            //                 colliders.push(
            //                     vertices.iter().map(|vertex|
            //                         Vertex {
            //                             position: [
            //                                 ((((vertex.x + chunk_position.x as f32) * CHUNK_SIZE as f32) / (CHUNK_SIZE * WORLD_WIDTH) as f32) - 0.5) * 2.0, 
            //                                 ((-((vertex.y + chunk_position.y as f32) * CHUNK_SIZE as f32) / (CHUNK_SIZE * WORLD_HEIGHT) as f32) + 0.5) * 2.0
            //                             ]
            //                         }
            //                     ).collect()
            //                 );
            //             })
            //     });
        }

        for chunk_position in self.chunk_manager.suspended_chunks.clone() {
            self.chunk_manager.suspended_chunks.remove(&chunk_position);
            let chunk = self.chunk_manager.chunks.get(&chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * (WORLD_WIDTH * CHUNK_SIZE);

            let chunk_data = chunk.chunk_data.read().unwrap();

            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    let pixel_index = ((y_offset + y * (WORLD_WIDTH * CHUNK_SIZE)) + x + x_offset) * 4;
                    let cell = &chunk_data.cells[get_cell_index(x as i32, y as i32)];
                    let offset = rand::thread_rng().gen_range(0..25);

                    let rgba = match cell.element {
                        MatterType::Empty => [0, 0, 0, 255],
                        MatterType::Static { color, .. } | MatterType::Powder { color, .. } => [
                            color[0].saturating_add(cell.ra),
                            color[1].saturating_add(cell.ra),
                            color[2].saturating_add(cell.ra),
                            color[3].saturating_add(cell.ra),
                        ],
                        MatterType::Liquid { color, .. } => [
                            color[0].saturating_add(offset),
                            color[1].saturating_add(offset),
                            color[2].saturating_add(offset),
                            color[3].saturating_add(offset),
                        ],
                        MatterType::Gas { color, .. } => [
                            color[0].saturating_add(offset * 2),
                            color[1].saturating_add(offset * 2),
                            color[2].saturating_add(offset * 2),
                            color[3].saturating_add(offset * 2),
                        ],
                    };

                    frame[pixel_index as usize] = rgba[0];
                    frame[pixel_index as usize + 1] = rgba[1];
                    frame[pixel_index as usize + 2] = rgba[2];
                    frame[pixel_index as usize + 3] = rgba[3];
                }
            }
        }

        colliders
    }
}