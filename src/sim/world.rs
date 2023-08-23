use std::{sync::Arc, thread};

use dashmap::{DashMap, mapref::one::Ref};
use rand::Rng;

use crate::{vec2, vector::Vector2, constants::*};

use super::{chunk::Chunk, elements::Element, helpers::get_cell_index};

pub struct World {
    pub(super) chunks: DashMap<Vector2, Chunk>,
    pub(super) maximum_size: i64,
}

impl World {
    pub fn new() -> WorldApi {
        let manager = Self {
                chunks: DashMap::new(),
                maximum_size: (WORLD_SIZE * CHUNK_SIZE) as i64,
        };
        
        for x in 0..WORLD_SIZE {
            for y in 0..WORLD_SIZE {
                manager.chunks.insert(vec2!(x, y), Chunk::new());
            }
        }

        WorldApi{chunk_manager: Arc::new(manager), iter_bit: false}
    }    

    pub(crate) fn place(&self, x: i64, y: i64, element: Element) {
        if x < 0 || y < 0 || x > self.maximum_size || y > self.maximum_size {
            return;
        }
        let chunk_x = x / CHUNK_SIZE;
        let chunk_y = y / CHUNK_SIZE;
        if let Some(chunk) = self.get_chunk(chunk_x, chunk_y) {
            chunk.place(x % CHUNK_SIZE, y % CHUNK_SIZE, element);
        }
    }    

    // pub(crate) fn get_cell_from_pixel_index(&self, index: i64) -> Option<&AtomicCell<Cell>> {
    //     let y = index % (CHUNK_ELEMENTS * WORLD_SIZE as i64) / self.maximum_size;
    //     let x = index % CHUNK_SIZE as i64;

    //     let chunk_y = index / (CHUNK_ELEMENTS * WORLD_SIZE as i64);
    //     let chunk_x = index % self.maximum_size / CHUNK_SIZE as i64;

    //     if let Some(chunk) = self.get_chunk(chunk_x, chunk_y) {
    //         Some(&chunk.cells[get_cell_index(x as i64, y as i64) as usize])
    //     }
    //     else {
    //         None
    //     }
    // }

    // pub(crate) fn get_cell(&self, chunk_x: i64, chunk_y: i64, x: i64, y: i64) -> Option<&AtomicCell<Cell>> {
    //     if let Some(chunk) = self.get_chunk(chunk_x, chunk_y) {
    //         Some(&chunk.cells[get_cell_index(x, y)])
    //     }
    //     else {
    //         None
    //     }
    // }

    pub(crate) fn get_chunk(&self, x: i64, y: i64) -> Option<Ref<Vector2, Chunk>> {
        self.chunks.get(&vec2!(x, y))
    }

    pub(crate) fn get_chunk_by_pixel(&self, x: i64, y: i64) -> Option<Ref<Vector2, Chunk>> {
        let chunk_x = x.div_euclid(CHUNK_SIZE);
        let chunk_y = y.div_euclid(CHUNK_SIZE);
        self.chunks.get(&vec2!(chunk_x, chunk_y))
    }
}

pub struct WorldApi {
    chunk_manager: Arc<World>,
    iter_bit: bool,
}

impl WorldApi {
    pub fn update(&mut self, _dt: f32) {
        self.iter_bit = !self.iter_bit;

        for iteration in 0..4 {
            let (x_range, y_range): (Vec<i64>, Vec<i64>) = match iteration {
                0 => {(
                    (0..WORLD_SIZE).filter(|x| x%2 == 0).collect(),
                    (0..WORLD_SIZE).filter(|y| y%2 == 0).collect()
                )},
                1 => {(
                    (0..WORLD_SIZE).filter(|x| x%2 == 0).collect(),
                    (0..WORLD_SIZE).filter(|y| y%2 == 1).collect()
                )},
                2 => {(
                    (0..WORLD_SIZE).filter(|x| x%2 == 1).collect(),
                    (0..WORLD_SIZE).filter(|y| y%2 == 0).collect()
                )},
                3 => {(
                    (0..WORLD_SIZE).filter(|x| x%2 == 1).collect(),
                    (0..WORLD_SIZE).filter(|y| y%2 == 1).collect()
                )},
                _ => {panic!("how")},
            };

            thread::scope(|s| {
                for x in x_range.iter() {
                    for y in y_range.iter() {
                        s.spawn(|| {
                            let chunk = self.chunk_manager.get_chunk(*x, *y).unwrap();
                            chunk.update(vec2!(*x, *y), self.chunk_manager.clone(), self.iter_bit);
                        });
                    }
                }
            });
        }
    }

    pub fn place(&self, x: i64, y: i64, element: Element) {
        self.chunk_manager.place(x, y, element);
    }    
    
    // fn get_cell_updates(dt: f32, chunk_index: i64, manager_ref: &Arc<RwLock<World>>) -> Vec<((i64, i64), Vec<CellAction>)> {
    //     let mut cell_updates: Vec<((i64, i64), Vec<CellAction>)> = vec![];
    //     let manager = manager_ref.read().unwrap();
    //     let chunk = &manager.chunks[chunk_index];

    //     for x in 0..manager.chunk_size {
    //         for y in 0..manager.chunk_size {
    //             let cell = chunk.cells[manager.get_cell_index(x, y)];
    //             if cell.iter_bit == manager.iter_bit {
    //                 let actions = cell.update(PixelToChunkApi { 
    //                     x: x, 
    //                     y: y, 
    //                     chunk_index: chunk_index, 
    //                     chunk_manager: &manager,
    //                 }, dt);
    
