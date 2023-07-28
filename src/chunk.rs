use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock, Mutex, Weak, RwLockReadGuard};
use std::thread;

use rand::Rng;

use super::cell::*;
use super::elements::Element;

pub struct Chunk {
    index: usize,
    cells: Vec<Cell>,    
    iter_bit: bool,    
    size: usize,
}

pub struct ChunkManager {
    chunks: Vec<Chunk>,
    world_size: i64,
    chunk_size: i64,
    maximum_size: usize,
    elements_in_chunk: usize,
}

pub struct PixelToChunkApi<'a> {
    x: i64,
    y: i64,    
    chunk_index: usize,
    chunk_manager: &'a RwLockReadGuard<'a, ChunkManager>,
}

pub struct UserToChunkApi {
    chunk_manager: Arc<RwLock<ChunkManager>>,
}

impl Chunk {
    fn new(element: Element, index: usize, size: usize) -> Self {
        let mut chunk = Self {
            index: index,
            iter_bit: false,
            cells: vec![Cell::default(); size.pow(2)],
            size: size,
        };

        for cell in chunk.cells.iter_mut() {
            cell.element = element;
        };

        chunk
    }

    fn place(&mut self, index: usize, element: Element) {
        if matches!(self.cells[index].element, Element::Empty) || matches!(element, Element::Empty) {
            self.cells[index] = Cell::new(element);
        }
    }

    fn get_index(&self, x: i64, y: i64) -> usize {
        (y * self.size as i64 + x) as usize
    }

    fn get_cell(&self, x: i64, y: i64) -> Cell {
        let i = self.get_index(x, y);
        return self.cells[i]
    }

    fn get_cell_by_index(&self, index: usize) -> Cell {
        return self.cells[index]
    }
}

impl ChunkManager {
    pub fn new(chunk_size: i64, world_size: i64) -> UserToChunkApi {
        let manager = Arc::new(
            RwLock::new(
                Self {
                    chunks: Vec::new(),
                    world_size,
                    chunk_size,
                    maximum_size: (world_size * chunk_size) as usize,
                    elements_in_chunk: chunk_size.pow(2) as usize
                }
            )
        );

        let clone = manager.clone();
        let mut write = clone.write().unwrap();
        
        for index in 0..world_size.pow(2) {
            write.chunks.push(Chunk::new(
                Element::Empty, 
                index as usize, 
                chunk_size as usize,
            ));
        }

        UserToChunkApi{chunk_manager: manager}
    }    

    fn place(&mut self, x: i64, y: i64, element: Element) {
        if x < 0 || x > (self.maximum_size - 1) as i64 || y < 0 || y > (self.maximum_size - 1) as i64 {
            return;
        }

        let index = ((y % self.chunk_size * self.chunk_size) + (x % self.chunk_size)) as usize;
        self.get_chunk(x,y).place(index, element);
    }    

    fn get_chunk(&mut self, x: i64, y: i64) -> &mut Chunk {
        &mut self.chunks[((y / self.chunk_size * self.world_size) + (x / self.chunk_size)) as usize]
    }

    fn get_chunk_by_index(&self, index: usize) -> &Chunk {
        let y = index / (self.elements_in_chunk * self.world_size as usize);
        let x = index % self.maximum_size / self.chunk_size as usize;
        &self.chunks[(y * self.world_size as usize) + x]
    }

    fn switch_chunk(&self, index: usize, mut nx: i64, mut ny:i64) -> (usize, i64, i64) {
        let chunk_offset_x = {
            if nx < 0 { 
                nx = self.chunk_size - 1;
                -1 
            }
            else if nx > (self.chunk_size - 1) { 
                nx = 0;
                1 
            }
            else { 0 }
        };

        let chunk_offset_y = {
            if ny < 0 { 
                ny = self.chunk_size - 1;
                -1 
            }
            else if ny > (self.chunk_size - 1) { 
                ny = 0;
                1 
            }
            else { 0 }
        };
        
        let x_mod = {
            if chunk_offset_x > 0 {
                if (index + chunk_offset_x as usize) % self.world_size as usize == 0 { 0 }
                else { chunk_offset_x }
            }
            else {
                if (index) % self.world_size as usize == 0 { 0 }
                else { chunk_offset_x }
            }
        };
        let y_mod = {
            if chunk_offset_y > 0 {
                if index > (self.world_size * (self.world_size - 1)) as usize { 0 }
                else { chunk_offset_y }
            }
            else {
                if (index) % self.maximum_size == 0 { 0 }
                else { chunk_offset_y }
            }
        };
        let new_index = (index as i64 + (y_mod * self.world_size) + x_mod) as usize;
        (new_index, nx, ny)
    }

    fn get_cell_from_chunk(&self, index: usize) -> Cell {
        let y = index % (self.elements_in_chunk * self.world_size as usize) / self.maximum_size as usize;
        let x = index % self.chunk_size as usize;
        self.get_chunk_by_index(index).get_cell_by_index(y * self.chunk_size as usize + x)
    }
}

