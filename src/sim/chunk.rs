use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicU8};
use std::sync::Arc;

use crossbeam::atomic::AtomicCell;
use egui_winit::egui::epaint::ahash::HashSet;
use parking_lot::Mutex;
use rand::Rng;

use super::cell::*;
use super::elements::Element;
use super::helpers::get_cell_index;
use super::objects::marching_squares;
use super::world::World;

use crate::renderer::Vertex;
use crate::{constants::*, vec2};
use crate::vector::Vector2;

#[derive(Default)]
pub struct Chunk {
    pub(super) cells: Vec<AtomicCell<Cell>>,
    pub(super) dirty_rect: Mutex<Rect>,
    placing_queue: Mutex<VecDeque<(Vector2, Element)>>,
    pub(super) cell_count: AtomicU64,
    pub(super) position: Vector2,
    pub(super) frame_idling: AtomicU8,
    pub(super) objects: Mutex<Vec<Vec<(f64, f64)>>>
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

impl Chunk {
    pub(crate) fn new(position: Vector2) -> Self {
        let mut chunk = Self {
            cells: Vec::with_capacity(CHUNK_SIZE.pow(2) as usize),
            placing_queue: Mutex::new(VecDeque::new()),
            dirty_rect: Mutex::new(Rect::default()),
            position,
            ..Default::default()
        };

        for _ in 0..(CHUNK_SIZE.pow(2)) {
            chunk.cells.push(AtomicCell::new(Cell::default()));
        }

        chunk
    }

    //================
    // Global methods
    //================

    pub(crate) fn place(&self, x: i64, y: i64, element: Element) {
        let mut queue = self.placing_queue.lock();
        queue.push_back((vec2!(x, y), element));
    }

    pub(crate) fn update_dirty_rect(&self, position: &Vector2) {
        let mut rect_lock = self.dirty_rect.lock();
        rect_lock.update_at_corners(position);
    }

    pub(crate) fn maximize_dirty_rect(&self) {
        let mut rect_lock = self.dirty_rect.lock();

        rect_lock.update(&vec2!(0, 0));
        rect_lock.update(&vec2!(CHUNK_SIZE-1, CHUNK_SIZE-1));
    }

    //==================
    // Work through api
    //==================

    pub(crate) fn get_cell(&self, cell_position: Vector2) -> Cell {
        self.cells[cell_position.to_index(CHUNK_SIZE)].load()
    }

    pub(crate) fn set_cell(&self, cell_position: Vector2, cell: Cell) {
        self.cells[cell_position.to_index(CHUNK_SIZE)].store(cell);
    }

    pub(crate) fn swap_cells(&self, cell_position_1: Vector2, cell_position_2: Vector2) {
        let index_1 = cell_position_1.to_index(CHUNK_SIZE);
        let index_2 = cell_position_2.to_index(CHUNK_SIZE);
        self.cells[index_1].store(self.cells[index_2].swap(self.cells[index_1].load()));
    }
    
    //===========
    // Colliders
    //===========

    fn label_cell(&self, x: i64, y: i64, label: i32, labeled_cells: &mut Vec<i32>) {
        if x < 0 || x > CHUNK_SIZE - 1 || y < 0 || y > CHUNK_SIZE - 1 {
            return;
        }
    
        let index = get_cell_index(x, y);
        if labeled_cells[index] != 0 || self.cells[index].load().element != Element::Wood {
            return;
        }
    
        labeled_cells[index] = label;
    
        self.label_cell(x + 1, y, label, labeled_cells);
        self.label_cell(x - 1, y, label, labeled_cells);
        self.label_cell(x, y + 1, label, labeled_cells);
        self.label_cell(x, y - 1, label, labeled_cells);
    }

    pub fn create_collider(&self) {
        let mut labeled_cells = vec![0; CHUNK_SIZE.pow(2) as usize];
        let mut label = 0;
        
        // Connected-component labeling
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let index = get_cell_index(x, y);
                if labeled_cells[index] == 0 && self.cells[get_cell_index(x, y)].load().element == Element::Wood {
                    label += 1;
                    self.label_cell(x, y, label, &mut labeled_cells);
                }
            }
        }

