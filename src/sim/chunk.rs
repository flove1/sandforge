use std::ops::{AddAssign, SubAssign};
use std::sync::{Arc, Mutex, RwLock};

use rand::{Rng, SeedableRng};
use rapier2d::prelude::{Collider, RigidBodyHandle};

use super::cell::*;
use super::elements::Element;
use super::helpers::get_cell_index;
use super::colliders::{label_matrix, create_polyline_colliders};
use super::world::World;

use crate::vector::Pos2;
use crate::{constants::*, pos2};

#[derive(Default)]
pub struct Chunk {
    placing_queue: Mutex<Vec<(Pos2, Element)>>,
    pub(super) chunk_data: RwLock<ChunkData>,
    pub(super) position: Pos2,
    pub(super) cell_count: Mutex<u64>,
}

#[derive(Default)]
pub struct ChunkData {
    pub(super) cells: Vec<Cell>,
    pub(super) dirty_rect: Rect,
    pub(super) rb_handle: Option<RigidBodyHandle>,
    pub(super) colliders: Vec<(Collider, (f32, f32))>
}
#[derive(Default, Clone, Copy)]
pub struct Rect {
    // x1, y1, x2, y2
    corners: Option<[i32; 4]>,
}

impl Rect {
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    pub fn combine(&mut self, other: Self) {
        if other.is_empty() {
            return;
        }

        if self.is_empty() {
            self.corners = other.corners;
            return;
        }


        let corners_1 = self.corners.as_mut().unwrap();
        let corners_2 = other.corners.unwrap();
        
        if corners_1[0] > corners_2[0] {
            corners_1[0] = corners_2[0];
        }

        if corners_1[1] > corners_2[1] {
            corners_1[1] = corners_2[1];
        }

        if corners_1[2] < corners_2[2] {
            corners_1[2] = corners_2[2];
        }

        if corners_1[3] < corners_2[3] {
            corners_1[3] = corners_2[3];
        }
    }

