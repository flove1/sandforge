use rapier2d::{prelude::*, na::{Isometry2, Vector2}};

use crate::{constants::{CHUNK_SIZE, WORLD_WIDTH, WORLD_HEIGHT, PHYSICS_TO_WORLD, PHYSICS_SCALE}, pos2, vector::Pos2};

use super::{cell::{Cell, SimulationType}, colliders::create_triangulated_collider};

pub struct PhysicsObject {
    pub rb_handle: RigidBodyHandle,
    pub collider_handle: ColliderHandle,
    pub cells: Vec<ObjectPoint>,
    pub object_size: usize,
}

pub struct ObjectPoint {
    pub texture_coords: Vector2<f32>,
    pub cell: Cell,
}

pub struct Physics {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    physics_hooks: Box<dyn PhysicsHooks>,
    event_handler: Box<dyn EventHandler>,

    pub objects: Vec<PhysicsObject>,
    pub active_object_ids: Vec<usize>,
}

impl Physics {
    pub fn new() -> Self {
        Self { 
            rigid_body_set: RigidBodySet::default(),
            collider_set: ColliderSet::default(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::default(),
            island_manager: IslandManager::default(),
            broad_phase: BroadPhase::default(),
            narrow_phase: NarrowPhase::default(),
            impulse_joint_set: ImpulseJointSet::default(),
            multibody_joint_set: MultibodyJointSet::default(),
            ccd_solver: CCDSolver::default(),
            physics_hooks: Box::new(()),
            event_handler: Box::new(()),

            objects: vec![],
            active_object_ids: vec![],
        }
    }

    // pub fn modify_object(&mut self, object_id: usize, cell_index: usize, cell: Cell) {
    //     self.objects[object_id].matrix[cell_index] = cell;
    // }
    
    pub fn new_object(&mut self, cells: Vec<((i32, i32), Cell)>, static_flag: bool) {
        let mut x_positions = cells.iter().map(|(position, _)| position.0).collect::<Vec<i32>>();
        x_positions.sort();

        let mut y_positions = cells.iter().map(|(position, _)| position.1).collect::<Vec<i32>>();
        y_positions.sort();

        let (x_min, x_max) = (
            *x_positions.first().unwrap(),
            *x_positions.last().unwrap(),
        );

        let (y_min, y_max) = (
            *y_positions.first().unwrap(),
            *y_positions.last().unwrap(),
        );

        let (size, x_offset, y_offset) = {
            let dx = x_max - x_min + 1;
            let dy = y_max - y_min + 1;
            
            if dx > dy {
                (dx as usize, 0, (dx - dy) / 2)
            }
            else {
                (dy as usize, (dy - dx) / 2, 0)
            }
        };

        let mut matrix = vec![0; size.pow(2)];
        // let mut cell_matrix = vec![EMPTY_CELL.clone(); size.pow(2)];

        let mut object_cells = vec![];

        cells.into_iter().for_each(|((x, y), mut cell)| {
            let index = ((y - y_min + y_offset) * size as i32 + (x - x_min + x_offset)) as usize;

            cell.simulation = SimulationType::RigidBody(self.objects.len(), object_cells.len());

            object_cells.push(ObjectPoint {
                texture_coords: vector![
                    ((x - x_min + x_offset) as f32 - size as f32 / 2.0) / PHYSICS_TO_WORLD as f32,
                    ((y - y_min + y_offset) as f32 - size as f32 / 2.0) / PHYSICS_TO_WORLD as f32 
                ],
                cell
            });

            matrix[index] = 1;
        });

        let (collider, _) = create_triangulated_collider(&mut matrix, size as i32);
        
        let rb_handle = self.rigid_body_set.insert(
            if static_flag {
                RigidBodyBuilder::fixed().position(Isometry2::translation((x_min + x_max) as f32 / 2.0 / PHYSICS_TO_WORLD, (y_max + y_min) as f32 / 2.0 / PHYSICS_TO_WORLD))
            }
            else {
                RigidBodyBuilder::dynamic().position(Isometry2::translation((x_min + x_max) as f32 / 2.0 / PHYSICS_TO_WORLD, (y_max + y_min) as f32 / 2.0 / PHYSICS_TO_WORLD))
            }
        );

        let collider_handle = self.collider_set.insert_with_parent(
            collider, 
            rb_handle, 
            &mut self.rigid_body_set
        );

        self.objects.push(
            PhysicsObject { 
                rb_handle, 
                collider_handle,
                cells: object_cells,
                object_size: size
            }
        );
    }

    pub fn new_empty_static_object(&mut self, x: f32, y: f32) -> RigidBodyHandle {
        self.rigid_body_set.insert(
            RigidBodyBuilder::fixed().position(Isometry2::translation(x as f32, y as f32))
        )
    }

    pub fn remove_collider_from_object(&mut self, rb_handle: RigidBodyHandle) {
        let colliders = self.rigid_body_set[rb_handle].colliders().iter().map(|handle| *handle).collect::<Vec<ColliderHandle>>();

        for collider_handle in colliders {
            self.collider_set.remove(
                collider_handle, 
                &mut self.island_manager, 
                &mut self.rigid_body_set, 
                false
            );
        };
    }

    pub fn replace_colliders_to_static_body(&mut self, rb_handle: RigidBodyHandle, colliders: &[(Collider, (f32, f32))]) {
        colliders.iter()
            .for_each(|(collider, _)| {
                self.collider_set.insert_with_parent(
                    collider.clone(),
                    rb_handle,
                    &mut self.rigid_body_set
                );
            })

    }

    pub fn step(&mut self) {
        self.physics_pipeline.step(
            &vector![0.0, 1.0 / PHYSICS_SCALE],
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            None,
            self.physics_hooks.as_ref(),
            self.event_handler.as_ref(),
        );

        self.objects.retain(|object| {
            let position = self.rigid_body_set[object.rb_handle].position().translation;
            let x = position.x * PHYSICS_TO_WORLD / CHUNK_SIZE as f32;
            let y = position.y * PHYSICS_TO_WORLD / CHUNK_SIZE as f32;
            if x < 0.0 || y < 0.0 || x > WORLD_WIDTH as f32 || y > WORLD_HEIGHT as f32 {
                println!("Object left boundaries");
                self.rigid_body_set.remove(
                    object.rb_handle, 
                    &mut self.island_manager, 
                    &mut self.collider_set, 
                    &mut self.impulse_joint_set, 
                    &mut self.multibody_joint_set, 
                    true
                );
                false
            }
            else {
                true
            }
        });
    }

    pub fn rb_to_ca(&self) -> Vec<(&PhysicsObject, Vec<(&ObjectPoint, Pos2)>)> {
        self.objects.iter()
            .map(|object| {
                let rb = &self.rigid_body_set[object.rb_handle];
                (object, rb.position().translation.vector, rb.rotation().angle())
            })
            .map(|(object, center, angle)| {                
                let rotation_matrix = nalgebra::Matrix2::new(
                    angle.cos(), 
                    -angle.sin(), 
                    angle.sin(), 
                    angle.cos()
                );

                (
                    object, 
                    object.cells.iter()
                        .map(|cell| {
                                let position = rotation_matrix * cell.texture_coords + center;
                                (cell, pos2!((position.x * PHYSICS_TO_WORLD).trunc() as i32, (position.y * PHYSICS_TO_WORLD).trunc() as i32))
                            })
                        .collect::<Vec<(&ObjectPoint, Pos2)>>()
                )
            })
            .collect::<Vec<(&PhysicsObject, Vec<(&ObjectPoint, Pos2)>)>>()
    }
}