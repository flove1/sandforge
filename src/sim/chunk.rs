use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crossbeam::atomic::AtomicCell;
use rand::Rng;

use super::cell::*;
use super::elements::Element;
use super::helpers::get_cell_index;
use super::world::World;

use crate::{constants::*, vec2};
use crate::vector::Vector2;

#[derive(Default)]
pub struct Chunk {
    pub(super) cells: Vec<AtomicCell<Cell>>,
    pub(super) dirty_rect: Mutex<Rect>,
    placing_queue: Mutex<VecDeque<(Vector2, Element)>>,

    pub(super) active: AtomicBool,
    pub(super) needs_refreshing: AtomicBool,
}

#[derive(Default, Clone, Copy)]
pub struct Rect {
    // x1, y1, x2, y2
    corners: Option<[i64; 4]>,
}

impl Rect {
    fn update(&mut self, x: i64, y: i64) {
        match &mut self.corners {
            Some(corners) => {
                if x < corners[0] {
                    corners[0] = x;
                }
        
                if y < corners[1] {
                    corners[1] = y;
                }
                
                if x+1 >= corners[2] {
                    corners[2] = x+1;
                }
                
                if y+1 >= corners[3] {
                    corners[3] = y+1;
                }
            }
            None => {
                self.corners = Some([x, y, x+1, y+1]);
                return;
            }
        };
    }

    pub fn get_ranges(&self, iter_bit: bool) -> (Vec<i64>, Vec<i64>) {
        match self.corners {
            Some(corners) => {
                return (
                    if iter_bit {
                        (corners[0]..corners[2]).collect()
                    }
                    else {
                        (corners[0]..corners[2]).rev().collect()
                    },
                    (corners[1]..corners[3]).collect()
                )
            },
            None => panic!(),
        }
    }

    pub fn get_ranges_render(&self) -> (Vec<i64>, Vec<i64>) {
        match self.corners {
            Some(corners) => {
                ((corners[0]..corners[2]).collect(), (corners[1]..corners[3]).collect())
            },
            None => (vec![], vec![]),
        }
    }

    fn retrieve(&mut self) -> Rect {
        let clone = self.clone();
        self.corners = None;
        clone
    }

    fn is_empty_rect(&self) -> bool {
        self.corners == None
    }
}

impl Chunk {
    pub(crate) fn new() -> Self {
        let mut chunk = Self {
            cells: Vec::with_capacity(CHUNK_SIZE.pow(2) as usize),
            placing_queue: Mutex::new(VecDeque::new()),
            dirty_rect: Mutex::new(Rect::default()),
            ..Default::default()
        };

        for _ in 0..(CHUNK_SIZE.pow(2)) {
            chunk.cells.push(AtomicCell::new(Cell::default()));
        }

        chunk
    }

    pub(crate) fn place(&self, x: i64, y: i64, element: Element) {
        let mut queue = self.placing_queue.lock().unwrap();
        queue.push_back((vec2!(x, y), element));
    }

    pub(crate) fn swap_cells(&self, index_1: usize, index_2: usize) {
        self.cells[index_1].store(self.cells[index_2].swap(self.cells[index_1].load()));
    }

    pub(crate) fn update_dirty_rect(&self, x: i64, y: i64) {
        let mut rect_lock = self.dirty_rect.lock().unwrap();
        let corners = [
            ((x - DIRTY_CHUNK_OFFSET), (y - DIRTY_CHUNK_OFFSET)),
            ((x + DIRTY_CHUNK_OFFSET), (y + DIRTY_CHUNK_OFFSET)),
        ].map(|(x, y)| (x.clamp(0, CHUNK_SIZE-1), y.clamp(0, CHUNK_SIZE-1)));

        for (x, y) in corners {
            rect_lock.update(x, y);
        }            
    }
    
    // pub(crate) fn set_cell(&self, x: i64, y: i64, cell: Cell) {
    //     self.cells[get_cell_index(x, y)].store(cell);

    //     let mut rect_lock = self.dirty_rect.lock().unwrap();
    //         let corners = [
    //             ((x - DIRTY_CHUNK_OFFSET), (y - DIRTY_CHUNK_OFFSET)),
    //             ((x + DIRTY_CHUNK_OFFSET), (y + DIRTY_CHUNK_OFFSET)),
    //         ].map(|(x, y)| (x.clamp(0, CHUNK_SIZE-1), y.clamp(0, CHUNK_SIZE-1)));

    //         for (x, y) in corners {
    //             rect_lock.update(x, y);
    //         }

