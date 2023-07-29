use rand::Rng;

use crate::chunk::*;
use super::elements::*;

#[derive(Default, Clone, Copy)]
pub struct Cell {
    pub element: Element,
    pub ra: u8,
    pub rb: u8,
    pub iter_bit: bool,
}

pub static WALL_CELL: Cell = Cell {
    element: Element::Stone,
    ra: 0,
    rb: 0,
    iter_bit: false,
};

pub enum CellAction {
    Swap(i64, i64),
    Set(i64, i64, Cell),
    Update(Cell),
}

impl Cell {
    pub fn new(element: Element) -> Self {
        let mut cell = Self {
            element,
            ..Default::default()
        };

        match cell.element {
            Element::Sand => {cell.ra = rand::thread_rng().gen_range(0..25)},
            _ => {},
        }

        cell
    }

    pub fn update(&self, api: PixelToChunkApi, dt: f32) -> Vec<CellAction> {
        match self.element {
            Element::Empty => { vec![] },
            Element::Stone => { vec![] },
            Element::Water => { update_liquid(*self, api, dt) },
            Element::Sand => { update_sand(*self, api, dt) },
            Element::GlowingSand => { update_sand(*self, api, dt) },
        }
    }
}