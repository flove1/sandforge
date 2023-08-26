use std::{sync::Arc, collections::HashSet};

use dashmap::{DashMap, mapref::one::Ref};
use egui_winit::egui::mutex::Mutex;
use rand::Rng;
use scoped_threadpool::Pool;

use crate::{vec2, vector::Vector2, constants::*};

use super::{chunk::Chunk, elements::Element, helpers::get_cell_index, cell::{EMPTY_CELL, Cell}};

pub struct World {
    pub(super) chunks: DashMap<Vector2, Chunk>,
    pub(super) active_chunks: Mutex<HashSet<Vector2>>,
    pub(super) suspended_chunks: Mutex<HashSet<Vector2>>,
    pub(super) maximum_size: i64,
}

impl World {
    pub fn new() -> WorldApi {
        let manager = Self {
                chunks: DashMap::new(),
                maximum_size: (WORLD_SIZE * CHUNK_SIZE) as i64,
                active_chunks: Mutex::new(HashSet::new()),
                suspended_chunks: Mutex::new(HashSet::new()),
        };
        
        for x in 0..WORLD_SIZE {
            for y in 0..WORLD_SIZE {
                manager.chunks.insert(vec2!(x, y), Chunk::new(vec2!(x, y)));
            }
        }

        WorldApi{
            chunk_manager: Arc::new(manager), 
            clock: 0, 
            previous_update_ms: 0, pool: 
            Pool::new(4)
        }
    }    

    pub(crate) fn place(&self, x: i64, y: i64, element: Element) {
        if x < 0 || y < 0 || x > self.maximum_size || y > self.maximum_size {
            return;
        }
        let position = vec2!(x / CHUNK_SIZE, y / CHUNK_SIZE);
        if let Some(chunk) = self.get_chunk(&position) {
            self.refresh_chunk(&position);
            chunk.place(x % CHUNK_SIZE, y % CHUNK_SIZE, element);
        }
    }

    pub(crate) fn update_cell(&self, chunk_position: Vector2, cell_position: Vector2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);

