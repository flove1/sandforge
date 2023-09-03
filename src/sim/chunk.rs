use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicU8};
use std::sync::Arc;

use egui_winit::egui::epaint::ahash::HashSet;
use parking_lot::{Mutex, MutexGuard};
use rand::Rng;

use super::cell::*;
use super::elements::Element;
use super::helpers::get_cell_index;
use super::objects::marching_squares;
use super::world::World;

use crate::{constants::*, vec2};
use crate::vector::Vector2;

#[derive(Default)]
pub struct Chunk {
    placing_queue: Mutex<VecDeque<(Vector2, Element)>>,
    pub(super) chunk_data: Mutex<ChunkData>,
    pub(super) position: Vector2,
    pub(super) frame_idling: AtomicU8,
    pub(super) cell_count: AtomicU64,
}


#[derive(Default)]
pub struct ChunkData {
    pub(super) cells: Vec<Cell>,
    pub(super) dirty_rect: Rect,
    pub(super) objects: Vec<Vec<(f64, f64)>>
}
#[derive(Default, Clone, Copy)]
pub struct Rect {
    // x1, y1, x2, y2
    corners: Option<[i64; 4]>,
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

    pub fn update(&mut self, position: &Vector2) {
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


    pub fn update_at_corners(&mut self, center_position: &Vector2) {
        let corners_position = [
            &center_position.add(-DIRTY_CHUNK_OFFSET, -DIRTY_CHUNK_OFFSET).clamp(0, CHUNK_SIZE-1),
            &center_position.add(DIRTY_CHUNK_OFFSET, DIRTY_CHUNK_OFFSET).clamp(0, CHUNK_SIZE-1),
        ];

        for position in corners_position {
            self.update(position);
        }
    }

    pub fn get_ranges(&self, clock: u8) -> (Vec<i64>, Vec<i64>) {
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

    pub fn is_empty(&self) -> bool {
        self.corners.is_none()
    }
}

impl  ChunkData {
    //==================
    // Work through api
    //==================

    pub(crate) fn get_cell(&self, cell_position: Vector2) -> Cell {
        self.cells[cell_position.to_index(CHUNK_SIZE)]
    }

    pub(crate) fn set_cell(&mut self, cell_position: Vector2, cell: Cell) {
        self.cells[cell_position.to_index(CHUNK_SIZE)] = cell;
    }

    pub(crate) fn replace_cell(&mut self, cell_position: Vector2, cell: Cell) -> Cell {
        let index = cell_position.to_index(CHUNK_SIZE);
        let replaced_cell = self.cells[index];
        self.cells[index] = cell;
        replaced_cell
    }

    pub(crate) fn swap_cells(&mut self, cell_position_1: Vector2, cell_position_2: Vector2) {
        let index_1 = cell_position_1.to_index(CHUNK_SIZE);
        let index_2 = cell_position_2.to_index(CHUNK_SIZE);

        self.cells.swap(index_1, index_2);
    }

    pub(crate) fn update_dirty_rect(&mut self, position: &Vector2) {
        self.dirty_rect.update_at_corners(position);
    }

    pub(crate) fn maximize_dirty_rect(&mut self) {
        self.dirty_rect.update(&vec2!(0, 0));
        self.dirty_rect.update(&vec2!(CHUNK_SIZE-1, CHUNK_SIZE-1));
    }
    
}

impl Chunk {
    pub(crate) fn new(position: Vector2) -> Self {
        Self {
            chunk_data: Mutex::new(ChunkData {
                cells: vec![Cell::default(); CHUNK_SIZE.pow(2) as usize],
                dirty_rect: Rect::default(),
                objects: vec![],
            }),
            placing_queue: Mutex::new(VecDeque::new()),
            position,
            ..Default::default()
        }
    }

    //================
    // Global methods
    //================

    pub(crate) fn place(&self, x: i64, y: i64, element: Element) {
        let mut queue = self.placing_queue.lock();
        queue.push_back((vec2!(x, y), element));
    }

    pub(crate) fn update_dirty_rect(&self, position: &Vector2) {
        self.chunk_data.lock().dirty_rect.update_at_corners(position);
    }

    pub(crate) fn maximize_dirty_rect(&self) {
        let mut data = self.chunk_data.lock();
        data.dirty_rect.update(&vec2!(0, 0));
        data.dirty_rect.update(&vec2!(CHUNK_SIZE-1, CHUNK_SIZE-1));
    }
    
    //===========
    // Colliders
    //===========

    fn label_cell<'a>(&self, x: i64, y: i64, label: i32, labeled_cells: &mut Vec<i32>, data: &MutexGuard<'a, ChunkData>) {
        if x < 0 || x > CHUNK_SIZE - 1 || y < 0 || y > CHUNK_SIZE - 1 {
            return;
        }
    
        let index = get_cell_index(x, y);
        if labeled_cells[index] != 0 || data.cells[index].element != Element::Wood {
            return;
        }
    
        labeled_cells[index] = label;
    
        self.label_cell(x + 1, y, label, labeled_cells, data);
        self.label_cell(x - 1, y, label, labeled_cells, data);
        self.label_cell(x, y + 1, label, labeled_cells, data);
        self.label_cell(x, y - 1, label, labeled_cells, data);
    }

    pub fn create_collider(&self) {
        let mut data = self.chunk_data.lock();

        let mut labeled_cells = vec![0; CHUNK_SIZE.pow(2) as usize];
        let mut label = 0;
        
        // Connected-component labeling
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let index = get_cell_index(x, y);
                if labeled_cells[index] == 0 && data.cells[get_cell_index(x, y)].element == Element::Wood {
                    label += 1;
                    self.label_cell(x, y, label, &mut labeled_cells, &data);
                }
            }
        }

