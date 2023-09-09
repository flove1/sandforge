use std::{sync::{Arc, Mutex}, ops::{AddAssign, SubAssign}, collections::{BTreeMap, BTreeSet}};

use ahash::RandomState;
use dashmap::{DashMap, DashSet};
use rand::Rng;
use threadpool::ThreadPool;

use crate::{vec2, vector::Vector2, constants::*, renderer::Vertex};
use super::{chunk::Chunk, elements::Element, helpers::get_cell_index, cell::{EMPTY_CELL, Cell}};

pub struct World {
    pub(super) chunks: DashMap<Vector2, Chunk, RandomState>,
    pub(super) active_chunks: DashSet<Vector2>,
    pub(super) suspended_chunks: DashSet<Vector2>,
}

impl World {
    pub fn new() -> WorldApi {
        let world = Self {
            chunks: DashMap::with_hasher_and_shard_amount(RandomState::new(), 8),
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
            pool: ThreadPool::new(4),
        }
    }

    pub(crate) fn place(&self, x: i64, y: i64, element: Element) {
        if x < 0 || y < 0 || x >= (WORLD_WIDTH * CHUNK_SIZE) || y >= (WORLD_HEIGHT * CHUNK_SIZE) {
            return;
        }

        let chunk_position = vec2!(x / CHUNK_SIZE, y / CHUNK_SIZE);
        if let Some(chunk) = self.chunks.get(&chunk_position) {
            chunk.place(x % CHUNK_SIZE, y % CHUNK_SIZE, element);
            self.active_chunks.insert(chunk_position);
        }
    }

    pub(crate) fn update_cell(&self, chunk_position: Vector2, cell_position: Vector2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return;
        }

        let chunk = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk_data = chunk.chunk_data.write().unwrap();

        chunk_data.set_cell(cell_position, cell);
    }    

    pub(crate) fn set_cell(&self, chunk_position: Vector2, cell_position: Vector2, cell: Cell) {
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

    pub(crate) fn get_cell(&self, chunk_position: Vector2, cell_position: Vector2) -> Cell {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return EMPTY_CELL;
        }

        self.chunks.get(&new_chunk_position).unwrap()
            .chunk_data.read().unwrap().get_cell(cell_position)
    }    

    pub(crate) fn replace_cell(&self, chunk_position: Vector2, cell_offset: Vector2, cell: Cell) -> Cell {
        let (cell_position,chunk_offset ) = cell_offset.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;
        
        if chunk_offset.is_zero() {
            panic!();
        }

        if !self.chunks.contains_key(&new_chunk_position) {
            return EMPTY_CELL;
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
        
        let old_cell = chunk_data.replace_cell(cell_position, cell);
        if old_cell.element != Element::Empty && cell.element == Element::Empty {
            chunk.cell_count.lock().unwrap().sub_assign(1);
        }
        else if old_cell.element == Element::Empty && cell.element != Element::Empty {
            chunk.cell_count.lock().unwrap().add_assign(1);
        }
        
        old_cell   
    }

    pub fn release_chunk(&self, chunk_position: &Vector2) {
        self.active_chunks.remove(chunk_position);
        self.suspended_chunks.insert(*chunk_position);
    }

    pub fn refresh_chunk(&self, chunk_position: &Vector2, cell_position: &Vector2) {
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
        let updated_pixels = Arc::new(Mutex::new(0));
        
        let positions: Vec<Vector2> = self.chunk_manager.active_chunks.iter().map(|v| *v).collect();

        let chunk_count = positions.len();

        let mut groups: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();

        for position in positions {
            groups.entry(position.x).or_insert(BTreeSet::new()).insert(position.y);
        }

        // Non optimized due to lock contention (?)
        #[cfg(feature = "multithreading")]
        {
            for iteration in 0..3 {
                for (x, group) in groups.iter_mut().filter(|(x, _)| *x % 3 == iteration % 3).rev() {
                    self.pool.execute({
                        let manager = self.chunk_manager.clone();
                        let updated_pixels = updated_pixels.clone();
                        let clock = self.clock;
                        let x = *x;
                        let group = group.clone();
                        move || {
                            for y in group {
                                let position = vec2!(x, y);
                                
                                // println!("{} {}", x, y);
                                // std::thread::sleep(Duration::from_secs(2));

                                let chunk = manager.chunks.get(&position).unwrap();
                                updated_pixels.lock().unwrap().add_assign(chunk.update(manager.clone(), clock));
    
                                if chunk.cell_count.lock().unwrap().eq(&0) {
                                    manager.release_chunk(&position);
                                }
                            }
                        }
                    });
                }

                self.pool.join();
            }
        }

        #[cfg(not(feature = "multithreading"))]
        for (x, group) in groups.iter_mut().rev() {   
            for y in group.iter().rev() {
                let position = &vec2!(*x, *y);
                let chunk = self.chunk_manager.chunks.get(position).unwrap();
                updated_pixels.lock().unwrap().add_assign(chunk.update(self.chunk_manager.clone(), self.clock));
                
                if chunk.cell_count.lock().unwrap().eq(&0) {
                    self.chunk_manager.release_chunk(position);
                }
            }
        }

        let lock = Arc::try_unwrap(updated_pixels).expect("Lock still has multiple owners");
        (chunk_count as u128, lock.into_inner().unwrap())
    }

    pub fn place(&self, x: i64, y: i64, element: Element) {
        self.chunk_manager.place(x, y, element);
    }

    pub fn render(&self, frame: &mut [u8]) -> Vec<Vec<Vertex>> {
        let mut boundaries: Vec<Vec<Vertex>> = vec![];

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
                    let cell = chunk_data.cells[get_cell_index(x as i64, y as i64)];
                    let offset = rand::thread_rng().gen_range(0..10);
                
                    let mut rgba = cell.element.color();
                    match cell.element {
                        Element::Sand | Element::Wood => {
                            for color in rgba.iter_mut() {
                                *color = color.saturating_add(cell.ra);
                            }
                        }
                        
                        Element::Water => {
                            for color in rgba.iter_mut() {
                                *color = color.saturating_add(offset);
                            }
                        }
                        _ => {}
                    }

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
            let chunk = self.chunk_manager.chunks.get(&chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * (WORLD_WIDTH * CHUNK_SIZE);

            let chunk_data = chunk.chunk_data.read().unwrap();

            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    let pixel_index = ((y_offset + y * (WORLD_WIDTH * CHUNK_SIZE)) + x + x_offset) * 4;
                    let cell = chunk_data.cells[get_cell_index(x as i64, y as i64)];
                    let offset = rand::thread_rng().gen_range(0..25);

                    let mut rgba = cell.element.color();
                    match cell.element {
                        Element::Sand | Element::Wood => {
                            for color in rgba.iter_mut() {
                                *color = color.saturating_add(cell.ra);
                            }
                        }
                        
                        Element::Water => {
                            for color in rgba.iter_mut() {
                                *color = color.saturating_add(offset);
                            }
                        }
                        _ => {}
                    }

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