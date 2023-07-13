use rand::Rng;

use super::cell::*;
use super::elements::Element;

pub struct ChunkApi<'a> {
    x: i32,
    y: i32,
    chunk: &'a mut Chunk,
}

pub struct Chunk {
    width: i32,
    height: i32,
    cells: Vec<Cell>,    
    iter_bit: bool,
}

impl Chunk {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            width: w as i32,
            height: h as i32,
            cells: vec![Cell::default(); (w * h) as usize],
            iter_bit: false,
        }   
    }

    pub fn place(&mut self, x: i32, y: i32, element: Element) {
        if x < 0 || x > self.width - 1 || y < 0 || y > self.height - 1 {
            return;
        }
        let index = self.get_index(x, y);
        if matches!(self.get_cell(x, y).element, Element::Empty) || matches!(element, Element::Empty) {
            self.cells[index] = Cell::new(element);
        }
    }

    fn get_index(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    fn get_cell(&self, x: i32, y: i32) -> Cell {
        let i = self.get_index(x, y);
        return self.cells[i]
    }

    pub fn clear(&mut self) {
        self.cells = vec![Cell::default(); (self.width * self.height) as usize];
    }

    pub fn update(&mut self, dt: f32) {
        self.iter_bit = !self.iter_bit;
        if self.iter_bit {
            for x in 0..self.width {
                for y in (0..self.height).rev() {
                    self.get_cell(x, y).update(ChunkApi { x, y, chunk: self }, dt);
                }
            }
        }
        else {
            for x in (0..self.width).rev() {
                for y in (0..self.height).rev() {
                    self.get_cell(x, y).update(ChunkApi { x, y, chunk: self }, dt);
                }
            }
        }        
    }

    pub fn draw(&self, frame: &mut [u8]) {
        for (cell, pixel) in self.cells.iter().zip(frame.chunks_exact_mut(4)) {
            let rgba = match cell.element {
                Element::Empty => [0x00, 0x00, 0x00, 0xff],
                Element::Stone => [0x77, 0x77, 0x77, 0xff],
                Element::Sand => [0xff, 0xff, 0x00, 0xff],
                Element::Water => [0x00, 0x00, 0xff, 0xff],
            };
            pixel.copy_from_slice(&rgba);
        }
    }
}

impl<'a> ChunkApi<'a> {
    pub fn get(&mut self, dx: i32, dy: i32) -> Cell {
        let nx = self.x + dx;
        let ny = self.y + dy;
        if nx < 0 || nx > self.chunk.width - 1 || ny < 0 || ny > self.chunk.height - 1 {
            return WALL_CELL;
        }
        self.chunk.get_cell(nx, ny)
    }

    pub fn set(&mut self, dx: i32, dy: i32, cell: Cell) {
        let x = self.x + dx;
        let y = self.y + dy;
        if x < 0 || x > self.chunk.width - 1 || y < 0 || y > self.chunk.height - 1 {
            return;
        }
        let index = self.chunk.get_index(x, y);
        self.chunk.cells[index] = cell;
    }

    pub fn swap(&mut self, dx:i32, dy: i32) {
        let i1 = self.chunk.get_index(self.x, self.y);
        let i2 = self.chunk.get_index(self.x + dx, self.y + dy);
        self.chunk.cells.swap(i1, i2);
        self.x += dx;
        self.y += dy;
    }

    pub fn random_float(&self, v1: f32, v2: f32) -> f32 {
        rand::thread_rng().gen_range(v1..v2)
    }
 
    pub fn get_direction(&self) -> i32 {
        let i = rand::thread_rng().gen_range(0..1000);
        if i%2 == 0 {
            -1
        }
        else {
            1
        }
    }

    pub fn get_iter_bit(&self) -> bool {
        self.chunk.iter_bit
    }
 }