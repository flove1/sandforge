use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::{thread, vec};

use rand::Rng;

use super::cell::*;
use super::elements::Element;

pub struct Chunk {
    index: usize,
    cells: Vec<Cell>,
}

pub struct ChunkManager {
    chunks: Vec<Chunk>,
    iter_bit: bool,    
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
    fn new(index: usize, size: usize) -> Self {
        Self {
            index,
            cells: vec![Cell::default(); size.pow(2)],
        }
    }

    fn place(&mut self, index: usize, element: Element) {
        if matches!(self.cells[index].element, Element::Empty) || matches!(element, Element::Empty) {
            self.cells[index] = Cell::new(element);
        }
    }
}

impl ChunkManager {
    pub fn new(chunk_size: i64, world_size: i64) -> UserToChunkApi {
        let manager = Arc::new(
            RwLock::new(
                Self {
                    chunks: vec![],
                    iter_bit: false,
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
        self.get_chunk_mut(x,y).place(index, element);
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

    fn get_chunk_by_pixel(&self, index: usize) -> &Chunk {
        let y = index / (self.elements_in_chunk * self.world_size as usize);
        let x = index % self.maximum_size / self.chunk_size as usize;
        &self.chunks[(y * self.world_size as usize) + x]
    }

    fn get_cell_from_pixel_index(&self, index: usize) -> &Cell {
        let y = index % (self.elements_in_chunk * self.world_size as usize) / self.maximum_size;
        let x = index % self.chunk_size as usize;
        &self.get_chunk_by_pixel(index).cells[self.get_cell_index(x as i64, y as i64)]
    }

    fn get_cell_mut(&mut self , chunk_index: usize, x: i64, y: i64) -> &mut Cell{
        let index = self.get_cell_index(x, y);
        &mut self.chunks[chunk_index].cells[index]
    }

    fn get_chunk_mut(&mut self, x: i64, y: i64) -> &mut Chunk {
        &mut self.chunks[((y / self.chunk_size * self.world_size) + (x / self.chunk_size)) as usize]
    }

    fn get_cell_index(&self, x: i64, y: i64) -> usize {
        (y * self.chunk_size + x) as usize
    }
}

impl<'a> PixelToChunkApi<'a> {
    pub fn get(&mut self, dx: i64, dy: i64) -> &Cell {
        let mut nx = self.x + dx;
        let mut ny = self.y + dy;
        let mut chunk_index = self.chunk_index;

        if nx < 0 || nx > (self.chunk_manager.chunk_size - 1) as i64 || ny < 0 || ny > (self.chunk_manager.chunk_size - 1) as i64 {
            (chunk_index, nx, ny) = self.chunk_manager.switch_chunk(self.chunk_index, nx, ny);
        }

        &self.chunk_manager.chunks[chunk_index].cells[self.chunk_manager.get_cell_index(nx, ny)]
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
        self.chunk_manager.iter_bit
    }
 }

impl UserToChunkApi {
    pub fn update(&mut self, dt: f32) {
        {
            let mut writer = self.chunk_manager.write().unwrap();
            writer.iter_bit = !writer.iter_bit;
        }

        //separate chunks in squares and process them one by one, so there is no concurency during moving pixels between chunks
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
                    let handle = UserToChunkApi::update_chunk(
                        dt, 
                        ((y * manager.world_size) + x) as usize, 
                        Arc::clone(&self.chunk_manager)
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

    fn update_chunk(dt: f32, chunk_index: usize, manager_ref: Arc<RwLock<ChunkManager>>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let cell_updates = UserToChunkApi::get_cell_updates(dt, chunk_index, &manager_ref);  
            UserToChunkApi::process_cell_updates(chunk_index, cell_updates, &manager_ref);
        })
    }
    
    fn get_cell_updates(dt: f32, chunk_index: usize, manager_ref: &Arc<RwLock<ChunkManager>>) -> Vec<((i64, i64), Vec<CellAction>)> {
        let mut cell_updates: Vec<((i64, i64), Vec<CellAction>)> = vec![];
        let manager = manager_ref.read().unwrap();
        let chunk = &manager.chunks[chunk_index];

        for x in 0..manager.chunk_size {
            for y in 0..manager.chunk_size {
                let cell = chunk.cells[manager.get_cell_index(x, y)];
                if cell.iter_bit == manager.iter_bit {
                    let actions = cell.update(PixelToChunkApi { 
                        x: x, 
                        y: y, 
                        chunk_index: chunk_index, 
                        chunk_manager: &manager,
                    }, dt);
    
                    if actions.len() != 0 {
                        cell_updates.push((
                            (x, y), 
                            actions
                        ));
                    }
                }
            }
        }

        cell_updates
    }

    fn process_cell_updates(chunk_index: usize, cell_updates: Vec<((i64, i64), Vec<CellAction>)>, manager_ref: &Arc<RwLock<ChunkManager>>) {
        let mut manager = manager_ref.write().unwrap();

        for ((mut x, mut y), actions) in cell_updates {
            let mut cell_chunk_index = chunk_index;
            for action in actions {
                match action {
                    CellAction::Swap(dx, dy) => {
                        let cell_index_1 = manager.get_cell_index(x, y);
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || nx > (manager.chunk_size - 1) as i64 || ny < 0 || ny > (manager.chunk_size - 1) as i64 {
                            let (new_cell_chunk_index, nx, ny) = manager.switch_chunk(cell_chunk_index, nx, ny);
                            let cell_index_2 = manager.get_cell_index(nx, ny);
                            let temp_cell = manager.chunks[cell_chunk_index].cells[cell_index_1];
                            manager.chunks[cell_chunk_index].cells[cell_index_1] = manager.chunks[new_cell_chunk_index].cells[cell_index_2];
                            manager.chunks[new_cell_chunk_index].cells[cell_index_2] = temp_cell;
                            x = nx;
                            y = ny;
                            cell_chunk_index = new_cell_chunk_index;
                        }
                        else {
                            let cell_index_2 = manager.get_cell_index(nx, ny);
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
                            let index = manager.get_cell_index(nx, ny);
                            manager
                                .chunks[new_chunk_index]
                                .cells[index] = cell;
                        }
                        else {
                            let index = manager.get_cell_index(nx, ny);
                            manager
                                .chunks[cell_chunk_index]
                                .cells[index] = cell;
                        }
                    },
                    CellAction::Update(cell) => {
                        let index = manager.get_cell_index(x, y);
                        manager
                            .chunks[cell_chunk_index]
                            .cells[index] = cell;
                    },
                }
            }
            let index = manager.get_cell_index(x, y);
            manager
                .chunks[cell_chunk_index]
                .cells[index]
                .iter_bit = !manager.iter_bit;
        }
    }

    pub fn render(&self, frame: &mut [u8]) {
        let manager = self.chunk_manager.read().unwrap();
        
        let mut glowing_pixels: Vec<(usize, &Cell)> = vec![];

        for (index, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let cell = manager.get_cell_from_pixel_index(index);
            let offset = rand::thread_rng().gen_range(0..25);
            let rgba = match cell.element {
                Element::Empty => [0x00, 0x00, 0x00, 0xff],
                Element::Stone => [0x77, 0x77, 0x77, 0xff],
                Element::Sand => [0xf2_u8.saturating_add(cell.ra), 0xf1_u8.saturating_add(cell.ra), 0xa3_u8.saturating_add(cell.ra), 0xff],
                Element::Water => [0x47 + offset, 0x7C + offset, 0xB8 + offset, 0xff],
                Element::GlowingSand => {
                    glowing_pixels.push((index, cell));
                    [0xe8, 0x6a, 0x17, 0xff]
                },
            };
            pixel.copy_from_slice(&rgba);
        }

        for (index, _) in glowing_pixels {
            let y = (index / manager.maximum_size) as i64;
            let x = (index % manager.maximum_size) as i64;
            let range = 0..(manager.maximum_size as i64);
            for offset_x in -4..5 {
                for offset_y in -4..5 {
                    if offset_x == 0 && offset_y == 0 {
                        continue;
                    }

                    let affected_pixel_x = x + offset_x; 
                    let affected_pixel_y = y + offset_y; 
                    if !range.contains(&affected_pixel_x) || !range.contains(&affected_pixel_y) {
                        continue;
                    }
                    
                    let intensity = (1.0 / (offset_x.abs() + offset_y.abs()) as f32).clamp(0.0, 0.2);
                    
                    let index = ((affected_pixel_y * (manager.maximum_size) as i64 + affected_pixel_x) * 4) as usize;
                    frame[index] = ((frame[index] as f32 * (1.0 - intensity) + 0xe8 as f32 * (intensity)) / 2.0) as u8;
                    frame[index + 1] = ((frame[index + 1] as f32 * (1.0 - intensity) + 0x6a as f32 * (intensity)) / 2.0) as u8;
                    frame[index + 2] = ((frame[index + 2] as f32 * (1.0 - intensity) + 0x17 as f32 * (intensity)) / 2.0) as u8;
                }
            }
        }
    }
}