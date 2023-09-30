use rapier2d::prelude::{Collider, RigidBodyHandle};

use super::cell::*;
use super::elements::{MatterType, Element};
use super::helpers::get_cell_index;
use super::colliders::{label_matrix, create_polyline_colliders};
use super::world::World;

use crate::vector::Pos2;
use crate::{constants::*, pos2};

#[derive(Default)]
pub struct Chunk {
    pub(super) position: Pos2,

    pub(super) cells: Vec<Cell>,
    pub(super) dirty_rect: Option<[i32; 4]>,
    pub(super) placing_queue: Vec<(Pos2, Cell)>,
    pub(super) cell_count: u64,
    
    pub(super) rb_handle: RigidBodyHandle,
    pub(super) colliders: Vec<(Collider, (f32, f32))>,
    pub(super) texture: Option<wgpu::Texture>,
}

impl Chunk {
    pub(crate) fn new(position: Pos2, rb_handle: RigidBodyHandle) -> Self {
        Self {
            position,
            cells: vec![Cell::default(); CHUNK_SIZE.pow(2) as usize],
            dirty_rect: None,
            placing_queue: vec![],
            cell_count: 0,
    
            rb_handle,
            colliders: vec![], 
            texture: None,
        }
    }

    //================
    // Global methods
    //================

    pub fn place(&mut self, x: i32, y: i32, cell: Cell) {
        self.placing_queue.push((pos2!(x, y), cell));
    }

    pub fn place_batch(&mut self, positions: Vec<((i32, i32), Cell)>) {
        positions.into_iter()
            .for_each(|(pos, cell)| {
                self.placing_queue.push((pos2!(pos.0, pos.1), cell));
            });
    }

    pub fn is_dirty_rect_empty(&self) -> bool {
        self.dirty_rect.is_none()
    }

    pub fn retrieve_dirt_rect(&mut self, clock: u8) -> (Vec<i32>, Vec<i32>) {
        let positions = match self.dirty_rect {
            Some(corners) => {
                (
                    if clock % 2 == 0 {
                        (corners[0]..corners[2]).collect()
                    }
                    else {
                        (corners[0]..corners[2]).rev().collect()
                    },
                    (corners[1]..corners[3]).collect()
                )
            },
            None => panic!(),
        };

        self.dirty_rect = None;

        positions
    }

    pub fn get_ranges_render(&self) -> (Vec<i32>, Vec<i32>) {
        match self.dirty_rect {
            Some(corners) => {
                ((corners[0]..corners[2]).collect(), (corners[1]..corners[3]).collect())
            },
            None => (vec![], vec![]),
        }
    }

    //============
    // API access
    //============

    pub(crate) fn get_cell(&self, cell_position: Pos2) -> Cell {
        self.cells.get(cell_position.to_index(CHUNK_SIZE)).unwrap_or(&Cell::default()).clone()
    }

    pub(crate) fn match_cell(&self, cell_position: Pos2, element: &Element) -> bool {
        self.cells[cell_position.to_index(CHUNK_SIZE)].element == *element
    }

    pub(crate) fn set_cell(&mut self, cell_position: Pos2, cell: Cell) {
        self.cells[cell_position.to_index(CHUNK_SIZE)] = cell;
    }

    pub(crate) fn replace_cell(&mut self, cell_position: Pos2, cell: Cell) -> Cell {
        let index = cell_position.to_index(CHUNK_SIZE);
        let replaced_cell = self.cells[index].clone();
        self.cells[index] = cell;
        replaced_cell
    }

    pub(crate) fn swap_cells(&mut self, cell_position_1: Pos2, cell_position_2: Pos2) {
        let index_1 = cell_position_1.to_index(CHUNK_SIZE);
        let index_2 = cell_position_2.to_index(CHUNK_SIZE);

        self.cells.swap(index_1, index_2);
    }
    

    pub fn update_dirty_rect_with_offset(&mut self, position: &Pos2) {
        let corners_position = [
            &position.add(-DIRTY_CHUNK_OFFSET, -DIRTY_CHUNK_OFFSET).clamp(0, CHUNK_SIZE-1),
            &position.add(DIRTY_CHUNK_OFFSET, DIRTY_CHUNK_OFFSET).clamp(0, CHUNK_SIZE-1),
        ];

        for position in corners_position {
            self.update_dirty_rect(position);
        }
    }