        if let Some(chunk) = self.get_chunk(&(chunk_position + chunk_offset)) {
            chunk.update_cell(cell_position, cell);
        }
    }    

    pub(crate) fn set_cell(&self, chunk_position: Vector2, cell_position: Vector2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);

        if let Some(chunk) = self.get_chunk(&(chunk_position + chunk_offset)) {
            chunk.set_cell(cell_position, cell);
            self.refresh_chunk_at_cell(&chunk_position, &cell_position);
        }
    }    

    pub(crate) fn get_cell(&self, chunk_position: Vector2, cell_position: Vector2) -> Cell {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        match self.get_chunk(&(chunk_position + chunk_offset)) {
            Some(chunk) => {
                chunk.get_cell(cell_position)
            },
            None => {
                EMPTY_CELL
            },
        }
    }    

    pub(crate) fn swap_cells(&self, chunk_position: Vector2, cell_position: Vector2, new_cell_position: Vector2) {
        let (cell_position,chunk_offset ) = cell_position.wrap(0, CHUNK_SIZE);
        let (new_cell_position, new_chunk_offset) = new_cell_position.wrap(0, CHUNK_SIZE);

        if chunk_offset == new_chunk_offset {
            match self.get_chunk(&(chunk_position + chunk_offset)) {
                Some(chunk) => chunk.swap_cells(cell_position, new_cell_position),
                None => {},
            }
        }
        else {
            let result_1 = self.get_chunk(&(chunk_position + chunk_offset));
            let result_2 = self.get_chunk(&(chunk_position + new_chunk_offset));
            if result_1.is_none() && result_2.is_none() {
                return ;
            }

            match result_1 {
                Some(chunk) => {
                    match result_2 {
                        Some(new_chunk) => {
                            let cell_1 = chunk.get_cell(cell_position);
                            let cell_2 = new_chunk.get_cell(new_cell_position);

                            if cell_1.element == Element::Empty {
                                chunk.cell_count.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                                new_chunk.cell_count.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                                
                                self.refresh_chunk_at_cell(&(chunk_position + chunk_offset), &cell_position);
                            }
                            else if cell_2.element == Element::Empty {
                                chunk.cell_count.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                                new_chunk.cell_count.fetch_add(1, std::sync::atomic::Ordering::AcqRel);

                                self.refresh_chunk_at_cell(&(chunk_position + new_chunk_offset), &new_cell_position);
                            }
                            else {
                                self.refresh_chunk_at_cell(&(chunk_position + chunk_offset), &cell_position);
                                self.refresh_chunk_at_cell(&(chunk_position + new_chunk_offset), &new_cell_position);
                            }

                            chunk.update_cell(cell_position, cell_2);
                            new_chunk.update_cell(new_cell_position, cell_1);
                        },
                        None => {
                            chunk.set_cell(cell_position, EMPTY_CELL);
                            chunk.cell_count.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                        },
                    }
                },
                None => {
                    panic!()
                },
            }
        }
    }    

    pub(crate) fn get_chunk(&self, position: &Vector2) -> Option<Ref<Vector2, Chunk>> {
        self.chunks.get(position)
    }

    pub(crate) fn get_chunk_by_pixel(&self, x: i64, y: i64) -> Option<Ref<Vector2, Chunk>> {
        let chunk_x = x.div_euclid(CHUNK_SIZE);
        let chunk_y = y.div_euclid(CHUNK_SIZE);
        self.chunks.get(&vec2!(chunk_x, chunk_y))
    }

    pub fn refresh_chunk_at_cell(&self, chunk_position: &Vector2, cell_position: &Vector2) {
        match self.get_chunk(chunk_position) {
            Some(chunk) => {
                let mut active_lock = self.active_chunks.lock();

                if !active_lock.contains(&chunk_position) {
                    active_lock.insert(*chunk_position);
                    chunk.maximize_dirty_rect();
                }
                else {
                    chunk.update_dirty_rect(cell_position);
                }
            },
            None => {},
        }
    }


    pub fn refresh_chunk(&self, chunk_position: &Vector2) {
        match self.get_chunk(chunk_position) {
            Some(chunk) => {
                let mut active_lock = self.active_chunks.lock();

                if !active_lock.contains(&chunk_position) {
                    active_lock.insert(*chunk_position);
                    chunk.maximize_dirty_rect();
                }
            },
            None => {},
        }
    }

    pub fn release_chunk(&self, chunk_position: &Vector2) {
        self.active_chunks.lock().remove(chunk_position);
        self.suspended_chunks.lock().insert(*chunk_position);
    }
}

pub struct WorldApi {
    chunk_manager: Arc<World>,
    previous_update_ms: u128,
    clock: u8,
    pool: Pool,
}

impl WorldApi {
    pub fn needs_update(&mut self, dt: u128) -> bool {
        self.previous_update_ms += dt;
        self.previous_update_ms > DELAY
    }

    pub fn update(&mut self) -> (u128, u128) {
        let mut chunk_count: u128 = 0;
        let mut pixel_count: u128 = 0;

        while self.previous_update_ms > DELAY {
            self.previous_update_ms -= DELAY;
            let active_chunks = self.chunk_manager.active_chunks.lock().clone();
            chunk_count += active_chunks.len() as u128;
        
            let iter_range: Vec<i8> = if self.clock % 2 == 0 { (0..4).collect() } else { (0..4).rev().collect() };

            for iteration in iter_range {
                self.clock = self.clock.wrapping_add(1);
                self.pool.scoped(|s| {
                    let positions: Vec<&Vector2> = match iteration {
                        0 => active_chunks.iter().filter(|v| v.x%2 == 0 && v.y%2 == 0).collect(),
                        1 => active_chunks.iter().filter(|v| v.x%2 == 0 && v.y%2 == 1).collect(),
                        2 => active_chunks.iter().filter(|v| v.x%2 == 1 && v.y%2 == 0).collect(),
                        3 => active_chunks.iter().filter(|v| v.x%2 == 1 && v.y%2 == 1).collect(),
                        _ => panic!("how"),
                    };
    
                    s.execute(|| {
                        for position in  positions {
                            let chunk = self.chunk_manager.chunks.get(position).unwrap();
                            pixel_count += chunk.update(self.chunk_manager.clone(), self.clock);
    
                            if chunk.cell_count.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                                self.chunk_manager.release_chunk(position);
                            }
                        }
                    })
                });
            }
        }

