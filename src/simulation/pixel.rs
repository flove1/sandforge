use lazy_static::lazy_static;

use super::materials::{MaterialInstance, PhysicsType};

#[derive(Debug, Clone)]
pub struct Pixel {
    pub material: MaterialInstance,

    pub ra: u8,
    pub rb: u8,
    pub updated_at: u8,

    pub temperature: i32,

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
            temperature: 30,
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
        temperature: 30,

        on_fire: false,
        conductive: false,
    };
}

impl Pixel {
    pub fn new(material: MaterialInstance) -> Self {
        Self {
            material: material.clone(),
            ..Default::default()
        }
    }

    pub fn with_clock(self, updated_at: u8) -> Self {
        Self {
            updated_at,
            ..self
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
                .map(|channel| channel.saturating_add(fastrand::u8(0..10))),
            PhysicsType::Gas => self
                .material
                .color
                .map(|channel| channel.saturating_add(fastrand::u8(0..50))),
            PhysicsType::Actor => [0; 4],
            PhysicsType::Disturbed( .. ) => self.material.color,
        }
    }

    pub fn with_material(mut self, material: MaterialInstance) -> Self {
        Self {
            material,
            ..self
        }
    }

    pub fn is_empty(&self) -> bool {
        self.material.physics_type == PhysicsType::Air
    }
}