use crate::chunk::*;
use super::elements::*;

#[derive(Default, Clone, Copy)]
pub struct Cell {
    pub vel_x: f32,
    pub vel_y: f32,
    pub element: Element,
    pub iter_bit: bool,
    pub falling: bool,
    pub active: bool,
}

pub static WALL_CELL: Cell = Cell {
    vel_x: 0.0,
    vel_y: 0.0,
    iter_bit: false,
    falling: false,
    active: true,
    element: Element::Stone,
};

impl Cell {
    pub fn new(element: Element) -> Self {
        Self {
            vel_y: 2.0,
            element,
            active: true,
            ..Default::default()
        }
    }

    pub fn update(&mut self, api: ChunkApi, dt: f32) {
        if self.iter_bit == api.get_iter_bit() || !self.active {
            return;
        }
        self.iter_bit = !self.iter_bit;
        match self.element {
            Element::Empty => {},
            Element::Stone => {},
            Element::Water => update_liquid(*self, api, dt),
            Element::Sand => update_sand(*self, api, dt),
        }
    }
}