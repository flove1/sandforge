use rand::Rng;

use super::chunk::ChunkApi;
use super::elements::*;

#[derive(Default, Clone)]
pub struct Cell {
    pub element: MatterType,
    pub ra: u8,
    pub rb: u8,
    pub clock: u8,
    pub flags: u16,
    pub simulation: SimulationType
}

#[derive(Default, Clone)]
pub enum SimulationType {
    #[default]
    Ca,
    Particle {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        collided: bool,
    },
    RigiBody(u64),
}

pub static EMPTY_CELL: Cell = Cell {
    element: MatterType::Empty,
    ra: 0,
    rb: 0,
    clock: 0,
    flags: 0,
    simulation: SimulationType::Ca,
};

impl Cell {
    pub fn new(element: &MatterType, clock: u8) -> Self {
        let mut cell = Self {
            element: element.clone(),
            clock,
            ..Default::default()
        };

        match cell.element {
            MatterType::Empty => {},
            MatterType::Static { color_offset, .. } 
                | MatterType::Powder { color_offset, .. } 
                | MatterType::Liquid { color_offset, .. }
                | MatterType::Gas { color_offset, .. }
                => cell.ra = rand::thread_rng().gen_range(0..=color_offset),
        }

        cell
    }

    pub fn new_with_rb(element: &MatterType, clock: u8, rb: u8) -> Self {
        let mut cell = Self::new(element, clock);
        cell.rb = rb;

        cell
    }

    pub fn update<'a, 'b>(&mut self, api: &mut ChunkApi<'a, 'b>, dt: f32, clock: u8) {
        self.clock = clock;

        match self.simulation {
            SimulationType::Ca => {
                match self.element {
                    MatterType::Powder{ .. } => update_sand(self, api, dt),
                    MatterType::Liquid{ .. } => update_liquid(self, api, dt),
                    MatterType::Gas{ .. } => update_gas(self, api, dt),
                    _ => {}
                };
            },
            SimulationType::Particle { .. } => update_particle(self, api, dt),
            SimulationType::RigiBody (_) => {},
        }
    }
}