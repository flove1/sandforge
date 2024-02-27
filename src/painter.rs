use bevy::ecs::{system::Resource, world::{FromWorld, World}};

use crate::materials::Material;

#[derive(Resource)]
pub struct BrushRes{
    pub material: Option<Material>,
    pub brush_type: BrushType,
    pub shape: BrushShape,
    pub size: i32, 
}

#[derive(Clone, PartialEq)]
pub enum BrushType {
    Cell,
    Object,
    StaticObject,
    Particle(u8),
    Force(f32),
    ObjectEraser,
}

#[derive(Clone, PartialEq)]
pub enum BrushShape {
    Circle,
    Square,
}

impl BrushShape {
    pub fn draw<F: FnMut(i32, i32)> (
        &self,
        x: i32, 
        y: i32, 
        size: i32,
        operation: &mut F
    ) {
        match self {
            BrushShape::Circle => {
                for dx in -size..=size {
                    for dy in -size..=size {
                        if (dx).pow(2) + (dy).pow(2) > size.pow(2) {
                            continue;
                        }

                        operation(x + dx, y + dy);
                    }
                }
            },
            BrushShape::Square => {
                for dx in -size..=size {
                    for dy in -size..=size {
                        operation(x + dx, y + dy);
                    }
                }
            },
        }
    }    
}

impl FromWorld for BrushRes {
    fn from_world(_world: &mut World) -> Self {
        Self {
            material: None, 
            brush_type: BrushType::Cell, 
            shape: BrushShape::Circle, 
            size: 10,
        }
    }
}