    pub fn update_dirty_rect(&mut self, position: &Pos2) {
        if self.is_dirty_rect_empty() {
            self.dirty_rect = Some([position.x, position.y, position.x+1, position.y+1]);
            return;
        }

        let corners = self.dirty_rect.as_mut().unwrap();

        if corners[0] > position.x {
            corners[0] = position.x;
        }

        if corners[1] > position.y {
            corners[1] = position.y;
        }
        
        if corners[2] < position.x+1 {
            corners[2] = position.x+1;
        }
        
        if corners[3] < position.y+1 {
            corners[3] = position.y+1;
        }
    }

    pub fn maximize_dirty_rect(&mut self) {
        self.update_dirty_rect(&pos2!(0, 0));
        self.update_dirty_rect(&pos2!(CHUNK_SIZE-1, CHUNK_SIZE-1));
    }

    //===========
    // Colliders
    //===========

    pub fn create_colliders(&mut self) {
        let mut matrix = vec![0; CHUNK_SIZE.pow(2) as usize];
        let mut label = 0;

        let condition = |index: usize| {
            matches!(self.cells[index].element.matter, MatterType::Static { .. }) && !matches!(self.cells[index].simulation, SimulationType::RigidBody(..))
        };
        
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let index = get_cell_index(x, y);
                if matrix[index] == 0 && condition(index) {
                    label += 1;
                    label_matrix(x, y, label, &mut matrix, CHUNK_SIZE, CHUNK_SIZE, &condition);
                }
            }
        }

        self.colliders.clear();
        self.colliders.append(&mut &mut create_polyline_colliders(label, &matrix, CHUNK_SIZE));
    }

    //==========
    // Updating
    //==========

    pub fn create_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut pixel_data: Vec<u8> = Vec::with_capacity(self.cells.len());
        let size = (self.cells.len() as f32).sqrt().trunc() as u32;

        self.cells.iter()
            .for_each(|cell| pixel_data.extend(&cell.get_color()));

        let extent = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };
        
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Chunk Texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb, // Adjust format as needed
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            // Tells wgpu where to copy the pixel data
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // The actual pixel data
            &pixel_data,
            // The layout of the texture
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            extent,
        );

        self.texture = Some(texture);
    }
    
    pub(crate) fn process_placing(&mut self, clock: u8) {
        if self.placing_queue.is_empty() && self.is_dirty_rect_empty() {
            return;
        }

        self.placing_queue.drain(..)
            .collect::<Vec<(Pos2, Cell)>>().into_iter()
            .for_each(|(cell_position, mut cell)| {
                let index = get_cell_index(cell_position.x, cell_position.y);
                cell.clock = clock.wrapping_add(4);
    
                if self.cells[index].element.matter == MatterType::Empty {
                    if cell.element.matter != MatterType::Empty {
                        self.cells[index] = cell;
                        self.cell_count += 1;
                    }
                }
                else if self.cells[index].element.matter != MatterType::Empty && !matches!(self.cells[index].simulation, SimulationType::RigidBody( .. )) {
                    if cell.element.matter == MatterType::Empty {
                        self.cell_count -= 1;
                        self.cells[index] = cell;
                    }
                }
    
                self.update_dirty_rect(&cell_position);
            });
    }

    // TODO Rewrite so chunks takes ownership of neighboring chunks' cells up to half chunks size from edge and later returns and combines them
    pub(crate) fn update(&mut self, world: &World, clock: u8) -> u128 {
        self.process_placing(clock);

        if self.is_dirty_rect_empty() {
            return 0;
        }
        
        let mut updated_count: u128 = 0;
        let (x_range, y_range) = self.retrieve_dirt_rect(clock);

        for x in x_range.iter() {
            for y in y_range.iter() {
                let cell = &self.cells[get_cell_index(*x, *y)];
            
                if cell.element.matter == MatterType::Empty {
                    continue;
                }

                if cell.clock == clock {
                    self.update_dirty_rect(&pos2!(*x, *y));
                    continue;
                }

                cell.clone().update_cell(
                    &mut ChunkApi { 
                        cell_position: pos2!(*x, *y),
                        chunk: self,
                        world,
                        clock,
                    }, 
                    0.0, 
                    clock
                );

                updated_count += 1;
            }
        }

        return updated_count;
    }
}