impl<'a> PixelToChunkApi<'a> {
    pub fn get(&mut self, dx: i64, dy: i64) -> Cell {
        let mut nx = self.x + dx;
        let mut ny = self.y + dy;
        let mut chunk_index = self.chunk_index;

        if nx < 0 || nx > (self.chunk_manager.chunk_size - 1) as i64 || ny < 0 || ny > (self.chunk_manager.chunk_size - 1) as i64 {
            (chunk_index, nx, ny) = self.chunk_manager.switch_chunk(self.chunk_index, nx, ny);
        }

        self.chunk_manager.chunks[chunk_index].get_cell(nx, ny)
    }

    // pub fn set(&mut self, dx: i64, dy: i64, cell: Cell) {
    //     let x = self.x as i64 + dx;
    //     let y = self.y as i64 + dy;
    //     if x < 0 || x > (self.chunk_manager.chunk_size - 1) as i64 || y < 0 || y > (self.chunk_manager.chunk_size - 1) as i64 {
    //         return;
    //     }
    //     let index = manager.chunks[self.chunk_index].get_index(x, y);
    //     drop(manager);
    //     self.chunk_manager.write().unwrap().chunks[self.chunk_index].cells[index] = cell;
    // }

    // pub fn update(&mut self, cell: Cell) {
    //     let manager = self.chunk_manager.read().unwrap();
    //     let index = manager.chunks[self.chunk_index].get_index(self.x, self.y);
    //     drop(manager);
    //     self.chunk_manager.write().unwrap().chunks[self.chunk_index].cells[index] = cell;
    // }

    // pub fn swap(&mut self, dx:i64, dy: i64) {
    //     let manager = self.chunk_manager.read().unwrap();
    //     let nx = self.x + dx;
    //     let ny = self.y + dy;
    //     let i1 = manager.chunks[self.chunk_index].get_index(self.x, self.y);
    //     if nx < 0 || nx > (manager.chunk_size - 1) as i64 || ny < 0 || ny > (manager.chunk_size - 1) as i64 {
    //         return
            // let chunk_offset_x = {
            //     if nx < 0 { -1 }
            //     else if nx > (manager.chunk_size - 1) { 1 }
            //     else { 0 }
            // };

            // let chunk_offset_y = {
            //     if ny < 0 { -1 }
            //     else if ny > (manager.chunk_size - 1) { 1 }
            //     else { 0 }
            // };

            // let (new_chunk, new_index) = self.get_adjacent_chunk(chunk_offset_x, chunk_offset_y);
            // let i2 = manager.chunks[new_index].get_index(self.x + dx, self.y + dy);
        // }
    //     let i2 = manager.chunks[self.chunk_index].get_index(self.x + dx, self.y + dy);
    //     drop(manager);
    //     self.chunk_manager.write().unwrap().chunks[self.chunk_index].cells.swap(i1, i2);
    //     self.x += dx;
    //     self.y += dy;
    // }

    // pub fn get_adjacent_chunk(&self, x: i64, y:i64) -> (&Chunk, usize) {
    //     let manager = self.chunk_manager.read().unwrap();
    //     let x_mod = {
    //         if x > 0 {
    //             if (self.chunk_index + x as usize) % manager.world_size as usize == 0 { 0 }
    //             else { x as usize }
    //         }
    //         else {
    //             if (self.chunk_index) % manager.world_size as usize == 0 { 0 }
    //             else { x as usize }
    //         }
    //     };
    //     let y_mod = {
    //         if y > 0 {
    //             if self.chunk_index > (manager.world_size * (manager.world_size - 1)) as usize { 0 }
    //             else { y as usize }
    //         }
    //         else {
    //             if (self.chunk_index) % manager.maximum_size == 0 { 9 }
    //             else { self.chunk_index - x as usize }
    //         }
    //     };
    //     let index = self.chunk_index + (y_mod * manager.maximum_size as usize) + x_mod;
    //     (&manager.chunks[self.chunk_index + (y_mod * manager.maximum_size as usize) + x_mod], index)
    // }

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
        self.chunk_manager.chunks[self.chunk_index].iter_bit
    }
 }

