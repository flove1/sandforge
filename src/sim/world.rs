use std::{sync::Arc, ops::{AddAssign, SubAssign}, collections::BTreeMap};

use dashmap::{DashMap, mapref::one::Ref, DashSet};
use parking_lot::Mutex;
use rand::Rng;
use rayon::{ThreadPool, ThreadPoolBuilder};

use crate::{vec2, vector::Vector2, constants::*, renderer::Vertex};
use super::{chunk::Chunk, elements::Element, helpers::get_cell_index, cell::{EMPTY_CELL, Cell}};

pub struct World {
    pub(super) chunks: DashMap<Vector2, Chunk>,
    pub(super) active_chunks: DashSet<Vector2>,
    pub(super) suspended_chunks: DashSet<Vector2>,
}

impl World {
    pub fn new() -> WorldApi {
        let world = Self {
            chunks: DashMap::with_shard_amount(8),
            active_chunks: DashSet::new(),
            suspended_chunks: DashSet::new(),
        };
        
        for x in 0..WORLD_WIDTH {
            for y in 0..WORLD_HEIGHT {
                world.chunks.insert(vec2!(x, y), Chunk::new(vec2!(x, y)));
            }
        }

        WorldApi {
            chunk_manager: Arc::new(world), 
            clock: 0, 
            previous_update_ms: 0, 
            pool: ThreadPoolBuilder::new().num_threads(4).build().unwrap(),
        }
    }    

    pub(crate) fn place(&self, x: i64, y: i64, element: Element) {
        if x < 0 || y < 0 || x >= (WORLD_WIDTH * CHUNK_SIZE) || y >= (WORLD_HEIGHT * CHUNK_SIZE) {
            return;
        }

        let chunk_position = vec2!(x / CHUNK_SIZE, y / CHUNK_SIZE);
        if let Some(chunk) = self.get_chunk(&chunk_position) {
            self.activate_chunk(&chunk_position);
            chunk.place(x % CHUNK_SIZE, y % CHUNK_SIZE, element);
        }
    }

    pub(crate) fn update_cell(&self, chunk_position: Vector2, cell_position: Vector2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);

        let chunk = {
            let result = self.get_chunk(&(chunk_position + chunk_offset));

            if result.is_none() {
                return;
            }

            result.unwrap()
        };

