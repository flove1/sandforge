use std::sync::Arc;

use crossbeam::atomic::AtomicCell;
use rand::Rng;

use super::cell::*;
use super::elements::Element;
use super::helpers::get_cell_index;
use super::world::World;

use crate::{constants::*, vec2};
use crate::vector::Vector2;

pub struct Chunk {
    pub(super) cells: Vec<AtomicCell<Cell>>,
}

impl Chunk {
    pub(crate) fn new() -> Self {
        let mut chunk = Self {
            cells: Vec::with_capacity(CHUNK_SIZE.pow(2) as usize),
        };

        for _ in 0..(CHUNK_SIZE.pow(2)) {
            chunk.cells.push(AtomicCell::new(Cell::default()))
        }

        chunk
    }

    pub(crate) fn place(&self, index: i64, element: Element) {
        if matches!(self.cells[index as usize].load().element, Element::Empty) || matches!(element, Element::Empty) {
            self.cells[index as usize].store(Cell::new(element));
        }
    }

    pub(crate) fn update(&self, position: Vector2, manager: Arc<World>, iter_bit: bool) {
        let x_range: Vec<i64> = if iter_bit {(0..CHUNK_SIZE).collect()} else {(0..CHUNK_SIZE).rev().collect()};
        for x in x_range {
            for y in 0..CHUNK_SIZE {
                let cell = self.cells[get_cell_index(x, y) as usize].load();
                if cell.iter_bit == iter_bit {
                    cell.update(ChunkApi { 
                        cell_position: vec2!(x, y),
                        chunk_position: position,
                        chunk_manager: &manager,
                        iter_bit,
                    }, 0.0);
                }
            }
        }
    }
}

pub struct ChunkApi<'a> {
    pub(super) cell_position: Vector2,
    pub(super) chunk_position: Vector2,
    pub(super) chunk_manager: &'a Arc<World>,
    pub(super) iter_bit: bool,
}

impl<'a> ChunkApi<'a> {
    pub fn get(&mut self, dx: i64, dy: i64) -> Cell {
        let mut cell_position = vec2!(self.cell_position.x + dx, self.cell_position.y + dy);

        let result = {
            if !cell_position.is_between(0, CHUNK_SIZE) {
                let chunk_offset = cell_position.wrap_and_return_offset(0, CHUNK_SIZE);
                self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset.x, self.chunk_position.y + chunk_offset.y)
            }
            else {
                self.chunk_manager.get_chunk(self.chunk_position.x, self.chunk_position.y)
            }
        };

        if let Some(chunk) = result {
            chunk.cells[get_cell_index(cell_position.x, cell_position.y)].load()
        }
        else {
            EMPTY_CELL
        }
    }

    pub fn match_element(&mut self, dx: i64, dy: i64, element: Element) -> bool {
        let mut cell_position = vec2!(self.cell_position.x + dx, self.cell_position.y + dy);

        let result = {
            if !cell_position.is_between(0, CHUNK_SIZE) {
                let chunk_offset = cell_position.wrap_and_return_offset(0, CHUNK_SIZE);
                self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset.x, self.chunk_position.y + chunk_offset.y)
            }
            else {
                self.chunk_manager.get_chunk(self.chunk_position.x, self.chunk_position.y)
            }
        };

        if let Some(chunk) = result {
            chunk.cells[get_cell_index(cell_position.x, cell_position.y)].load().element == element
        }
        else if element == Element::Empty {
            true   
        }
        else {
            false
        }
    }

    pub fn set(&mut self, dx: i64, dy: i64, cell: Cell) {
        let mut cell_position = vec2!(self.cell_position.x + dx, self.cell_position.y + dy);

        let result = {
            if !cell_position.is_between(0, CHUNK_SIZE) {
                let chunk_offset = cell_position.wrap_and_return_offset(0, CHUNK_SIZE);
                self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset.x, self.chunk_position.y + chunk_offset.y)
            }
            else {
                self.chunk_manager.get_chunk(self.chunk_position.x, self.chunk_position.y)
            }
        };

        if let Some(chunk) = result {
            let cell_index = get_cell_index(cell_position.x, cell_position.y);
            chunk.cells[cell_index].store(cell);
        }
    }

    pub fn swap(&mut self, dx:i64, dy: i64) {
        let mut cell_position = vec2!(self.cell_position.x + dx, self.cell_position.y + dy);
        
        if !cell_position.is_between(0, CHUNK_SIZE) {
            let chunk_offset = cell_position.wrap_and_return_offset(0, CHUNK_SIZE);

            let cell_index_1 = get_cell_index(self.cell_position.x, self.cell_position.y);
            let cell_index_2 = get_cell_index(cell_position.x, cell_position.y);

            let chunk = self.chunk_manager.chunks.get(&self.chunk_position).unwrap();
            let result = self.chunk_manager.get_chunk(self.chunk_position.x + chunk_offset.x, self.chunk_position.y + chunk_offset.y);

            if let Some(new_chunk) = result {
                chunk.cells[cell_index_1].store(new_chunk.cells[cell_index_2].swap(chunk.cells[cell_index_1].load()));
            }
            else {
                chunk.cells[cell_index_1].store(EMPTY_CELL);
            }
            self.cell_position = cell_position;
            self.chunk_position.x += chunk_offset.x;
            self.chunk_position.y += chunk_offset.y;
        }
        else {
            let cell_index_1 = get_cell_index(self.cell_position.x, self.cell_position.y);
            let cell_index_2 = get_cell_index(cell_position.x, cell_position.y);

            let chunk = self.chunk_manager.chunks.get(&self.chunk_position).unwrap();
            chunk.cells[cell_index_1].store(chunk.cells[cell_index_2].swap(chunk.cells[cell_index_1].load()));
            self.cell_position = cell_position;
        }
    }

    pub fn update(&mut self, cell: Cell) {
        let cell_index = get_cell_index(self.cell_position.x, self.cell_position.y);
        let result = self.chunk_manager.chunks.get(&self.chunk_position);
        if let Some(chunk) = result {
            chunk.cells[cell_index as usize].store(cell);
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