        (chunk_count, pixel_count)
        
    }

    pub fn place(&self, x: i64, y: i64, element: Element) {
        self.chunk_manager.place(x, y, element);
    }

    pub fn render(&self, frame: &mut [u8]) {
        let active_lock = self.chunk_manager.active_chunks.lock();
        let mut suspended_lock = self.chunk_manager.suspended_chunks.lock();

        for chunk_position in active_lock.iter() {
            let chunk = self.chunk_manager.get_chunk(chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * self.chunk_manager.maximum_size;

            let (dirty_rect_x, dirty_rect_y) = chunk.dirty_rect.lock().unwrap().get_ranges_render();

            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    let pixel_index = ((y_offset + y * self.chunk_manager.maximum_size) + x_offset + x) * 4;
                    let cell = chunk.cells[get_cell_index(x as i64, y as i64)].load();
                    let offset = rand::thread_rng().gen_range(0..25);
                    let rgba = match cell.element {
                        Element::Empty => [0x00, 0x00, 0x00, 0xff],
                        Element::Stone => [0x77, 0x77, 0x77, 0xff],
                        Element::Sand => [0xf2_u8.saturating_add(cell.ra), 0xf1_u8.saturating_add(cell.ra), 0xa3_u8.saturating_add(cell.ra), 0xff],
                        Element::Water => [0x47 + offset, 0x7C + offset, 0xB8 + offset, 0xff],
                        Element::GlowingSand => {
                            [0xe8, 0x6a, 0x17, 0xff]
                        },
                    };

                    if dirty_rect_x.contains(&x) && dirty_rect_y.contains(&y) {
                        frame[pixel_index as usize] = rgba[0].saturating_add(25);
                        frame[pixel_index as usize + 1] = rgba[1].saturating_add(50);
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
                let end_offset = (((CHUNK_SIZE-1) * self.chunk_manager.maximum_size + x + x_offset + y_offset) * 4) as usize;
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
                let start_offset = ((y * self.chunk_manager.maximum_size + x_offset + y_offset)*4) as usize;
                let end_offset = ((y * self.chunk_manager.maximum_size + CHUNK_SIZE - 1 + x_offset + y_offset)*4) as usize;
                frame[start_offset as usize] = frame[start_offset as usize].saturating_add(25);
                frame[start_offset+1 as usize] = frame[start_offset+1 as usize].saturating_add(25);
                frame[start_offset+2 as usize] = frame[start_offset+2 as usize].saturating_add(25);
                frame[start_offset+3 as usize] = frame[start_offset+3 as usize].saturating_add(25);

                frame[end_offset as usize] = frame[end_offset as usize].saturating_add(25);
                frame[end_offset+1 as usize] = frame[end_offset+1 as usize].saturating_add(25);
                frame[end_offset+2 as usize] = frame[end_offset+2 as usize].saturating_add(25);
                frame[end_offset+3 as usize] = frame[end_offset+3 as usize].saturating_add(25);
            }
        }

        for chunk_position in suspended_lock.clone() {
            suspended_lock.remove(&chunk_position);
            let chunk = self.chunk_manager.get_chunk(&chunk_position).unwrap();
            let x_offset = chunk_position.x * CHUNK_SIZE;
            let y_offset = chunk_position.y * CHUNK_SIZE * self.chunk_manager.maximum_size;

            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    let pixel_index = ((y_offset + y * self.chunk_manager.maximum_size) + x + x_offset) * 4;
                    let cell = chunk.cells[get_cell_index(x as i64, y as i64)].load();
                    let offset = rand::thread_rng().gen_range(0..25);
                    let rgba = match cell.element {
                        Element::Empty => [0x00, 0x00, 0x00, 0xff],
                        Element::Stone => [0x77, 0x77, 0x77, 0xff],
                        Element::Sand => [0xf2_u8.saturating_add(cell.ra), 0xf1_u8.saturating_add(cell.ra), 0xa3_u8.saturating_add(cell.ra), 0xff],
                        Element::Water => [0x47 + offset, 0x7C + offset, 0xB8 + offset, 0xff],
                        Element::GlowingSand => {
                            [0xe8, 0x6a, 0x17, 0xff]
                        },
                    };

                    frame[pixel_index as usize] = rgba[0];
                    frame[pixel_index as usize + 1] = rgba[1];
                    frame[pixel_index as usize + 2] = rgba[2];
                    frame[pixel_index as usize + 3] = rgba[3];
                }
            }
        }
    }
}