        chunk.chunk_data.write().unwrap().set_cell(cell_position, cell);
    }    

    pub(crate) fn set_cell(&self, chunk_position: Vector2, cell_position: Vector2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);

        let chunk = {
            let result = self.get_chunk(&(chunk_position + chunk_offset));

            if result.is_none() {
                return;
            }

            result.unwrap()
        };

        let mut data = chunk.chunk_data.write().unwrap();
        if self.activate_chunk(&(chunk_position + chunk_offset)) {
            data.maximize_dirty_rect();
        }
        else {
            data.update_dirty_rect(&cell_position);
        }
        data.set_cell(cell_position, cell);
    }    

    pub(crate) fn get_cell(&self, chunk_position: Vector2, cell_position: Vector2) -> Cell {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);

        let chunk = {
            let result = self.get_chunk(&(chunk_position + chunk_offset));

            if result.is_none() {
                return EMPTY_CELL;
            }

            result.unwrap()
        };

        let data = chunk.chunk_data.read().unwrap();
        data.get_cell(cell_position)
    }    

    pub(crate) fn replace_cell(&self, chunk_position: Vector2, cell_offset: Vector2, cell: Cell) -> Cell {
        let (cell_position,chunk_offset ) = cell_offset.wrap(0, CHUNK_SIZE);

        if chunk_offset.is_zero() {
            panic!();
        }
        let chunk = {
            let result = self.get_chunk(&(chunk_position + chunk_offset));

            if result.is_none() {
                return EMPTY_CELL;
            }

            result.unwrap()
        };

        let mut data = chunk.chunk_data.write().unwrap();

        let old_cell = data.replace_cell(cell_position, cell);

        if old_cell.element != Element::Empty && cell.element == Element::Empty {
            chunk.cell_count.lock().unwrap().sub_assign(1);
        }
        else if old_cell.element == Element::Empty && cell.element != Element::Empty {
            chunk.cell_count.lock().unwrap().add_assign(1);
        }

        if self.activate_chunk(&(chunk_position + chunk_offset)) {
            data.maximize_dirty_rect();
        }
        else {
            data.update_dirty_rect(&cell_position);
        }

        old_cell
    }    

    pub(crate) fn get_chunk(&self, position: &Vector2) -> Option<Ref<Vector2, Chunk>> {
        self.chunks.get(position)
    }

    pub(crate) fn get_chunk_by_pixel(&self, x: i64, y: i64) -> Option<Ref<Vector2, Chunk>> {
        let chunk_x = x.div_euclid(CHUNK_SIZE);
        let chunk_y = y.div_euclid(CHUNK_SIZE);
        self.chunks.get(&vec2!(chunk_x, chunk_y))
    }

    /// Returns true if chunk was activated
    pub fn activate_chunk(&self, chunk_position: &Vector2) -> bool {
        if self.get_chunk(chunk_position).is_none() {
            return false;
        }
        
        if self.active_chunks.contains(&chunk_position) {
            return false;
        }

        self.active_chunks.insert(*chunk_position);
        return true;
    }

    pub fn release_chunk(&self, chunk_position: &Vector2) {
        self.active_chunks.remove(chunk_position);
        self.suspended_chunks.insert(*chunk_position);
    }

    pub fn refresh_chunk(&self, chunk_position: &Vector2, cell_position: &Vector2) {
        let chunk = {
            let result = self.get_chunk(chunk_position);

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
    chunk_manager: Arc<World>,
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
            let (updated_chunk_count, updated_pixels_count) = self.update_iteration();
            chunks_count += updated_chunk_count;
            pixels_count += updated_pixels_count;
        }

        (chunks_count, pixels_count)
    }

    pub fn update_iteration(&mut self) -> (u128, u128){
        self.clock = self.clock.wrapping_add(1);
        let updated_pixels = Mutex::new(0);
        
        let positions: Vec<Vector2> = self.chunk_manager.active_chunks.iter().map(|v| *v).collect();

        let chunk_count = positions.len();

        let mut groups: BTreeMap<i64, Vec<Vector2>> = BTreeMap::new();

        for position in positions {
            groups.entry(position.y).or_insert(Vec::with_capacity(WORLD_WIDTH as usize)).push(position);
        }

        for (_, group) in groups.iter_mut().rev() {
            #[cfg(feature = "multithreading")]  
            for iteration in 0..3 {
                group.sort_by(|v1, v2| {
                    if v1.x > v2.x {
                        std::cmp::Ordering::Greater
                    }
                    else {
                        std::cmp::Ordering::Less
                    }
                });
                self.pool.scope(|s| {
                    for position in group.iter().filter(|v| v.x % 3 == iteration % 3) {
                        s.spawn(|_| {
                            let chunk = self.chunk_manager.chunks.get(position).unwrap();
                            updated_pixels.lock().add_assign(chunk.update(self.chunk_manager.clone(), self.clock));
                            
                            if chunk.cell_count.lock().unwrap().eq(&0) {
                                self.chunk_manager.release_chunk(position);
                            }
                        });
                    }
                });
            }

            #[cfg(not(feature = "multithreading"))]
            for position in group.iter() {
                let chunk = self.chunk_manager.chunks.get(position).unwrap();
                updated_pixels.lock().add_assign(chunk.update(self.chunk_manager.clone(), self.clock));
                
                if chunk.cell_count.lock().unwrap().eq(&0) {
                    self.chunk_manager.release_chunk(position);
                }
            }
        }

        (chunk_count as u128, updated_pixels.into_inner())

        // let active_chunks = self.chunk_manager.active_chunks.clone();
        // for iteration in 0..4 {

        //     let positions: Vec<Vector2> = match iteration {
        //         0 => active_chunks.iter().filter(|v| v.x%2 == 0 && v.y%2 == 0).map(|v| *v).collect(),
        //         1 => active_chunks.iter().filter(|v| v.x%2 == 0 && v.y%2 == 1).map(|v| *v).collect(),
        //         2 => active_chunks.iter().filter(|v| v.x%2 == 1 && v.y%2 == 0).map(|v| *v).collect(),
        //         3 => active_chunks.iter().filter(|v| v.x%2 == 1 && v.y%2 == 1).map(|v| *v).collect(),
        //         _ => panic!("must be in range of 0..4"),
        //     };

        //     #[cfg(feature = "multithreading")]
        //     {    
        //         self.pool.scope(|s| {
        //             for position_chunked in  positions.chunks(16) {
        //                 s.spawn(|_| {
        //                     for position in position_chunked.iter() {
        //                         let chunk = self.chunk_manager.chunks.get(&position).unwrap();
        //                         updated_pixels.lock().add_assign(chunk.update(self.chunk_manager.clone(), self.clock));
                                
        //                         if chunk.cell_count.load(std::sync::atomic::Ordering::Relaxed) == 0 {
        //                             self.chunk_manager.release_chunk(&position);
        //                         }
        //                     }
        //                 });
        //             }
        //         });
        //     }

        //     #[cfg(not(feature = "multithreading"))]
        //     for position in positions.iter() {
        //         let chunk = self.chunk_manager.chunks.get(&position).unwrap();
        //         updated_pixels.lock().add_assign(chunk.update(self.chunk_manager.clone(), self.clock));
                
        //         if chunk.cell_count.load(std::sync::atomic::Ordering::Relaxed) == 0 {
        //             self.chunk_manager.release_chunk(&position);
        //         }
        //     }
        // }

        // (active_chunks.len() as u128, updated_pixels.into_inner())
    }

    pub fn place(&self, x: i64, y: i64, element: Element) {
        self.chunk_manager.place(x, y, element);
    }

    pub fn render(&self, frame: &mut [u8]) -> Vec<Vec<Vertex>> {
        let mut boundaries: Vec<Vec<Vertex>> = vec![];

        for chunk_position in self.chunk_manager.active_chunks.clone() {
            let chunk = self.chunk_manager.get_chunk(&chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * (WORLD_WIDTH * CHUNK_SIZE);

            let chunk_data = chunk.chunk_data.read().unwrap();

            #[cfg(feature = "dirty_chunk_rendering")]
            let (dirty_rect_x, dirty_rect_y) = chunk_data.dirty_rect.get_ranges_render();

            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    let pixel_index = ((y_offset + y * (WORLD_WIDTH * CHUNK_SIZE)) + x_offset + x) * 4;
                    let cell = chunk_data.cells[get_cell_index(x as i64, y as i64)];
                    let offset = rand::thread_rng().gen_range(0..10);

                    let rgba = match cell.element {
                        Element::Empty => [0x00, 0x00, 0x00, 0xff],
                        Element::Stone => [0x77, 0x77, 0x77, 0xff],
                        Element::Sand => [0xf2_u8.saturating_add(cell.ra), 0xf1_u8.saturating_add(cell.ra), 0xa3_u8.saturating_add(cell.ra), 0xff],
                        Element::Water => [0x47 + offset, 0x7C + offset, 0xB8 + offset, 0xff],
                        Element::GlowingSand => [0xe8, 0x6a, 0x17, 0xff],
                        Element::Wood => [0x6a_u8.saturating_add(cell.ra), 0x4b_u8.saturating_add(cell.ra), 0x35_u8.saturating_add(cell.ra), 0xff],
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

            let chunk_boundaries = chunk_data.objects.clone();

            // Convert from chunk coordinates to screen coordinates
            for boundary in chunk_boundaries.iter() {
                boundaries.push(boundary.iter().map(|point| Vertex {
                    position: [
                        (((point.0 as f32 + (chunk_position.x * CHUNK_SIZE) as f32) / (CHUNK_SIZE * WORLD_WIDTH) as f32) - 0.5) * 2.0, 
                        ((-(point.1 as f32 + (chunk_position.y * CHUNK_SIZE) as f32) / (CHUNK_SIZE * WORLD_HEIGHT) as f32) + 0.5) * 2.0
                    ]
                })
                .collect());
            }
        }

        for chunk_position in self.chunk_manager.suspended_chunks.clone() {
            self.chunk_manager.suspended_chunks.remove(&chunk_position);
            let chunk = self.chunk_manager.get_chunk(&chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * (WORLD_WIDTH * CHUNK_SIZE);

            let chunk_data = chunk.chunk_data.read().unwrap();

            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    let pixel_index = ((y_offset + y * (WORLD_WIDTH * CHUNK_SIZE)) + x + x_offset) * 4;
                    let cell = chunk_data.cells[get_cell_index(x as i64, y as i64)];
                    let offset = rand::thread_rng().gen_range(0..25);
                    let rgba = match cell.element {
                        Element::Empty => [0x00, 0x00, 0x00, 0xff],
                        Element::Stone => [0x77, 0x77, 0x77, 0xff],
                        Element::Sand => [0xf2_u8.saturating_add(cell.ra), 0xf1_u8.saturating_add(cell.ra), 0xa3_u8.saturating_add(cell.ra), 0xff],
                        Element::Water => [0x47 + offset, 0x7C + offset, 0xB8 + offset, 0xff],
                        Element::GlowingSand => [0xe8, 0x6a, 0x17, 0xff],
                        Element::Wood => [0x6a_u8.saturating_add(cell.ra), 0x4b_u8.saturating_add(cell.ra), 0x35_u8.saturating_add(cell.ra), 0xff],
                    };

                    frame[pixel_index as usize] = rgba[0];
                    frame[pixel_index as usize + 1] = rgba[1];
                    frame[pixel_index as usize + 2] = rgba[2];
                    frame[pixel_index as usize + 3] = rgba[3];
                }
            }
        }

        boundaries
    }
}