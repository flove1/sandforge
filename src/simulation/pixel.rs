use lazy_static::lazy_static;

use super::materials::{MaterialInstance, PhysicsType};

#[derive(Debug, Clone)]
pub struct Pixel {
    pub material: MaterialInstance,

    pub ra: u8,
    pub rb: u8,
    pub updated_at: u8,

    pub on_fire: bool,
    pub conductive: bool,
}

impl Default for Pixel {
    fn default() -> Self {
        Self {
            material: MaterialInstance {
                id: "air".to_string(),
                physics_type: PhysicsType::Air,
                color: [0; 4],
                fire_parameters: None,
            },

            ra: 0,
            rb: 0,
            updated_at: 0,
            on_fire: false,
            conductive: false,
        }
    }
}

lazy_static! {
    pub static ref WALL: Pixel = Pixel {
        material: MaterialInstance {
            id: "wall".to_string(),
            color: [0; 4],
            physics_type: PhysicsType::Static,
            fire_parameters: None,
        },

        ra: 0,
        rb: 0,
        updated_at: 0,
        // simulation: SimulationType::Ca,

        on_fire: false,
        conductive: false,
    };
}

impl Pixel {
    pub fn new(material: MaterialInstance, updated_at: u8) -> Self {
        Self {
            material: material.clone(),
            updated_at,
            ..Default::default()
        }
    }

    pub fn get_color(&self) -> [u8; 4] {
        match self.material.physics_type {
            PhysicsType::Air
            | PhysicsType::Static
            | PhysicsType::Powder
            | PhysicsType::Rigidbody => self.material.color,
            PhysicsType::Liquid { .. } => self
                .material
                .color
                .map(|channel| channel + fastrand::u8(0..10)),
            PhysicsType::Gas => self
                .material
                .color
                .map(|channel| channel + fastrand::u8(0..50)),
            PhysicsType::Actor => [0; 4],
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

    pub fn is_empty(&self) -> bool {
        self.material.physics_type == PhysicsType::Air
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
