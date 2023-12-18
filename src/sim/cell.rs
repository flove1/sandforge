use compact_str::{CompactString, format_compact};
use rand::Rng;
use lazy_static::lazy_static;

use super::chunk::ChunkApi;
use super::elements::*;

#[derive(Clone)]
pub struct Cell {
    // Read from element:
    pub element_id: CompactString,
    pub color: [u8; 4],
    pub matter_type: MatterType,

    // Created automatically
    pub ra: u8,
    pub rb: u8,
    pub clock: u8,
    pub simulation: SimulationType,

    pub temperature: u16,
    pub fire_parameters: Option<FireParameters>,
    
    pub on_fire: bool,
    pub conductive: bool,
}

#[derive(Default, Clone)]
pub enum SimulationType {
    #[default]
    Ca,
    RigidBody(usize, usize),
    Particle(f32, f32),
}

impl Default for Cell {
    fn default() -> Self {
        Self {    // Read from element:
            element_id: format_compact!("air"),
            color: [0; 4],
            matter_type: MatterType::Empty,

            ra: 0,
            rb: 0,
            clock: 0,
            simulation: SimulationType::Ca,

            temperature: 30,
            fire_parameters: None,
            
            on_fire: false,
            conductive: false,
        }
    }
}

lazy_static! {
    pub static ref WALL: Cell = Cell {
        element_id: format_compact!("wall"),
        color: [0, 0, 0, 255],
        matter_type: MatterType::Static,

        ra: 0,
        rb: 0,
        clock: 0,
        simulation: SimulationType::Ca,

        temperature: 30,
        fire_parameters: None,

        on_fire: false,
        conductive: false,
    };
}

impl Cell {
    pub fn new(element: &Element, clock: u8) -> Self {
        Self {
            element_id: element.id.clone(),
            color: element.color,
            matter_type: element.matter_type,
            fire_parameters: element.fire_parameters.clone(),
            
            temperature: 30,
            ra: rand::thread_rng().gen_range(0..=element.color_offset),
            clock,
            ..Default::default()
        }
    }

    pub fn get_color(&self) -> [u8; 4] {
        match self.matter_type {
            MatterType::Empty => [0; 4],
            MatterType::Static | MatterType::Powder => [
                self.color[0].saturating_add(self.ra),
                self.color[1].saturating_add(self.ra),
                self.color[2].saturating_add(self.ra),
                self.color[3].saturating_add(self.ra),
            ],
            MatterType::Liquid{..} => [
                self.color[0].saturating_add(fastrand::u8(0..10)),
                self.color[1].saturating_add(fastrand::u8(0..10)),
                self.color[2].saturating_add(fastrand::u8(0..10)),
                self.color[3].saturating_add(fastrand::u8(0..10)),
            ],
            MatterType::Gas => [
                self.color[0].saturating_add(fastrand::u8(0..50)),
                self.color[1].saturating_add(fastrand::u8(0..50)),
                self.color[2].saturating_add(fastrand::u8(0..50)),
                self.color[3].saturating_add(fastrand::u8(0..50)),
            ],
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

    pub fn update_cell(mut self, api: &mut ChunkApi, dt: f32, clock: u8) {
        self.clock = clock;

        let directions = [
            [1, 0],
            [-1, 0],
            [0, 1],
            [0, -1]
        ];

        if let Some(reactions) = REACTIONS.get(self.element_id.as_str()) {
            for direction in directions.iter() {
                let cell = api.get(direction[0], direction[1]);

                if let Some(reaction) = reactions.get(cell.element_id.as_str()) {
                    if fastrand::f32() < reaction.value().probability {
                        let element_1 = ELEMENTS.get(reaction.out_element_1.as_str()).unwrap();
                        let element_2 = ELEMENTS.get(reaction.out_element_2.as_str()).unwrap();

                        api.set(direction[0], direction[1], Cell::new(&element_2, clock));
                        api.update(Cell::new(&element_1, clock));
                        
                        api.keep_alive(0, 0);

                        return;
                    }
                }
            }
        }
        
        match self.on_fire {
            true => {
                if let Some(fire_parameters) = self.fire_parameters.as_mut() {
                    for direction in directions.iter() {
                        let mut cell = api.get(direction[0], direction[1]);

                        if cell.temperature < fire_parameters.fire_temperature {
                            cell.temperature += (fastrand::f32() * (cell.temperature.abs_diff(fire_parameters.fire_temperature) as f32 / 8.0)) as u16;
                        }

                        api.set(direction[0], direction[1], cell);
                    }
    
                    if fire_parameters.fire_hp <= 0 {
                        api.update(Cell::default());
                        return;
                    }
                    else if fastrand::f32() > 0.75 {
                        fire_parameters.fire_hp -= 1;
                    }
                }
            },
            false => {
                if let Some(fire_parameters) = &mut self.fire_parameters {
                    if self.temperature >= fire_parameters.ignition_temperature {
                        self.on_fire = true;
                    }
                    else {
                        self.temperature -= 30u16.abs_diff(self.temperature) / 16
                    }
                }
            },
        }

        api.update(self.clone());

        match self.simulation {
            SimulationType::Ca => {
                match self.matter_type {
                    MatterType::Powder{ .. } => update_sand(self, api, dt),
                    MatterType::Liquid{ .. } => update_liquid(self, api, dt),
                    MatterType::Gas{ .. } => update_gas(self, api, dt),
                    _ => {}
                };
            },
            // SimulationType::Particle { .. } => update_particle(self, api, dt),
            SimulationType::RigidBody ( .. ) => {},
            SimulationType::Particle( .. ) => update_particle(self, api, dt),
        }
    }
}