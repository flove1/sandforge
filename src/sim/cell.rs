use rand::Rng;

use super::chunk::ChunkApi;
use super::elements::*;

#[derive(Default, Clone)]
pub struct Cell {
    pub element: Element,
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
    RigidBody(usize, usize),
}

impl Cell {
    pub fn new(element: &Element, clock: u8) -> Self {
        Self {
            ra: rand::thread_rng().gen_range(0..=element.color_offset),
            element: element.clone(),
            clock,
            ..Default::default()
        }
    }

    // pub fn new_particle(element: &MatterType, x: f32, y: f32, dx: f32, dy: f32) -> Self {
    //     let mut cell = Self {
    //         element: element.clone(),
    //         clock: 0,
    //         simulation: SimulationType::Particle { x, y, dx, dy, collided: false },
    //         ..Default::default()
    //     };

    //     match cell.element {
    //         MatterType::Empty => {},
    //         MatterType::Static { color_offset, .. } 
    //             | MatterType::Powder { color_offset, .. } 
    //             | MatterType::Liquid { color_offset, .. }
    //             | MatterType::Gas { color_offset, .. }
    //             => cell.ra = rand::thread_rng().gen_range(0..=color_offset),
    //     }

    //     cell
    // }

    pub fn new_with_rb(element: &Element, clock: u8, rb: u8) -> Self {
        let mut cell = Self::new(element, clock);
        cell.rb = rb;

        cell
    }

    pub fn update_cell<'a>(&mut self, api: &mut ChunkApi<'a>, dt: f32, clock: u8) {
        self.clock = clock;

        match self.simulation {
            SimulationType::Ca => {
                match self.element.matter {
                    MatterType::Powder{ .. } => update_sand(self, api, dt),
                    MatterType::Liquid{ .. } => update_liquid(self, api, dt),
                    MatterType::Gas{ .. } => update_gas(self, api, dt),
                    _ => {}
                };
            },
            // SimulationType::Particle { .. } => update_particle(self, api, dt),
            SimulationType::RigidBody ( .. ) => {},
        }
    }
}