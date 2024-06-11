use lazy_static::lazy_static;

use super::materials::{ Fire, Material, PhysicsType };

#[derive(Clone)]
pub struct Pixel {
    pub material: Material,
    pub physics_type: PhysicsType,

    pub durability: Option<f32>,
    pub color: [u8; 4],
    pub fire_parameters: Option<Fire>,

    pub updated_at: u8,
    pub on_fire: bool,
}

impl Default for Pixel {
    fn default() -> Self {
        Self {
            physics_type: PhysicsType::Air,
            material: Material::default(),
            fire_parameters: None,
            on_fire: false,
            updated_at: 0,
            color: [0; 4],
            durability: None,
        }
    }
}

lazy_static! {
    pub static ref WALL: Pixel = Pixel::from(Material {
        id: "barrier".to_string(),
        physics_type: PhysicsType::Air,
        ..Default::default()
    });
}

impl From<Material> for Pixel {
    fn from(val: Material) -> Self {
        let channel_offset = fastrand::u8(0..=val.color_offset);

        let color_offseted = [
            val.color[0].saturating_add(channel_offset),
            val.color[1].saturating_add(channel_offset),
            val.color[2].saturating_add(channel_offset),
            val.color[3],
        ];

        Self {
            color: color_offseted,
            durability: val.durability.clone(),
            physics_type: val.physics_type.clone(),
            fire_parameters: val.fire.clone(),
            material: val,

            ..Default::default()
        }
    }
}

impl From<&Material> for Pixel {
    fn from(val: &Material) -> Self {
        let channel_offset = fastrand::u8(0..=val.color_offset);

        let color_offseted = [
            val.color[0].saturating_add(channel_offset),
            val.color[1].saturating_add(channel_offset),
            val.color[2].saturating_add(channel_offset),
            val.color[3],
        ];

        Self {
            color: color_offseted,
            physics_type: val.physics_type.clone(),
            fire_parameters: val.fire.clone(),
            material: val.clone(),

            ..Default::default()
        }
    }
}

impl Pixel {
    pub fn get_color(&self) -> [u8; 4] {
        match self.physics_type {
            PhysicsType::Liquid { .. } =>
                self.color.map(|channel| channel.saturating_add(fastrand::u8(0..10))),
            PhysicsType::Gas(..) =>
                self.color.map(|channel| channel.saturating_add(fastrand::u8(0..50))),
            _ => self.color,
        }
    }

    pub fn with_clock(self, updated_at: u8) -> Self {
        Self {
            updated_at,
            ..self
        }
    }

    pub fn reset_physics(mut self) -> Self {
        self.physics_type = self.material.physics_type.clone();
        self
    }

    pub fn with_physics(mut self, physics: PhysicsType) -> Self {
        self.physics_type = physics;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.physics_type == PhysicsType::Air
    }
}