impl UserToChunkApi {
    pub fn update(&mut self, dt: f32) {     
        for chunk_offset in 0..4 {
            let manager = self.chunk_manager.read().unwrap();
            let x_range: Vec<i64> = if chunk_offset % 3 == 0 {
                (0..manager.world_size).filter(|x| x%2 == 0).collect()
            }
            else {
                (0..manager.world_size).filter(|x| x%2 == 1).collect()
            };

            let y_range: Vec<i64> = if chunk_offset < 2 {
                (0..manager.world_size).filter(|y| y%2 == 0).collect()
            }
            else {
                (0..manager.world_size).filter(|y| y%2 == 1).collect()
            };

            let mut handles = vec![];
            for x in x_range.iter() {
                for y in y_range.iter() {
                    let handle = thread::spawn({
                        let manager_ref = Arc::clone(&self.chunk_manager);
                        let chunk_index: usize = (y * manager.world_size + x) as usize;
                        
                        move || {
                            let mut writer = manager_ref.write().unwrap();
                            writer.chunks[chunk_index].iter_bit = !writer.chunks[chunk_index].iter_bit;
                            drop(writer);
                            
                            let mut updates: Vec<((i64, i64), Vec<CellAction>)> = vec![];
                            let manager = manager_ref.read().unwrap();
                            let chunk = &manager.chunks[chunk_index];
        
                            if chunk.iter_bit {
                                for x in 0..chunk.size as i64 {
                                    for y in (0..chunk.size as i64).rev() {
                                        let actions = chunk.get_cell(x, y).update(PixelToChunkApi { 
                                            x, 
                                            y, 
                                            chunk_index: chunk_index, 
                                            chunk_manager: &manager,
                                        }, dt);
        
                                        if actions.len() != 0 {
                                            updates.push((
                                                (x, y), 
                                                actions
                                            ));
                                        }
                                    }
                                }
                            }
                            else {
                                for x in (0..chunk.size as i64).rev() {
                                    for y in (0..chunk.size as i64).rev() {
                                        let actions = chunk.get_cell(x, y).update(PixelToChunkApi { 
                                            x, 
                                            y, 
                                            chunk_index: chunk_index, 
                                            chunk_manager: &manager,
                                        }, dt);
        
                                        if actions.len() != 0 {
                                            updates.push((
                                                (x, y), 
                                                actions
                                            ));
                                        }
                                    }
                                }
                            }
        
                            drop(manager);
                            let mut manager = manager_ref.write().unwrap();
        
                            for ((mut x, mut y), actions) in updates {
                                let mut cell_chunk_index = chunk_index;
                                for action in actions {
                                    match action {
                                        CellAction::Swap(dx, dy) => {
                                            let cell_index_1 = manager.chunks[cell_chunk_index].get_index(x, y);
                                            let nx = x + dx;
                                            let ny = y + dy;
                                            if nx < 0 || nx > (manager.chunk_size - 1) as i64 || ny < 0 || ny > (manager.chunk_size - 1) as i64 {
                                                let (new_cell_chunk_index, nx, ny) = manager.switch_chunk(cell_chunk_index, nx, ny);
                                                let cell_index_2 = manager.chunks[new_cell_chunk_index].get_index(nx, ny);
                                                let temp_cell = manager.chunks[cell_chunk_index].cells[cell_index_1];
                                                manager.chunks[cell_chunk_index].cells[cell_index_1] = manager.chunks[new_cell_chunk_index].cells[cell_index_2];
                                                manager.chunks[new_cell_chunk_index].cells[cell_index_2] = temp_cell;
                                                x = nx;
                                                y = ny;
                                                cell_chunk_index = new_cell_chunk_index;
                                            }
                                            else {
                                                let cell_index_2 = manager.chunks[cell_chunk_index].get_index(nx, ny);
                                                manager.chunks[cell_chunk_index].cells.swap(cell_index_1, cell_index_2);
                                                x += dx;
                                                y += dy;
                                            }
                                        },
                                        CellAction::Set(dx, dy, cell) => {
                                            let nx = x + dx;
                                            let ny = y + dy;
                                            if nx < 0 || nx > (manager.chunk_size - 1) as i64 || ny < 0 || ny > (manager.chunk_size - 1) as i64 {
                                                let (new_chunk_index, nx, ny) = manager.switch_chunk(cell_chunk_index, nx, ny);
                                                let index = manager.chunks[new_chunk_index].get_index(nx, ny);
                                                manager.chunks[new_chunk_index].cells[index] = cell;
                                            }
                                            else {
                                                let index = manager.chunks[cell_chunk_index].get_index(nx, ny);
                                                manager.chunks[cell_chunk_index].cells[index] = cell;
                                            }
                                        },
                                        CellAction::Update(cell) => {
                                            let index = manager.chunks[cell_chunk_index].get_index(x, y);
                                            manager.chunks[cell_chunk_index].cells[index] = cell;
                                        },
                                    }
                                }
                            }
                        }}
                    );
        
                    handles.push(handle);
                }
            }

            drop(manager);
            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    pub fn place(&mut self, x: i64, y: i64, element: Element) {
        self.chunk_manager.write().unwrap().place(x, y, element);
    }    
    
    pub fn render(&self, frame: &mut [u8]) {
        let manager = self.chunk_manager.read().unwrap();

        for (index, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let cell = manager.get_cell_from_chunk(index);
            let offset = rand::thread_rng().gen_range(0..25);
            let rgba = match cell.element {
                Element::Empty => [0x00, 0x00, 0x00, 0xff],
                Element::Stone => [0x77, 0x77, 0x77, 0xff],
                Element::Sand => [0xf2_u8.saturating_add(cell.ra), 0xf1_u8.saturating_add(cell.ra), 0xa3_u8.saturating_add(cell.ra), 0xff],
                Element::Water => [0x47 + offset, 0x7C + offset, 0xB8 + offset, 0xff],
            };
            pixel.copy_from_slice(&rgba);
        }
    }
}