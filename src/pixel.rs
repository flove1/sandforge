use compact_str::{format_compact, CompactString};
use lazy_static::lazy_static;
use rand::Rng;

use crate::materials::{Material, FireParameters, PhysicsType};

#[derive(Debug, Clone)]
pub struct Pixel {
    pub material_id: String,
    pub color: [u8; 4],
    pub matter_type: PhysicsType,

    // Created automatically
    pub ra: u8,
    pub rb: u8,
    pub updated_at: u8,
    pub simulation: SimulationType,

    pub temperature: u16,
    pub fire_parameters: Option<FireParameters>,

    pub on_fire: bool,
    pub conductive: bool,
}

#[derive(Debug, Default, Clone)]
pub enum SimulationType {
    #[default]
    Ca,
    RigidBody(usize, usize),
    Displaced(f32, f32),
}

impl Default for Pixel {
    fn default() -> Self {
        Self {
            // Read from material:
            material_id: "air".to_string(),
            color: [0; 4],
            matter_type: PhysicsType::Empty,

            ra: 0,
            rb: 0,
            updated_at: 0,
            simulation: SimulationType::Ca,

            temperature: 30,
            fire_parameters: None,

            on_fire: false,
            conductive: false,
        }
    }
}

lazy_static! {
    pub static ref WALL: Pixel = Pixel {
        material_id: "wall".to_string(),
        color: [0; 4],
        matter_type: PhysicsType::Static,

        ra: 0,
        rb: 0,
        updated_at: 0,
        simulation: SimulationType::Ca,

        temperature: 30,
        fire_parameters: None,

        on_fire: false,
        conductive: false,
    };
}

impl Pixel {
    pub fn new(material: &Material, updated_at: u8) -> Self {
        Self {
            material_id: material.id.clone(),
            color: material.color,
            matter_type: material.matter_type,
            fire_parameters: material.fire_parameters.clone(),

            temperature: 30,
            ra: rand::thread_rng().gen_range(0..=material.color_offset),
            updated_at,
            ..Default::default()
        }
    }

    pub fn get_color(&self) -> [u8; 4] {
        match self.matter_type {
            PhysicsType::Empty => [0; 4],
            PhysicsType::Static | PhysicsType::Powder => [
                self.color[0].saturating_add(self.ra),
                self.color[1].saturating_add(self.ra),
                self.color[2].saturating_add(self.ra),
                self.color[3].saturating_add(self.ra),
            ],
            PhysicsType::Liquid { .. } => [
                self.color[0].saturating_add(fastrand::u8(0..10)),
                self.color[1].saturating_add(fastrand::u8(0..10)),
                self.color[2].saturating_add(fastrand::u8(0..10)),
                self.color[3].saturating_add(fastrand::u8(0..10)),
            ],
            PhysicsType::Gas => [
                self.color[0].saturating_add(fastrand::u8(0..50)),
                self.color[1].saturating_add(fastrand::u8(0..50)),
                self.color[2].saturating_add(fastrand::u8(0..50)),
                self.color[3].saturating_add(fastrand::u8(0..50)),
            ],
        }
    }

    // pub fn new_particle(material: &PhysicsType, x: f32, y: f32, dx: f32, dy: f32) -> Self {
    //     let mut pixel = Self {
    //         material: material.clone(),
    //         clock: 0,
    //         simulation: SimulationType::Particle { x, y, dx, dy, collided: false },
    //         ..Default::default()
    //     };

    //     match pixel.material {
    //         PhysicsType::Empty => {},
    //         PhysicsType::Static { color_offset, .. }
    //             | PhysicsType::Powder { color_offset, .. }
    //             | PhysicsType::Liquid { color_offset, .. }
    //             | PhysicsType::Gas { color_offset, .. }
    //             => pixel.ra = rand::thread_rng().gen_range(0..=color_offset),
    //     }

    //     pixel
    // }

    pub fn new_with_rb(material: &Material, clock: u8, rb: u8) -> Self {
        let mut pixel = Self::new(material, clock);
        pixel.rb = rb;

        pixel
    }

    pub fn is_empty(&self) -> bool {
        self.matter_type == PhysicsType::Empty
    }

    // pub fn update_cell(mut self, api: &mut ChunkApi, dt: f32, clock: u8) {
    //     self.clock = clock;

    //     let directions = [
    //         [1, 0],
    //         [-1, 0],
    //         [0, 1],
    //         [0, -1]
    //     ];

    //     if let Some(reactions) = REACTIONS.get(self.material_id.as_str()) {
    //         for direction in directions.iter() {
    //             let pixel = api.get(direction[0], direction[1]);

    //             if let Some(reaction) = reactions.get(pixel.material_id.as_str()) {
    //                 if fastrand::f32() < reaction.value().probability {
    //                     let element_1 = ELEMENTS.get(reaction.out_element_1.as_str()).unwrap();
    //                     let element_2 = ELEMENTS.get(reaction.out_element_2.as_str()).unwrap();

    //                     api.set(direction[0], direction[1], Pixel::new(&element_2, clock));
    //                     api.update(Pixel::new(&element_1, clock));

    //                     api.keep_alive(0, 0);

    //                     return;
    //                 }
    //             }
    //         }
    //     }

    //     match self.on_fire {
    //         true => {
    //             if let Some(fire_parameters) = self.fire_parameters.as_mut() {
    //                 for direction in directions.iter() {
    //                     let mut pixel = api.get(direction[0], direction[1]);

    //                     if pixel.temperature < fire_parameters.fire_temperature {
    //                         pixel.temperature += (fastrand::f32() * (pixel.temperature.abs_diff(fire_parameters.fire_temperature) as f32 / 8.0)) as u16;
    //                     }

    //                     api.set(direction[0], direction[1], pixel);
    //                 }

    //                 if fire_parameters.fire_hp <= 0 {
    //                     api.update(Pixel::default());
    //                     return;
    //                 }
    //                 else if fastrand::f32() > 0.75 {
    //                     fire_parameters.fire_hp -= 1;
    //                 }
    //             }
    //         },
    //         false => {
    //             if let Some(fire_parameters) = &mut self.fire_parameters {
    //                 if self.temperature >= fire_parameters.ignition_temperature {
    //                     self.on_fire = true;
    //                 }
    //                 else {
    //                     self.temperature -= 30u16.abs_diff(self.temperature) / 16
    //                 }
    //             }
    //         },
    //     }

    //     api.update(self.clone());

    //     match self.simulation {
    //         SimulationType::Ca => {
    //             match self.matter_type {
    //                 PhysicsType::Powder{ .. } => update_sand(self, api, dt),
    //                 PhysicsType::Liquid{ .. } => update_liquid(self, api, dt),
    //                 PhysicsType::Gas{ .. } => update_gas(self, api, dt),
    //                 _ => {}
    //             };
    //         },
    //         // SimulationType::Particle { .. } => update_particle(self, api, dt),
    //         SimulationType::RigidBody ( .. ) => {},
    //         SimulationType::Displaced( .. ) => update_displaced(self, api, dt),
    //     }
    // }
}

