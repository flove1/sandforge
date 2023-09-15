use rand::Rng;

use super::chunk::ChunkApi;
use super::elements::*;

#[derive(Default, Clone, Copy)]
pub struct Cell {
    pub element: Element,
    pub ra: u8,
    pub rb: u8,
    pub clock: u8,
    pub parent_id: Option<u64>,
    pub flags: u16,
}

pub static WALL_CELL: Cell = Cell {
    element: Element::Stone,
    ra: 0,
    rb: 0,
    clock: 0,
    flags: 0,
    parent_id: None,
};

pub static EMPTY_CELL: Cell = Cell {
    element: Element::Empty,
    ra: 0,
    rb: 0,
    clock: 0,
    flags: 0,
    parent_id: None,
};

impl Cell {
    pub fn new(element: Element, clock: u8) -> Self {
        let mut cell = Self {
            element,
            clock,
            ..Default::default()
        };

        match cell.element {
            Element::Coal | Element::Water | Element::Gas | Element::Sand | Element::Wood => {cell.ra = rand::thread_rng().gen_range(0..25)},
            Element::Dirt => {cell.ra = rand::thread_rng().gen_range(0..100)},
            _ => {},
        }

        // match  cell.element {
        //     Element::Fire => {cell.rb = 3}
        //     _ => {}
        // }

        cell
    }

    pub fn new_with_rb(element: Element, clock: u8, rb: u8) -> Self {
        let mut cell = Self::new(element, clock);
        cell.rb = rb;

        cell
    }

    pub fn update<'a, 'b>(mut self, mut api: ChunkApi<'a, 'b>, dt: f32, clock: u8) -> ChunkApi<'a, 'b>  {
        self.clock = clock;

        api = match self.element {
            Element::Water => { update_liquid(&mut self, api, dt) },
            Element::Coal | Element::Sand => { update_sand(&mut self, api, dt) },
            // Element::Fire => { update_fire(&mut self, api, dt) },
            Element::Gas => { update_gas(&mut self, api, dt) },
            _ => { api }
        };

        return api;
    }
}