    //                 if actions.len() != 0 {
    //                     cell_updates.push((
    //                         (x, y), 
    //                         actions
    //                     ));
    //                 }
    //             }
    //         }
    //     }

    //     cell_updates
    // }

    // fn process_cell_updates(chunk_index: i64, cell_updates: Vec<((i64, i64), Vec<CellAction>)>, manager_ref: &Arc<RwLock<World>>) {
    //     let mut manager = manager_ref.write().unwrap();

    //     for ((mut x, mut y), actions) in cell_updates {
    //         let mut cell_chunk_index = chunk_index;
    //         for action in actions {
    //             match action {
    //                 CellAction::Swap(dx, dy) => {
    //                     let cell_index_1 = manager.get_cell_index(x, y);
    //                     let nx = x + dx;
    //                     let ny = y + dy;
    //                     if nx < 0 || nx > (manager.chunk_size - 1) as i64 || ny < 0 || ny > (manager.chunk_size - 1) as i64 {
    //                         let (new_cell_chunk_index, nx, ny) = manager.switch_chunk(cell_chunk_index, nx, ny);
    //                         let cell_index_2 = manager.get_cell_index(nx, ny);
    //                         let temp_cell = manager.chunks[cell_chunk_index].cells[cell_index_1];
    //                         manager.chunks[cell_chunk_index].cells[cell_index_1] = manager.chunks[new_cell_chunk_index].cells[cell_index_2];
    //                         manager.chunks[new_cell_chunk_index].cells[cell_index_2] = temp_cell;
    //                         x = nx;
    //                         y = ny;
    //                         cell_chunk_index = new_cell_chunk_index;
    //                     }
    //                     else {
    //                         let cell_index_2 = manager.get_cell_index(nx, ny);
    //                         manager.chunks[cell_chunk_index].cells.swap(cell_index_1, cell_index_2);
    //                         x += dx;
    //                         y += dy;
    //                     }
    //                 },
    //                 CellAction::Set(dx, dy, cell) => {
    //                     let nx = x + dx;
    //                     let ny = y + dy;
    //                     if nx < 0 || nx > (manager.chunk_size - 1) as i64 || ny < 0 || ny > (manager.chunk_size - 1) as i64 {
    //                         let (new_chunk_index, nx, ny) = manager.switch_chunk(cell_chunk_index, nx, ny);
    //                         let index = manager.get_cell_index(nx, ny);
    //                         manager
    //                             .chunks[new_chunk_index]
    //                             .cells[index] = cell;
    //                     }
    //                     else {
    //                         let index = manager.get_cell_index(nx, ny);
    //                         manager
    //                             .chunks[cell_chunk_index]
    //                             .cells[index] = cell;
    //                     }
    //                 },
    //                 CellAction::Update(cell) => {
    //                     let index = manager.get_cell_index(x, y);
    //                     manager
    //                         .chunks[cell_chunk_index]
    //                         .cells[index] = cell;
    //                 },
    //             }
    //         }
    //         let index = manager.get_cell_index(x, y);
    //         manager
    //             .chunks[cell_chunk_index]
    //             .cells[index]
    //             .iter_bit = !manager.iter_bit;
    //     }
    // }

    pub fn render(&self, frame: &mut [u8]) {
        for chunk_x in 0..WORLD_SIZE {
            for chunk_y in 0..WORLD_SIZE {
                let chunk = self.chunk_manager.get_chunk(chunk_x, chunk_y).unwrap();
                let x_offset = chunk_x * CHUNK_SIZE;
                let y_offset = chunk_y * CHUNK_SIZE * self.chunk_manager.maximum_size;

                let (dirty_rect_x, dirty_rect_y) = chunk.dirty_rect.lock().unwrap().get_ranges_render();

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
                        if dirty_rect_x.contains(&x) && dirty_rect_y.contains(&y) {
                            frame[pixel_index as usize] = rgba[0].saturating_add(100);
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

                // for y in 0..self.chunk_manager.maximum_size {
                //     world.place(0, y as i64, Element::Stone);
                //     world.place((CHUNK_SIZE * WORLD_SIZE - 1) as i64, y as i64, Element::Stone);
                // }
            }
        }

        // for (index, _) in glowing_pixels {
        //     let y = (index / self.chunk_manager.maximum_size) as i64;
        //     let x = (index % self.chunk_manager.maximum_size) as i64;
        //     let range = 0..(manager.maximum_size as i64);
        //     for offset_x in -4..5 {
        //         for offset_y in -4..5 {
        //             if offset_x == 0 && offset_y == 0 {
        //                 continue;
        //             }

        //             let affected_pixel_x = x + offset_x; 
        //             let affected_pixel_y = y + offset_y; 
        //             if !range.contains(&affected_pixel_x) || !range.contains(&affected_pixel_y) {
        //                 continue;
        //             }
                    
        //             let intensity = (1.0 / (offset_x.abs() + offset_y.abs()) as f32).clamp(0.0, 0.2);
                    
        //             let index = ((affected_pixel_y * (manager.maximum_size) as i64 + affected_pixel_x) * 4) as i64;
        //             frame[index] = ((frame[index] as f32 * (1.0 - intensity) + 0xe8 as f32 * (intensity)) / 2.0) as u8;
        //             frame[index + 1] = ((frame[index + 1] as f32 * (1.0 - intensity) + 0x6a as f32 * (intensity)) / 2.0) as u8;
        //             frame[index + 2] = ((frame[index + 2] as f32 * (1.0 - intensity) + 0x17 as f32 * (intensity)) / 2.0) as u8;
        //         }
        //     }
        // }
    }
}