//========================================================
// API to allow cells to easily interact with other cells
//========================================================

pub struct ChunkApi<'a, 'b> {
    pub(super) cell_position: Pos2,
    pub(super) chunk: &'a mut Chunk,
    pub(super) world: &'b World,
    pub(super) clock: u8,
}

impl<'a, 'b> ChunkApi<'a, 'b> {
    pub fn get(&self, dx: i32, dy: i32) -> Cell {
        let cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.get_cell(cell_position)
        }
        else {
            self.world.get_cell(self.chunk.position, cell_position)
        }
    }

    pub fn match_element(&self, dx: i32, dy: i32, element: &Element) -> bool {
        let cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.match_cell(cell_position, &element)
        }
        else {
            self.world.match_cell(self.chunk.position, cell_position, element)
        }
    }

    pub fn set(&mut self, dx: i32, dy: i32, cell: Cell) {
        let cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.set_cell(cell_position, cell);
            self.chunk.update_dirty_rect(&cell_position);
        }
        else {
            self.world.set_cell(self.chunk.position, cell_position, cell);
        }
    }

    pub fn swap(&mut self, dx:i32, dy: i32) {
        let cell_position = self.cell_position;
        let new_cell_position = cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            if new_cell_position.is_between(0, CHUNK_SIZE - 1) {
                self.chunk.swap_cells(cell_position, new_cell_position);
                self.chunk.update_dirty_rect(&new_cell_position);
    
                // Update chunks if cell is updated close to their border
                let chunk_offset = pos2!(
                    if cell_position.x == 0 { -1 }
                    else if cell_position.x == CHUNK_SIZE - 1 { 1 }
                    else { 0 },
    
                    if cell_position.y == 0 { -1 }
                    else if cell_position.y == CHUNK_SIZE - 1 { 1 }
                    else { 0 }
                );
    
                if !chunk_offset.is_zero() {
                    let (cell_position, _) = (cell_position + chunk_offset).wrap(0, CHUNK_SIZE);
                    self.world.refresh_chunk(
                        &(self.chunk.position + chunk_offset),
                        &cell_position,
                    );
                }
            }
            else {
                let old_cell = self.chunk.get_cell(cell_position);
                let new_cell = self.world.replace_cell(self.chunk.position, new_cell_position, old_cell.clone());

                if old_cell.element.matter != MatterType::Empty && new_cell.element.matter == MatterType::Empty {
                    self.chunk.cell_count -= 1;
                }
                else if old_cell.element.matter == MatterType::Empty && new_cell.element.matter != MatterType::Empty {
                    self.chunk.cell_count += 1;
                }

                self.chunk.set_cell(cell_position, new_cell);
            }

            self.chunk.update_dirty_rect(&cell_position);
            self.cell_position.change(dx, dy);
        } 

    }

    pub fn keep_alive(&mut self, dx: i32, dy: i32) {
        self.chunk.update_dirty_rect(&self.cell_position.add(dx, dy));
    }

    pub fn update(&mut self, cell: Cell) {
        if self.cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.set_cell(self.cell_position, cell);
        }
        else {
            self.world.update_cell(self.chunk.position, self.cell_position, cell);
        }
    }
    
    pub fn rand_int(&mut self, n: i32) -> i32 {
        fastrand::i32(0..n)
    }
 
    pub fn rand_dir(&mut self) -> i32 {
        let i = self.rand_int(1000);
        if i%2 == 0 {
            -1
        }
        else {
            1
        }
    }

    pub fn rand_vec(&mut self) -> (i32, i32) {
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

    pub fn rand_vec_8(&mut self) -> (i32, i32) {
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

    pub fn once_in(&mut self, n: i32) -> bool {
        self.rand_int(n) == 0
    }
 }