        data.objects.clear();
        data.objects.append(&mut marching_squares(label, &mut labeled_cells, CHUNK_SIZE));
        
        // let mut contours: Vec<Vec<Point>> = vec![vec![]; label];
        // let mut figures: Vec<Vec<Point>> = vec![vec![]; label];
        
        // Marching squares algorithm
        // for x in 0..CHUNK_SIZE {
        //     for y in 0..CHUNK_SIZE {
        //         let index = get_cell_index(x, y);
        //         if labeled_cells[index] != 0 {
        //             figures[labeled_cells[index] - 1].push(Point { x: x as f64, y: y as f64 });
        //             if self.cell_is_contour(x, y, &mut labeled_cells) {
        //                 contours[labeled_cells[index] - 1].push(Point { x: x as f64, y: y as f64 });
        //             }
        //         }
        //     }
        // }

        // Process contours with Douglas-Peucker algorithm
        // let mut objects: Vec<Vec<Point>> = vec![];
        // for contour in contours.iter() {
        //     objects.push(self.douglas_peucker(&contour));
        //     cdt::triangulate_contours(&[(0.5, 0.5), ], contours)
        // }
    }

    //==========
    // Updating
    //==========
    
    pub(crate) fn process_previous_updates(&self, clock: u8) -> Option<Rect> {
        let mut data = self.chunk_data.lock();
        let mut queue = self.placing_queue.lock();

        if queue.is_empty() && data.dirty_rect.is_empty() {
            return None;
        }

        while !queue.is_empty() {
            let (cell_position, element) = queue.pop_front().unwrap();
            let index = get_cell_index(cell_position.x, cell_position.y);

            if data.cells[index].element == Element::Empty && element != Element::Empty {
                data.set_cell(cell_position, Cell::new(element, clock.wrapping_sub(4)));
                self.cell_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            else if data.cells[index].element != Element::Empty && element == Element::Empty {
                data.set_cell(cell_position, Cell::new(element, clock.wrapping_sub(4)));
                self.cell_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }

            data.update_dirty_rect(&cell_position);
        }

        Some(data.dirty_rect.retrieve())
    }

    pub(crate) fn update(&self, manager: Arc<World>, clock: u8) -> u128 {
        let (x_range, y_range) = {
            match self.process_previous_updates(clock) {
                Some(dirty_rect) => {
                    self.frame_idling.store(0, std::sync::atomic::Ordering::Release);
                    dirty_rect.get_ranges(clock)
                },
                None => {
                    if self.frame_idling.load(std::sync::atomic::Ordering::Acquire) >= IDLE_FRAME_THRESHOLD {
                        return 0;
                    }
                    else {
                        self.frame_idling.fetch_add(1, std::sync::atomic::Ordering::Release);

                        (
                            if clock % 2 == 0 { (0..CHUNK_SIZE).collect() } else { (0..CHUNK_SIZE).rev().collect() },
                            (0..CHUNK_SIZE).collect()
                        )
                    }
                },
            }
        };

        let clock_range: HashSet<u8> = {
            if clock < 3 {
                (0..clock).chain(clock.wrapping_sub(3)..=255).collect()
            }
            else {
                (clock.wrapping_sub(3)..clock).collect()
            }
        };
            

        let mut data = self.chunk_data.lock();
        let mut updated_count: u128 = 0;
        
        let mut api = ChunkApi { 
            cell_position: vec2!(0, 0),
            chunk: self,
            chunk_data: &mut data,
            chunk_manager: manager.clone(),
            clock,
        };

        // self.create_collider();

        for x in x_range.iter() {
            for y in y_range.iter() {
                let cell = api.chunk_data.cells[get_cell_index(*x, *y)];
            
                if cell.clock == clock || cell.element == Element::Empty {
                    continue;
                }

                if clock_range.contains(&cell.clock) {
                    api.chunk_data.dirty_rect.update(&vec2!(*x, *y));
                    continue;
                }

                api.cell_position = vec2!(*x, *y);
                
                api = cell.update(api, 0.0, clock);
                updated_count += 1;
            }
        }

        return updated_count;
    }
}

