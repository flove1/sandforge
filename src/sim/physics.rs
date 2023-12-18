use rapier2d::{prelude::*, na::{Isometry2, Vector2}};

use crate::{constants::{CHUNK_SIZE, PHYSICS_TO_WORLD, WORLD_HEIGHT, WORLD_WIDTH}, pos2, vector::Pos2};

use super::{cell::{Cell, SimulationType}, colliders::create_triangulated_colliders, elements::MatterType};

pub struct PhysicsObject {
    pub rb_handle: RigidBodyHandle,
    pub collider_handle: ColliderHandle,
    pub cells: Vec<ObjectPoint>,
    pub width: i32,
    pub height: i32,
    pub texture: wgpu::Texture,
}

pub struct ObjectPoint {
    pub texture_coords: Vector2<f32>,
    pub world_coords: Pos2,
    pub old_world_coords: Pos2,
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
        }
    }

    // pub fn modify_object(&mut self, object_id: usize, cell_index: usize, cell: Cell) {
    //     self.objects[object_id].matrix[cell_index] = cell;
    // }
    
    pub fn new_object(
        &mut self, 
        cells: Vec<((i32, i32), Cell)>, 
        static_flag: bool,
        device: &wgpu::Device, 
        queue: &wgpu::Queue
    ) {
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

        let (width, height) = {
            (
                x_max - x_min + 1,
                y_max - y_min + 1
            )
        };

        let mut matrix = vec![0; (width * height) as usize];
        let mut object_cells = vec![];
        let mut pixel_data: Vec<u8> = vec![0; (width * height) as usize * 4];

        cells.into_iter()
            .filter(|(_, cell)| {
                cell.matter_type != MatterType::Empty
            })
            .for_each(|((x, y), mut cell)| {
                let index = ((y - y_min) * width + (x - x_min)) as usize;

                cell.simulation = SimulationType::RigidBody(self.objects.len(), object_cells.len());

                let color = cell.get_color();

                object_cells.push(ObjectPoint {
                    texture_coords: vector![
                        ((x - x_min) as f32 - width as f32 / 2.0) / PHYSICS_TO_WORLD,
                        ((y - y_min) as f32 - height as f32 / 2.0) / PHYSICS_TO_WORLD 
                    ],
                    world_coords: pos2!(x, y),
                    old_world_coords: pos2!(x, y),
                    cell
                });

                pixel_data[index * 4] = color[0];
                pixel_data[index * 4 + 1] = color[1];
                pixel_data[index * 4 + 2] = color[2];
                pixel_data[index * 4 + 3] = color[3];

                matrix[index] = 1;
            });

        let mut collider = create_triangulated_colliders(&matrix, width, height);

        collider.set_density(10.0);
        
        let rb_handle = self.rigid_body_set.insert(
            if static_flag {
                RigidBodyBuilder::fixed()
                    .position(Isometry2::translation(
                        (x_min + x_max) as f32 / 2.0 / PHYSICS_TO_WORLD, 
                        (y_max + y_min) as f32 / 2.0 / PHYSICS_TO_WORLD)
                    )
            }
            else {
                RigidBodyBuilder::dynamic()
                    .position(Isometry2::translation(
                        (x_min + x_max) as f32 / 2.0 / PHYSICS_TO_WORLD, 
                        (y_max + y_min) as f32 / 2.0 / PHYSICS_TO_WORLD)
                    )
                    .can_sleep(false)
            }
        );

        let collider_handle = self.collider_set.insert_with_parent(
            collider, 
            rb_handle, 
            &mut self.rigid_body_set
        );

        let extent = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };
        
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Chunk Texture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            // Tells wgpu where to copy the pixel data
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // The actual pixel data
            &pixel_data,
            // The layout of the texture
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width as u32),
                rows_per_image: Some(height as u32),
            },
            extent,
        );  

        self.objects.push(
            PhysicsObject { 
                rb_handle, 
                collider_handle,
                cells: object_cells,
                texture,
                width,
                height
            }
        );
    }

    pub fn new_empty_static_object(&mut self, x: f32, y: f32) -> RigidBodyHandle {
        self.rigid_body_set.insert(
            RigidBodyBuilder::fixed().position(Isometry2::translation(x, y))
        )
    }

    pub fn has_colliders(&mut self, rb_handle: RigidBodyHandle) -> bool {
        !self.rigid_body_set[rb_handle].colliders().is_empty()
    }

    pub fn remove_collider_from_object(&mut self, rb_handle: RigidBodyHandle) {
        let colliders = self.rigid_body_set[rb_handle].colliders().to_vec();

        for collider_handle in colliders {
            self.collider_set.remove(
                collider_handle, 
                &mut self.island_manager, 
                &mut self.rigid_body_set, 
                false
            );
        };
    }

    pub fn add_colliders_to_static_body(&mut self, rb_handle: RigidBodyHandle, colliders: &[Collider]) {
        colliders.iter()
            .for_each(|collider| {
                self.collider_set.insert_with_parent(
                    collider.clone(),
                    rb_handle,
                    &mut self.rigid_body_set
                );
            })
    }

    pub fn step(&mut self, camera_position: [f32; 2]) {
        self.physics_pipeline.step(
            &vector![0.0, -9.8 / PHYSICS_TO_WORLD * 4.0],
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

        self.objects.iter_mut()
            .for_each(|object| {
                let position = self.rigid_body_set[object.rb_handle].position().translation;
                let x = position.x * PHYSICS_TO_WORLD / CHUNK_SIZE as f32;
                let y = position.y * PHYSICS_TO_WORLD / CHUNK_SIZE as f32;

                let rb = &mut self.rigid_body_set[object.rb_handle];
                let out_of_bounds = 
                    x < (camera_position[0] - WORLD_WIDTH as f32 / 2.0) || 
                    y < (camera_position[1] - WORLD_HEIGHT as f32 / 2.0) || 
                    x > (camera_position[0] + WORLD_WIDTH as f32 / 2.0) || 
                    y > (camera_position[1] + WORLD_HEIGHT as f32 / 2.0);
                
                if !rb.is_sleeping() && out_of_bounds {
                    println!("Object left boundaries -> suspending it");
                    rb.sleep();
                }
        });

        self.objects.iter_mut()
            .map(|object| {
                let rb = &self.rigid_body_set[object.rb_handle];
                (object, rb.position().translation.vector, rb.rotation().angle())
            })
            .for_each(|(object, center, angle)| {                
                let rotation_matrix = nalgebra::Matrix2::new(
                    angle.cos(), 
                    -angle.sin(), 
                    angle.sin(), 
                    angle.cos()
                );

                object.cells.iter_mut()
                    .for_each(|cell| {
                        cell.old_world_coords = cell.world_coords;
                        
                        let position = rotation_matrix * cell.texture_coords + center;
                        cell.world_coords = pos2!((position.x * PHYSICS_TO_WORLD).trunc() as i32, (position.y * PHYSICS_TO_WORLD).trunc() as i32);
                    });
            });
    }

    pub fn rb_to_ca(&self) -> Vec<(&PhysicsObject, &Vec<ObjectPoint>)> {
        self.objects.iter()
            .map(|object| {
                (
                    object, 
                    &object.cells
                )
            })
            .collect::<Vec<(&PhysicsObject, &Vec<ObjectPoint>)>>()
    }
}