    pub fn update_position(&mut self, position: &Pos2) {
        if self.is_empty() {
            self.corners = Some([position.x, position.y, position.x+1, position.y+1]);
            return;
        }

        let corners = self.corners.as_mut().unwrap();

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


    pub fn update(&mut self, center_position: &Pos2) {
        let corners_position = [
            &center_position.add(-DIRTY_CHUNK_OFFSET, -DIRTY_CHUNK_OFFSET).clamp(0, CHUNK_SIZE-1),
            &center_position.add(DIRTY_CHUNK_OFFSET, DIRTY_CHUNK_OFFSET).clamp(0, CHUNK_SIZE-1),
        ];

        for position in corners_position {
            self.update_position(position);
        }
    }

    pub fn get_ranges(&self, clock: u8) -> (Vec<i32>, Vec<i32>) {
        match self.corners {
            Some(corners) => {
                return (
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
        }
    }

    pub fn get_ranges_render(&self) -> (Vec<i32>, Vec<i32>) {
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

    pub fn is_empty(&self) -> bool {
        self.corners.is_none()
    }
}

impl  ChunkData {
    //==================
    // Work through api
    //==================

    pub(crate) fn get_cell(&self, cell_position: Pos2) -> Cell {
        self.cells[cell_position.to_index(CHUNK_SIZE)]
    }

    pub(crate) fn match_cell(&self, cell_position: Pos2, element: Element) -> bool {
        self.cells[cell_position.to_index(CHUNK_SIZE)].element == element
    }

    pub(crate) fn set_cell(&mut self, cell_position: Pos2, cell: Cell) {
        self.cells[cell_position.to_index(CHUNK_SIZE)] = cell;
    }

    pub(crate) fn replace_cell(&mut self, cell_position: Pos2, cell: Cell) -> Cell {
        let index = cell_position.to_index(CHUNK_SIZE);
        let replaced_cell = self.cells[index];
        self.cells[index] = cell;
        replaced_cell
    }

    pub(crate) fn swap_cells(&mut self, cell_position_1: Pos2, cell_position_2: Pos2) {
        let index_1 = cell_position_1.to_index(CHUNK_SIZE);
        let index_2 = cell_position_2.to_index(CHUNK_SIZE);

        self.cells.swap(index_1, index_2);
    }

    pub(crate) fn update_dirty_rect(&mut self, position: &Pos2) {
        self.dirty_rect.update(position);
    }

    pub(crate) fn maximize_dirty_rect(&mut self) {
        self.dirty_rect.update_position(&pos2!(0, 0));
        self.dirty_rect.update_position(&pos2!(CHUNK_SIZE-1, CHUNK_SIZE-1));
    }
    
}

impl Chunk {
    pub(crate) fn new(position: Pos2, rb_handle: RigidBodyHandle) -> Self {
        Self {
            chunk_data: RwLock::new(ChunkData {
                cells: vec![Cell::default(); CHUNK_SIZE.pow(2) as usize],
                dirty_rect: Rect::default(),
                colliders: vec![],
                rb_handle: Some(rb_handle)
            }),
            position,
            ..Default::default()
        }
    }

    //================
    // Global methods
    //================

    pub(crate) fn place(&self, x: i32, y: i32, element: Element) {
        self.placing_queue.lock().unwrap().push((pos2!(x, y), element));
    }

    pub(crate) fn place_batch(&self, positions: Vec<(i32, i32)>, element: Element) {
        let mut queue = self.placing_queue.lock().unwrap();
        positions.into_iter()
            .for_each(|pos| {
                queue.push((pos2!(pos.0, pos.1), element));
            });
    }

    pub(crate) fn place_object(&self, cells: Vec<((i32, i32), Cell)>) {
        let mut data = self.chunk_data.write().unwrap();
        let mut count = self.cell_count.lock().unwrap();

        cells.iter()
            .for_each(|(pos, cell)| {
                let index = (pos.1 * CHUNK_SIZE + pos.0) as usize;
                if data.cells[index].element == Element::Empty {
                    count.add_assign(1);
                    data.cells[index] = *cell;
                }
            });
    }

    pub(crate) fn remove_object(&self, cells: Vec<(i32, i32)>) {
        // dbg!("removed", self.position, cells.len());
        let mut data = self.chunk_data.write().unwrap();
        let mut count = self.cell_count.lock().unwrap();

        cells.iter()
            .for_each(|pos| {
                let index = (pos.1 * CHUNK_SIZE + pos.0) as usize;
                if data.cells[index].parent_id.is_some() {
                    if data.cells[index].element != Element::Empty {
                        count.sub_assign(1);
                    }
                    data.cells[index] = EMPTY_CELL;
                }
            });
    }

    pub(crate) fn update_dirty_rect(&self, position: &Pos2) {
        self.chunk_data.write().unwrap().dirty_rect.update(position);
    }

    pub(crate) fn maximize_dirty_rect(&self) {
        let mut data = self.chunk_data.write().unwrap();
        data.dirty_rect.update_position(&pos2!(0, 0));
        data.dirty_rect.update_position(&pos2!(CHUNK_SIZE-1, CHUNK_SIZE-1));
    }
    
    //===========
    // Colliders
    //===========

    pub fn create_colliders(&self) {
        let mut data = self.chunk_data.write().unwrap();

        let mut matrix = vec![0; CHUNK_SIZE.pow(2) as usize];
        let mut label = 0;

        let condition = |index: usize| {
            matches!(data.cells[index].element, Element::Dirt)
        };
        
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let index = get_cell_index(x, y);
                if matrix[index] == 0 && condition(index) {
                    label += 1;
                    label_matrix(x, y, label, &mut matrix, CHUNK_SIZE, &condition);
                }
            }
        }

        data.colliders.clear();
        data.colliders.append(&mut &mut create_polyline_colliders(label, &matrix, CHUNK_SIZE));
    }

    //==========
    // Updating
    //==========
    
    pub(crate) fn process_placing(&self, clock: u8) -> Option<Rect> {
        let mut data = self.chunk_data.write().unwrap();
        let mut queue = self.placing_queue.lock().unwrap();

        if queue.is_empty() && data.dirty_rect.is_empty() {
            return None;
        }

        for (cell_position, element) in queue.drain(..) {
            let index = get_cell_index(cell_position.x, cell_position.y);

            if data.cells[index].element == Element::Empty && element != Element::Empty {
                data.set_cell(cell_position, Cell::new(element, clock.wrapping_sub(4)));
                self.cell_count.lock().unwrap().add_assign(1);
            }
            else if data.cells[index].element != Element::Empty && element == Element::Empty {
                data.set_cell(cell_position, Cell::new(element, clock.wrapping_sub(4)));
                self.cell_count.lock().unwrap().sub_assign(1);
            }

            data.update_dirty_rect(&cell_position);
        }

        Some(data.dirty_rect.retrieve())
    }

    // TODO Rewrite so chunks takes ownership of neighboring chunks' cells up to half chunks size from edge and later returns and combines them
    pub(crate) fn update(&self, manager: Arc<World>, clock: u8) -> u128 {
        let (x_range, y_range) = {
            match self.process_placing(clock) {
                Some(dirty_rect) => {
                    dirty_rect.get_ranges(clock)
                },
                None => {
                    return 0;
                },
            }
        };
        
        self.create_colliders();
        let mut data = self.chunk_data.write().unwrap();
        let mut updated_count: u128 = 0;
        
        let mut api = ChunkApi { 
            cell_position: pos2!(0, 0),
            chunk: self,
            chunk_data: &mut data,
            chunk_manager: manager.clone(),
            clock,
            rng: rand::rngs::SmallRng::from_entropy()
        };

        for x in x_range.iter() {
            for y in y_range.iter().rev() {
                let cell = &api.chunk_data.cells[get_cell_index(*x, *y)];
            
                if cell.element == Element::Empty || cell.parent_id.is_some() {
                    continue;
                }

                if cell.clock == clock {
                    api.chunk_data.dirty_rect.update(&pos2!(*x, *y));
                    continue;
                }

                api.cell_position = pos2!(*x, *y);
                
                api = (*cell).update(api, 0.0, clock);
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
    pub(super) chunk: &'a Chunk,
    pub(super) chunk_data: &'b mut ChunkData,
    pub(super) chunk_manager: Arc<World>,
    pub(super) clock: u8,
    pub(super) rng: rand::rngs::SmallRng,
}

impl<'a, 'b> ChunkApi<'a, 'b> {
    pub fn get(&self, dx: i32, dy: i32) -> Cell {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk_data.get_cell(cell_position)
        }
        else {
            self.chunk_manager.get_cell(self.chunk.position, cell_position)
        }
    }

    pub fn match_element(&self, dx: i32, dy: i32, element: Element) -> bool {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk_data.match_cell(cell_position, element)
        }
        else {
            self.chunk_manager.match_cell(self.chunk.position, cell_position, element)
        }
    }

    pub fn set(&mut self, dx: i32, dy: i32, cell: Cell) {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk_data.set_cell(cell_position, cell);
            self.chunk_data.update_dirty_rect(&cell_position);
        }
        else {
            self.chunk_manager.set_cell(self.chunk.position, cell_position, cell);
        }
    }

    pub fn swap(&mut self, dx:i32, dy: i32) {
        let mut cell_position = self.cell_position;
        let mut new_cell_position = cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            if new_cell_position.is_between(0, CHUNK_SIZE - 1) {
                self.chunk_data.swap_cells(cell_position, new_cell_position);
                self.chunk_data.update_dirty_rect(&new_cell_position);
    
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
                    self.chunk_manager.refresh_chunk(
                        &(self.chunk.position + chunk_offset),
                        &cell_position,
                    );
                }
            }
            else {
                let old_cell = self.chunk_data.get_cell(cell_position);
                let new_cell = self.chunk_manager.replace_cell(self.chunk.position, new_cell_position, old_cell);

                if old_cell.element != Element::Empty && new_cell.element == Element::Empty {
                    self.chunk.cell_count.lock().unwrap().sub_assign(1);
                }
                else if old_cell.element == Element::Empty && new_cell.element != Element::Empty {
                    self.chunk.cell_count.lock().unwrap().add_assign(1);
                }

                self.chunk_data.set_cell(cell_position, new_cell);
            }

            self.chunk_data.update_dirty_rect(&cell_position);
            self.cell_position.change(dx, dy);
        } 

    }

    pub fn update(&mut self, cell: Cell) {
        if self.cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk_data.set_cell(self.cell_position, cell);
        }
        else {
            self.chunk_manager.update_cell(self.chunk.position, self.cell_position, cell);
        }
    }
    
    pub fn rand_int(&mut self, n: i32) -> i32 {
        self.rng.gen_range(0..n)
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