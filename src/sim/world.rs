use std::{sync::Arc, collections::HashSet};

use dashmap::{DashMap, mapref::one::Ref};
use parking_lot::Mutex;
use rand::Rng;
use scoped_threadpool::Pool;

use crate::{vec2, vector::Vector2, constants::*, renderer::Vertex};

use super::{chunk::Chunk, elements::Element, helpers::get_cell_index, cell::{EMPTY_CELL, Cell}};

pub struct World {
    pub(super) chunks: DashMap<Vector2, Chunk>,
    pub(super) active_chunks: Mutex<HashSet<Vector2>>,
    pub(super) suspended_chunks: Mutex<HashSet<Vector2>>,
}

impl World {
    pub fn new() -> WorldApi {
        let world = Self {
                chunks: DashMap::new(),
                active_chunks: Mutex::new(HashSet::new()),
                suspended_chunks: Mutex::new(HashSet::new()),
        };
        
        for x in 0..WORLD_WIDTH {
            for y in 0..WORLD_HEIGHT {
                world.chunks.insert(vec2!(x, y), Chunk::new(vec2!(x, y)));
            }
        }

        WorldApi{
            chunk_manager: Arc::new(world), 
            clock: 0, 
            previous_update_ms: 0, pool: 
            Pool::new(4),
            pixel_count: 0,
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

        chunk.chunk_data.lock().set_cell(cell_position, cell);
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

        let mut data = chunk.chunk_data.lock();
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

        let data = chunk.chunk_data.lock();
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

        let mut data = chunk.chunk_data.lock();

        let old_cell = data.replace_cell(cell_position, cell);

        if old_cell.element != Element::Empty && cell.element == Element::Empty {
            chunk.cell_count.fetch_sub(1, std::sync::atomic::Ordering::Acquire);
        }
        else if old_cell.element == Element::Empty && cell.element != Element::Empty {
            chunk.cell_count.fetch_add(1, std::sync::atomic::Ordering::Acquire);
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
        
        let mut active_lock = self.active_chunks.lock();

        if active_lock.contains(&chunk_position) {
            return false;
        }

        active_lock.insert(*chunk_position);
        return true;
    }

    pub fn release_chunk(&self, chunk_position: &Vector2) {
        self.active_chunks.lock().remove(chunk_position);
        self.suspended_chunks.lock().insert(*chunk_position);
    }

    pub fn refresh_chunk_at_cell(&self, chunk_position: &Vector2, cell_position: &Vector2) {
        let chunk = {
            let result = self.get_chunk(chunk_position);

            if result.is_none() {
                return;
            }

            result.unwrap()
        };
        
        let mut active_lock = self.active_chunks.lock();

        if chunk.cell_count.load(std::sync::atomic::Ordering::Acquire) > 0 {
            if !active_lock.contains(&chunk_position) {
                active_lock.insert(*chunk_position);
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
    pool: Pool,
    pixel_count: u128,
}

impl WorldApi {
    pub fn needs_update(&mut self, dt: u128) -> bool {
        self.previous_update_ms += dt;
        self.previous_update_ms >= DELAY_MS
    }

    pub fn update(&mut self) -> (u128, u128, u128) {
        let mut chunks_count: u128 = 0;
        let mut updated_pixels_count: u128 = 0;
        while self.previous_update_ms >= DELAY_MS {
            self.pixel_count = 0;
            self.previous_update_ms -= DELAY_MS;
            let active_chunks = self.chunk_manager.active_chunks.lock().clone();
            chunks_count += active_chunks.len() as u128;
        
            let iter_range: Vec<i8> = if self.clock % 2 == 0 { (0..4).collect() } else { (0..4).rev().collect() };

            for iteration in iter_range {
                self.clock = self.clock.wrapping_add(1);
                self.pool.scoped(|s| {
                    let positions: Vec<&Vector2> = match iteration {
                        0 => active_chunks.iter().filter(|v| v.x%2 == 0 && v.y%2 == 0).collect(),
                        1 => active_chunks.iter().filter(|v| v.x%2 == 0 && v.y%2 == 1).collect(),
                        2 => active_chunks.iter().filter(|v| v.x%2 == 1 && v.y%2 == 0).collect(),
                        3 => active_chunks.iter().filter(|v| v.x%2 == 1 && v.y%2 == 1).collect(),
                        _ => panic!("must be in range of 0..4"),
                    };
    
                    s.execute(|| {
                        for position in  positions {
                            let chunk = self.chunk_manager.chunks.get(position).unwrap();
                            updated_pixels_count += chunk.update(self.chunk_manager.clone(), self.clock);
                            let pixel_count = chunk.cell_count.load(std::sync::atomic::Ordering::Relaxed);
                            
                            self.pixel_count += pixel_count as u128;

                            if pixel_count == 0 {
                                self.chunk_manager.release_chunk(position);
                            }
                        }
                    })
                });
            }
        }

        (chunks_count, updated_pixels_count, self.pixel_count)
        
    }

    pub fn place(&self, x: i64, y: i64, element: Element) {
        self.chunk_manager.place(x, y, element);
    }

    pub fn render(&self, frame: &mut [u8]) -> Vec<Vec<Vertex>> {
        let active_lock = self.chunk_manager.active_chunks.lock();
        let mut suspended_lock = self.chunk_manager.suspended_chunks.lock();
        
        let mut boundaries: Vec<Vec<Vertex>> = vec![];

        for chunk_position in active_lock.iter() {
            let chunk = self.chunk_manager.get_chunk(chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * (WORLD_WIDTH * CHUNK_SIZE);

            let chunk_data = chunk.chunk_data.lock();
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

                    if dirty_rect_x.contains(&x) && dirty_rect_y.contains(&y) {
                        frame[pixel_index as usize] = rgba[0].saturating_add(50);
                        frame[pixel_index as usize + 1] = rgba[1].saturating_add(25);
                        frame[pixel_index as usize + 2] = rgba[2].saturating_add(25);
                        frame[pixel_index as usize + 3] = rgba[3].saturating_add(25);
                    }
                    else {
                        frame[pixel_index as usize] = rgba[0];
                        frame[pixel_index as usize + 1] = rgba[1];
                        frame[pixel_index as usize + 2] = rgba[2];
                        frame[pixel_index as usize + 3] = rgba[3];
                    }
                }
            }

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

        for chunk_position in suspended_lock.clone() {
            suspended_lock.remove(&chunk_position);
            let chunk = self.chunk_manager.get_chunk(&chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * (WORLD_WIDTH * CHUNK_SIZE);

            let chunk_data = chunk.chunk_data.lock();

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