pub struct ChunkApi<'a, 'b> {
    pub(super) cell_position: Vector2,
    pub(super) chunk: &'a Chunk,
    pub(super) chunk_data: &'b mut ChunkData,
    pub(super) chunk_manager: Arc<World>,
    pub(super) clock: u8,
}

impl<'a, 'b> ChunkApi<'a, 'b> {
    pub fn get(&self, dx: i64, dy: i64) -> Cell {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk_data.get_cell(cell_position)
        }
        else {
            self.chunk_manager.get_cell(self.chunk.position, cell_position)
        }
    }

    pub fn match_element(&self, dx: i64, dy: i64, element: Element) -> bool {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk_data.get_cell(cell_position).element == element
        }
        else {
            self.chunk_manager.get_cell(self.chunk.position, cell_position).element == element
        }
    }

    pub fn set(&mut self, dx: i64, dy: i64, cell: Cell) {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk_data.set_cell(cell_position, cell);
            self.chunk_data.update_dirty_rect(&cell_position);
        }
        else {
            self.chunk_manager.set_cell(self.chunk.position, cell_position, cell);
        }
    }

    pub fn swap(&mut self, dx:i64, dy: i64) {
        let mut cell_position = self.cell_position;
        let mut new_cell_position = cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            if new_cell_position.is_between(0, CHUNK_SIZE - 1) {
                self.chunk_data.swap_cells(cell_position, new_cell_position);
                self.chunk_data.update_dirty_rect(&new_cell_position);
    
                // Update chunks if cell is updated close to their border
                let chunk_offset = vec2!(
                    if cell_position.x == 0 { -1 }
                    else if cell_position.x >= CHUNK_SIZE - 1 { 1 }
                    else { 0 },
    
                    if cell_position.y == 0 { -1 }
                    else if cell_position.y >= CHUNK_SIZE - 1 { 1 }
                    else { 0 }
                );
    
                if !chunk_offset.is_zero() {
                    let (cell_position, _) = (cell_position + chunk_offset).wrap(0, CHUNK_SIZE);
                    self.chunk_manager.refresh_chunk_at_cell(
                        &(self.chunk.position + chunk_offset),
                        &cell_position,
                    );
                }
            }
            else {
                let old_cell = self.chunk_data.get_cell(cell_position);
                let new_cell = self.chunk_manager.replace_cell(self.chunk.position, new_cell_position, old_cell);

                if old_cell.element != Element::Empty && new_cell.element == Element::Empty {
                    self.chunk.cell_count.fetch_sub(1, std::sync::atomic::Ordering::Acquire);
                }
                else if old_cell.element == Element::Empty && new_cell.element != Element::Empty {
                    self.chunk.cell_count.fetch_add(1, std::sync::atomic::Ordering::Acquire);
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
        self.clock % 2 == 0
    }
 }