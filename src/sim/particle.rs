use crate::{helpers::line_from_pixels, constants::CHUNK_SIZE};

use super::{cell::{Cell, SimulationType}, chunk::ChunkApi, elements::MatterType};

pub struct Particle {
    pub cell: Cell,
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub collided: bool,
}

pub enum ParticleType {
    DynamicCell(Cell),
    Light
}

pub enum ParticleState {
    Free,
}

impl Particle {
    pub fn new(
        cell: Cell,
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        collided: bool
    ) -> Self {
        Self { 
            cell,
            x,
            y,
            dx,
            dy,
            collided,
        }
    }

    pub fn update(&mut self, api: &mut ChunkApi) {
        if self.collided {
            return;
        }

        let mut last_x = 0;
        let mut last_y = 0;

        let mut operation = |current_dx, current_dy| {
            let current_cell = api.get(current_dx, current_dy);

            if !matches!(current_cell.element.matter, MatterType::Static { .. }) || (matches!(current_cell.simulation, SimulationType::RigidBody(..))) {
                last_x = current_dx;
                last_y = current_dy;
                true
            }
            else {
                false
            }
            
        };

        let return_to_ca = line_from_pixels(
            0, 
            0, 
            (self.dx * CHUNK_SIZE as f32).round() as i32, 
            (self.dy * CHUNK_SIZE as f32).round() as i32, 
            &mut operation
        );

        if return_to_ca {
            self.collided = true;
        }
        else {
            self.x += self.dx;
            self.y += self.dy;
            self.dy = f32::min(self.dy - (1.0 / CHUNK_SIZE as f32) / 10.0, self.dy.signum() * 9.1 * (1.0 / CHUNK_SIZE as f32) / 10.0);
        }
    }
}