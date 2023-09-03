use rand::Rng;

use super::chunk::ChunkApi;
use super::elements::*;

#[derive(Default, Clone, Copy)]
pub struct Cell {
    pub element: Element,
    pub ra: u8,
    pub rb: u8,
    pub clock: u8,
}

pub static WALL_CELL: Cell = Cell {
    element: Element::Stone,
    ra: 0,
    rb: 0,
    clock: 0,
};

pub static EMPTY_CELL: Cell = Cell {
    element: Element::Empty,
    ra: 0,
    rb: 0,
    clock: 0,
};

impl Cell {
    pub fn new(element: Element, clock: u8) -> Self {
        let mut cell = Self {
            element,
            clock,
            ..Default::default()
        };

        match cell.element {
            Element::Water | Element::Sand | Element::Wood => {cell.ra = rand::thread_rng().gen_range(0..25)},
            _ => {},
        }

        cell
    }

    pub fn update<'a, 'b>(mut self, mut api: ChunkApi<'a, 'b>, dt: f32, clock: u8) -> ChunkApi<'a, 'b>  {
        self.clock = clock;

        api = match self.element {
            Element::Empty => { api },
            Element::Stone => { api },
            Element::Water => { update_liquid(&mut self, api, dt) },
            Element::Sand => { update_sand(&mut self, api, dt) },
            Element::GlowingSand => { update_sand(&mut self, api, dt) },
            Element::Wood => { api },
        };

        api.update(self);
        return api;
    }
}