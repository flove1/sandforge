use lazy_static::lazy_static;

use super::materials::{ FireParameters, Material, PhysicsType };

#[derive(Debug, Clone)]
pub struct Pixel {
    pub id: String,
    pub physics_type: PhysicsType,
    pub color: [u8; 4],

    pub fire_parameters: Option<FireParameters>,

    pub ra: u8,
    pub rb: u8,
    pub updated_at: u8,

    pub on_fire: bool,
}

impl Default for Pixel {
    fn default() -> Self {
        Self {
            ra: 0,
            rb: 0,
            updated_at: 0,
            on_fire: false,
            id: "air".to_string(),
            physics_type: PhysicsType::Air,
            color: [0; 4],
            fire_parameters: None,
        }
    }
}

lazy_static! {
    pub static ref WALL: Pixel = Pixel {
        id: "wall".to_string(),
        color: [0; 4],
        physics_type: PhysicsType::Static,
        fire_parameters: None,

        ra: 0,
        rb: 0,
        updated_at: 0,

        on_fire: false,
    };
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
            id: val.id,
            color: color_offseted,
            physics_type: val.physics_type,
            fire_parameters: val.fire_parameters,
            
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
            id: val.id.clone(),
            color: color_offseted,
            physics_type: val.physics_type.clone(),
            fire_parameters: val.fire_parameters.clone(),
            
            ..Default::default()
        }
    }
}

impl Pixel {
    pub fn get_color(&self) -> [u8; 4] {
        match self.physics_type {
            PhysicsType::Air | PhysicsType::Static | PhysicsType::Powder | PhysicsType::Rigidbody =>
                self.color,
            PhysicsType::Liquid { .. } =>
                self.color.map(|channel| channel.saturating_add(fastrand::u8(0..10))),
            PhysicsType::Gas =>
                self.color.map(|channel| channel.saturating_add(fastrand::u8(0..50))),
            PhysicsType::Disturbed(..) => self.color,
        }
    }

    pub fn with_clock(self, updated_at: u8) -> Self {
        Self {
            updated_at,
            ..self
        }
    }

    // pub fn with_material(mut self, material: MaterialInstance) -> Self {
    //     self.material = material;
    //     self
    // }

    pub fn with_physics(mut self, physics: PhysicsType) -> Self {
        self.physics_type = physics;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.physics_type == PhysicsType::Air
    }
}