    //         let bordering_chunk_offset = vec2!(
    //             if x == 0 { -1 }
    //             else if x == CHUNK_SIZE - 1 { 1 }
    //             else { 0 },

    //             if y == 0 { -1 }
    //             else if y == CHUNK_SIZE - 1 { 1 }
    //             else { 0 }
    //         );

    //         if bordering_chunk_offset != vec2!(0, 0) {
    //             if let Some(chunk) = self.chunk_manager.get_chunk(self.chunk_position.x + bordering_chunk_offset.x, self.chunk_position.y + bordering_chunk_offset.y) {
    //                 if !chunk.active.load(std::sync::atomic::Ordering::Relaxed) {
    //                     chunk.needs_refreshing.store(true, std::sync::atomic::Ordering::Relaxed);
    //                 }
    //             }
    //         }
    // }

    pub(crate) fn process_previous_updates(&self, iter_bit: bool) -> Option<Rect> {
        let mut queue = self.placing_queue.lock().unwrap();
        let mut dirty_rect = self.dirty_rect.lock().unwrap();

        if queue.is_empty() && dirty_rect.is_empty_rect() {
            return None;
        }

        while !queue.is_empty() {
            let (cell_position, element) = queue.pop_front().unwrap();
            let index = (cell_position.y * CHUNK_SIZE + cell_position.x) as usize;
            if matches!(self.cells[index].load().element, Element::Empty) || matches!(element, Element::Empty) {
                self.cells[index].store(Cell::new(element, iter_bit));
            }

            dirty_rect.update(cell_position.x, cell_position.y);
        }

        Some(dirty_rect.retrieve())
    }

    pub(crate) fn update(&self, position: Vector2, manager: Arc<World>, iter_bit: bool) {
        let (x_range, y_range) = {
            let result = self.process_previous_updates(iter_bit);
            match result {
                Some(dirty_rect) => {
                    self.active.store(true, std::sync::atomic::Ordering::Relaxed);
                    dirty_rect.get_ranges(iter_bit)
                }
                None => {
                    self.active.store(false, std::sync::atomic::Ordering::Relaxed);
                    if !self.needs_refreshing.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }

                    self.needs_refreshing.store(false, std::sync::atomic::Ordering::Relaxed);
                    (
                        if iter_bit {(0..CHUNK_SIZE).collect()} else {(0..CHUNK_SIZE).rev().collect()},
                        (0..CHUNK_SIZE).collect(),
                    ) 
                }
            }
        };

        for x in x_range.iter() {
            for y in y_range.iter() {
                let cell = self.cells[get_cell_index(*x, *y)].load();

                let api = ChunkApi { 
                    cell_position: vec2!(*x, *y),
                    cell_offset: vec2!(0, 0),
                    chunk_position: position,
                    chunk_manager: manager.clone(),
                    iter_bit,
                    chunk: self,
                };

                // drop(cells_lock);
                if cell.iter_bit == iter_bit && cell.element == Element::Sand {
                    cell.update(api, 0.0);
                }
            }
        }
    }
}

pub struct ChunkApi<'a> {
    pub(super) cell_position: Vector2,
    pub(super) cell_offset: Vector2,
    pub(super) chunk: &'a Chunk,
    pub(super) chunk_position: Vector2,
    pub(super) chunk_manager: Arc<World>,
    pub(super) iter_bit: bool,
}