        let mut lock = self.objects.lock();
        lock.clear();
        lock.append(&mut marching_squares(label, &mut labeled_cells, CHUNK_SIZE));
        
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
        let mut queue = self.placing_queue.lock();
        let mut dirty_rect = self.dirty_rect.lock();

        if queue.is_empty() && dirty_rect.is_empty() {
            return None;
        }

        while !queue.is_empty() {
            let (cell_position, element) = queue.pop_front().unwrap();
            let index = get_cell_index(cell_position.x, cell_position.y);

            if self.cells[index].load().element == Element::Empty && element != Element::Empty {
                self.set_cell(cell_position, Cell::new(element, clock.wrapping_sub(4)));
                self.cell_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            else if self.cells[index].load().element != Element::Empty && element == Element::Empty {
                self.set_cell(cell_position, Cell::new(element, clock.wrapping_sub(4)));
                self.cell_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }

            dirty_rect.update(&cell_position);
        }

        Some(dirty_rect.retrieve())
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
            
        let mut api = ChunkApi { 
            cell_position: vec2!(0, 0),
            chunk: self,
            chunk_manager: manager.clone(),
            clock,
            dirty_rect: Rect::new(),
        };

        let mut updated_count: u128 = 0;

        self.create_collider();

        for x in x_range.iter() {
            for y in y_range.iter() {
                let cell = self.cells[get_cell_index(*x, *y)].load();
            
                if cell.clock == clock || cell.element == Element::Empty {
                    continue;
                }

                if clock_range.contains(&cell.clock) {
                    api.dirty_rect.update(&vec2!(*x, *y));
                    continue;
                }

                api.cell_position = vec2!(*x, *y);
                
                api = cell.update(api, 0.0);
                updated_count += 1;
            }
        }

        self.dirty_rect.lock().combine(api.dirty_rect);
        return updated_count;
    }
}

pub struct ChunkApi<'a> {
    pub(super) dirty_rect: Rect,
    pub(super) cell_position: Vector2,
    pub(super) chunk: &'a Chunk,
    pub(super) chunk_manager: Arc<World>,
    pub(super) clock: u8,
}

impl<'a> ChunkApi<'a> {
    pub fn get(&self, dx: i64, dy: i64) -> Cell {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.get_cell(cell_position)
        }
        else {
            self.chunk_manager.get_cell(self.chunk.position, cell_position)
        }
    }

    pub fn match_element(&self, dx: i64, dy: i64, element: Element) -> bool {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.get_cell(cell_position).element == element
        }
        else {
            self.chunk_manager.get_cell(self.chunk.position, cell_position).element == element
        }
    }

    pub fn set(&mut self, dx: i64, dy: i64, cell: Cell) {
        let mut cell_position = self.cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.set_cell(cell_position, cell);
            self.dirty_rect.update_at_corners(&cell_position);
        }
        else {
            self.chunk_manager.set_cell(self.chunk.position, cell_position, cell);
        }
    }

    pub fn swap(&mut self, dx:i64, dy: i64) {
        let mut cell_position = self.cell_position;
        let mut new_cell_position = cell_position.add(dx, dy);

        if cell_position.is_between(0, CHUNK_SIZE - 1) && new_cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.swap_cells(cell_position, new_cell_position);

            self.dirty_rect.update_at_corners(&cell_position);
            self.dirty_rect.update_at_corners(&new_cell_position);

            let chunk_offset = vec2!(
                if cell_position.x == 0 { -1 }
                else if cell_position.x >= CHUNK_SIZE - 1 { 1 }
                else { 0 },

                if cell_position.y == 0 { -1 }
                else if cell_position.y >= CHUNK_SIZE - 1 { 1 }
                else { 0 }
            );

            if chunk_offset != vec2!(0, 0) {
                (cell_position, _) = (cell_position + chunk_offset).wrap(0, CHUNK_SIZE);
                self.chunk_manager.refresh_chunk_at_cell(
                    &(self.chunk.position + chunk_offset),
                    &cell_position,
                );
            }
        }
        else {
            self.chunk_manager.swap_cells(self.chunk.position, cell_position, new_cell_position);
        }

        self.cell_position.change(dx, dy);
    }

    pub fn update(&mut self, mut cell: Cell) {
        cell.clock = self.clock;

        if self.cell_position.is_between(0, CHUNK_SIZE - 1) {
            self.chunk.set_cell(self.cell_position, cell);
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