impl<'a> ChunkApi<'a> {
    pub fn get(&mut self, dx: i64, dy: i64) -> Cell {
        let mut cell_position = vec2!(self.cell_position.x + self.cell_offset.x + dx, self.cell_position.y + self.cell_offset.y + dy);

        if cell_position.is_between(0, CHUNK_SIZE) {
            self.chunk.cells[get_cell_index(cell_position.x, cell_position.y)].load()
        }
        else {
            let chunk_offset = cell_position.wrap_and_return_offset(0, CHUNK_SIZE);
            match self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset.x, self.chunk_position.y + chunk_offset.y) {
                Some(chunk) => {
                    chunk.cells[get_cell_index(cell_position.x, cell_position.y)].load()
                },
                None => {
                    EMPTY_CELL
                },
            }
        }
    }

    pub fn match_element(&mut self, dx: i64, dy: i64, element: Element) -> bool {
        let mut cell_position = vec2!(self.cell_position.x + self.cell_offset.x + dx, self.cell_position.y + self.cell_offset.y + dy);

        if cell_position.is_between(0, CHUNK_SIZE) {
            self.chunk.cells[get_cell_index(cell_position.x, cell_position.y)].load().element == element
        }
        else {
            let chunk_offset = cell_position.wrap_and_return_offset(0, CHUNK_SIZE);
            match self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset.x, self.chunk_position.y + chunk_offset.y) {
                Some(chunk) => {
                    chunk.cells[get_cell_index(cell_position.x, cell_position.y)].load().element == element
                },
                None => {
                  element == Element::Empty  
                },
            }
        }
    }

    pub fn set(&mut self, _dx: i64, _dy: i64, _cell: Cell) {
        todo!();
        // let mut cell_position = vec2!(self.cell_position.x + self.cell_offset.x + dx, self.cell_position.y + self.cell_offset.y + dy);

        // let result = {
        //     if !cell_position.is_between(0, CHUNK_SIZE) {
        //         let chunk_offset = cell_position.wrap_and_return_offset(0, CHUNK_SIZE);
        //         self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset.x, self.chunk_position.y + chunk_offset.y)
        //     }
        //     else {
        //         self.chunk_manager.get_chunk(self.chunk_position.x, self.chunk_position.y)
        //     }
        // };

        // if let Some(chunk) = result {
        //     let cell_index = get_cell_index(cell_position.x, cell_position.y);
        //     chunk.cells[cell_index].store(cell);
        // }
    }

    pub fn swap(&mut self, dx:i64, dy: i64) {
        let mut cell_position_1 = vec2!(self.cell_position.x + self.cell_offset.x, self.cell_position.y + self.cell_offset.y);
        let mut cell_position_2 = vec2!(self.cell_position.x + self.cell_offset.x + dx, self.cell_position.y + self.cell_offset.y + dy);
        
        if cell_position_1.is_between(0, CHUNK_SIZE) && cell_position_2.is_between(0, CHUNK_SIZE) {
            let cell_index_1 = get_cell_index(cell_position_1.x, cell_position_1.y);
            let cell_index_2 = get_cell_index(cell_position_2.x, cell_position_2.y);

            // let chunk = self.chunk_manager.chunks.get(&self.chunk_position).unwrap();
            self.chunk.swap_cells(cell_index_1, cell_index_2);

            let mut rect_lock = self.chunk.dirty_rect.lock().unwrap();
            let corners = [
                ((cell_position_1.x - DIRTY_CHUNK_OFFSET), (cell_position_1.y - DIRTY_CHUNK_OFFSET)),
                ((cell_position_2.x - DIRTY_CHUNK_OFFSET), (cell_position_2.y - DIRTY_CHUNK_OFFSET)),
                ((cell_position_1.x + DIRTY_CHUNK_OFFSET), (cell_position_1.y + DIRTY_CHUNK_OFFSET)),
                ((cell_position_2.x + DIRTY_CHUNK_OFFSET), (cell_position_2.y + DIRTY_CHUNK_OFFSET)),
            ].map(|(x, y)| (x.clamp(0, CHUNK_SIZE-1), y.clamp(0, CHUNK_SIZE-1)));

            for (x, y) in corners {
                rect_lock.update(x, y);
            }

            let bordering_chunk_offset = vec2!(
                if cell_position_1.x <= 1 { -1 }
                else if cell_position_1.x >= CHUNK_SIZE - 2 { 1 }
                else { 0 },

                if cell_position_1.y <= 1 { -1 }
                else if cell_position_1.y >= CHUNK_SIZE - 2 { 1 }
                else { 0 }
            );

            if bordering_chunk_offset != vec2!(0, 0) {
                if let Some(chunk) = self.chunk_manager.get_chunk(self.chunk_position.x + bordering_chunk_offset.x, self.chunk_position.y + bordering_chunk_offset.y) {
                    if !chunk.active.load(std::sync::atomic::Ordering::Relaxed) {
                        chunk.needs_refreshing.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }   
        }
        else {
            let chunk_offset_1 = cell_position_1.wrap_and_return_offset(0, CHUNK_SIZE);
            let chunk_offset_2 = cell_position_2.wrap_and_return_offset(0, CHUNK_SIZE);

            let cell_index_1 = get_cell_index(cell_position_1.x, cell_position_1.y);
            let cell_index_2 = get_cell_index(cell_position_2.x, cell_position_2.y);

            if chunk_offset_1 == chunk_offset_2 {
                match self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset_1.x, self.chunk_position.y + chunk_offset_1.y) {
                    Some(chunk) => {
                        chunk.swap_cells(cell_index_1, cell_index_2);

                        let mut rect_lock = chunk.dirty_rect.lock().unwrap();

                        let corners = [
                            ((cell_position_1.x - DIRTY_CHUNK_OFFSET), (cell_position_1.y - DIRTY_CHUNK_OFFSET)),
                            ((cell_position_2.x - DIRTY_CHUNK_OFFSET), (cell_position_2.y - DIRTY_CHUNK_OFFSET)),
                            ((cell_position_1.x + DIRTY_CHUNK_OFFSET), (cell_position_1.y + DIRTY_CHUNK_OFFSET)),
                            ((cell_position_2.x + DIRTY_CHUNK_OFFSET), (cell_position_2.y + DIRTY_CHUNK_OFFSET)),
                        ].map(|(x, y)| (x.clamp(0, CHUNK_SIZE-1), y.clamp(0, CHUNK_SIZE-1)));
                        for (x, y) in corners {
                            rect_lock.update(x, y);
                        }
                    },
                    None => {
                        return;
                    },
                }
            }
            else {
                let chunk_1 = self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset_1.x, self.chunk_position.y + chunk_offset_1.y).unwrap();

                match self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset_2.x, self.chunk_position.y + chunk_offset_2.y) {
                    Some(chunk_2) => {
                        chunk_1.cells[cell_index_1].store(chunk_2.cells[cell_index_2].swap(chunk_1.cells[cell_index_1].load()));

                        let mut rect_lock = chunk_2.dirty_rect.lock().unwrap();
                        let corners = [
                            ((cell_position_2.x - DIRTY_CHUNK_OFFSET), (cell_position_2.y - DIRTY_CHUNK_OFFSET)),
                            ((cell_position_2.x + DIRTY_CHUNK_OFFSET), (cell_position_2.y + DIRTY_CHUNK_OFFSET)),
                            ].map(|(x, y)| (x.clamp(0, CHUNK_SIZE-1), y.clamp(0, CHUNK_SIZE-1)));
                        for (x, y) in corners {
                            rect_lock.update(x, y);
                        }
                    },
                    None => {
                        chunk_1.cells[cell_index_1].store(EMPTY_CELL);
                    },
                } 

                let mut rect_lock = chunk_1.dirty_rect.lock().unwrap();

                let corners = [
                    ((cell_position_1.x - DIRTY_CHUNK_OFFSET), (cell_position_1.y - DIRTY_CHUNK_OFFSET)),
                    ((cell_position_1.x + DIRTY_CHUNK_OFFSET), (cell_position_1.y + DIRTY_CHUNK_OFFSET)),
                    ].map(|(x, y)| (x.clamp(0, CHUNK_SIZE-1), y.clamp(0, CHUNK_SIZE-1)));
                for (x, y) in corners {
                    rect_lock.update(x, y);
                }
            }
        }

        self.cell_offset.x += dx;
        self.cell_offset.y += dy;
    }

    pub fn update(&mut self, cell: Cell) {
        let mut cell_position = vec2!(self.cell_position.x + self.cell_offset.x, self.cell_position.y + self.cell_offset.y);

        if cell_position.is_between(0, CHUNK_SIZE) {
            self.chunk.cells[get_cell_index(cell_position.x, cell_position.y)].store(cell);
        }
        else {
            let chunk_offset = cell_position.wrap_and_return_offset(0, CHUNK_SIZE);
            match self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset.x, self.chunk_position.y + chunk_offset.y) {
                Some(chunk) => {
                    chunk.cells[get_cell_index(cell_position.x, cell_position.y)].store(cell);
                },
                None => {},
            };
        }
    }

    pub fn rand_int(&mut self, n: i64) -> i64 {
        rand::thread_rng().gen_range(0..n)
    }
 
    pub fn rand_dir(&self) -> i64 {
        let i = rand::thread_rng().gen_range(0..1000);
        if i%2 == 0 {
            -1
        }
        else {
            1
        }
    }

    pub fn rand_vec(&mut self) -> (i64, i64) {
        let i = self.rand_int(2000);
        match i % 9 {
            0 => (1, 1),
            1 => (1, 0),
            2 => (1, -1),
            3 => (0, -1),
            4 => (-1, -1),
            5 => (-1, 0),
            6 => (-1, 1),
            7 => (0, 1),
            _ => (0, 0),
        }
    }

    pub fn rand_vec_8(&mut self) -> (i64, i64) {
        let i = self.rand_int(8);
        match i {
            0 => (1, 1),
            1 => (1, 0),
            2 => (1, -1),
            3 => (0, -1),
            4 => (-1, -1),
            5 => (-1, 0),
            6 => (-1, 1),
            _ => (0, 1),
        }
    }

    pub fn once_in(&mut self, n: i64) -> bool {
        self.rand_int(n) == 0
    }

    pub fn iter_bit(&self) -> bool {
        self.iter_bit
    }
 }