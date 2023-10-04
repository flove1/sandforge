use std::{collections::{BTreeMap, BTreeSet}, cell::RefCell};

use ahash::{HashMap, HashMapExt};
use dashmap::{DashMap, DashSet};
use rapier2d::{na::Point2, prelude::{vector, nalgebra}};

use crate::{constants::*, pos2, vector::Pos2};
use super::{chunk::{Chunk, ChunkApi}, cell::{Cell, SimulationType}, elements::{MatterType, Element}, physics::Physics, renderer::Renderer, particle::Particle};

pub struct World {
    chunks: DashMap<Pos2, RefCell<Chunk>, ahash::RandomState>,
    active_chunks: DashSet<Pos2, ahash::RandomState>,
    suspended_chunks: DashSet<Pos2, ahash::RandomState>,

    renderer: Renderer,
    physics_engine: Physics,
    previous_update_ms: u128,
    clock: u8,

    particles: Vec<Particle>,
}

impl World {
    pub fn new(
        device: &wgpu::Device, 
        format: &wgpu::TextureFormat
    ) -> World {
        let mut world = Self {
            chunks: DashMap::with_hasher_and_shard_amount(ahash::RandomState::new(), 8),
            active_chunks: DashSet::with_hasher(ahash::RandomState::new()),
            suspended_chunks: DashSet::with_hasher(ahash::RandomState::new()),

            physics_engine: Physics::new(),
            renderer: Renderer::new(device, format),
            previous_update_ms: 0,
            clock: 0,

            particles: vec![],
        };

        for x in 0..WORLD_WIDTH {
            for y in 0..WORLD_HEIGHT {
                let handler = world.physics_engine.new_empty_static_object(((x as f32 + 0.5) * CHUNK_SIZE as f32) as f32 / PHYSICS_TO_WORLD, ((y as f32 + 0.5) * CHUNK_SIZE as f32) as f32 / PHYSICS_TO_WORLD);

                world.chunks.insert(
                    pos2!(x, y), 
                    RefCell::new(Chunk::new(pos2!(x, y), handler))
                );
            }
        }
        
        world
    }
    
    //=====================
    // Interaction with ui
    //=====================

    pub fn set_cell_by_pixel(
        &mut self, 
        x: i32, 
        y: i32, 
        element: &Element
    ) {
        if x < 0 || y < 0 || x >= (WORLD_WIDTH * CHUNK_SIZE) || y >= (WORLD_HEIGHT * CHUNK_SIZE) {
            return;
        }

        let chunk_position = pos2!(x / CHUNK_SIZE, y / CHUNK_SIZE);
        if let Some(chunk) = self.chunks.get(&chunk_position) {
            chunk.borrow_mut().place(x % CHUNK_SIZE, y % CHUNK_SIZE, Cell::new(element, 0), self.clock);
            self.active_chunks.insert(chunk_position);
        }
    }

    pub fn get_cell_by_pixel(
        &mut self, 
        x: i32, 
        y: i32, 
    ) -> Cell {
        if x < 0 || y < 0 || x >= (WORLD_WIDTH * CHUNK_SIZE) || y >= (WORLD_HEIGHT * CHUNK_SIZE) {
            return Cell::default();
        }

        let chunk_position = pos2!(x / CHUNK_SIZE, y / CHUNK_SIZE);
        if let Some(chunk) = self.chunks.get(&chunk_position) {
            chunk.borrow_mut().get_cell(pos2!(x % CHUNK_SIZE, y % CHUNK_SIZE))
        }
        else {
            Cell::default()
        }
    }

    pub fn place_batch(&mut self, cells: Vec<((i32, i32), Cell)>) {
        let mut groups_by_chunks: HashMap<Pos2, Vec<((i32, i32), Cell)>> = HashMap::default();
        
        cells.into_iter()
            .filter(|(pos, _)| {
                pos.0 >= 0 && pos.1 >= 0 && pos.0 < (WORLD_WIDTH * CHUNK_SIZE) && pos.1 < (WORLD_HEIGHT * CHUNK_SIZE)
            })
            .for_each(|(pos, cell)| {
                groups_by_chunks
                    .entry(pos2!(pos.0 / CHUNK_SIZE, pos.1 / CHUNK_SIZE))
                    .or_insert(vec![])
                    .push(((pos.0 % CHUNK_SIZE, pos.1 % CHUNK_SIZE), cell));
            });

        groups_by_chunks.into_iter()
            .for_each(|(chunk_position, cells)| {
                if let Some(result) = self.chunks.get(&chunk_position) {
                    let mut chunk = result.borrow_mut();
                    chunk.place_batch(cells, self.clock);
                    self.active_chunks.insert(chunk_position);

                    if self.physics_engine.has_colliders(chunk.rb_handle) {
                        self.physics_engine.remove_collider_from_object(chunk.rb_handle);

                        if chunk.cell_count != 0 {
                            chunk.create_colliders();
                            self.physics_engine.add_colliders_to_static_body(chunk.rb_handle, &chunk.colliders);
                        }
                    }
                };
            });   
    }

    pub fn place_particles(
        &mut self, 
        positions: Vec<((i32, i32), Cell)>,
    ) {
        self.particles.append(&mut positions.into_iter()
            .map(|(pos, cell)| {
                Particle::new(
                    cell,
                    pos.0 as f32 / CHUNK_SIZE as f32, 
                    pos.1 as f32 / CHUNK_SIZE as f32,
                    0.0,
                    0.0,
                    false
                )
            })
            .collect()
        )
    }    

    //=============
    // Rigidbodies
    //=============

    pub fn place_object(
        &mut self, 
        cells: Vec<((i32, i32), Cell)>, 
        static_flag: bool, 
        device: &wgpu::Device, 
        queue: &wgpu::Queue
    ) {
        self.physics_engine.new_object(cells, static_flag, device, queue);
    }

    //=======================================
    // Interaction of chunks with each other
    //=======================================

    pub(crate) fn update_cell(
        &self, 
        chunk_position: Pos2, 
        cell_position: Pos2, 
        cell: Cell,
    ) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return;
        }

        let result = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk = result.borrow_mut();

        chunk.set_cell(cell_position, cell);
    }    

    pub(crate) fn set_cell(
        &self, 
        chunk_position: Pos2, 
        cell_position: Pos2, 
        cell: Cell,
    ) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return;
        }

        let is_chunk_activated = self.active_chunks.insert(new_chunk_position);        
        
        let result = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk = result.borrow_mut();

        if is_chunk_activated {
            chunk.maximize_dirty_rect();
        }
        else {
            chunk.update_dirty_rect_with_offset(&cell_position);
        }

        chunk.set_cell(cell_position, cell);
    }    

    pub(crate) fn get_cell(
        &self, 
        chunk_position: Pos2, 
        cell_position: Pos2,
    ) -> Cell {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return Cell::default();
        }

        self.chunks.get(&new_chunk_position).unwrap().borrow_mut()
            .get_cell(cell_position).clone()
    }
        
    pub(crate) fn match_cell(
        &self, 
        chunk_position: Pos2, 
        cell_position: Pos2, 
        element: &Element
    ) -> bool {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return true;
        }

        self.chunks.get(&new_chunk_position).unwrap().borrow_mut()
            .match_cell(cell_position, &element)
    }

    pub(crate) fn replace_cell(
        &self, 
        chunk_position: Pos2, 
        cell_offset: Pos2, 
        cell: Cell
    ) -> Cell {
        let (cell_position,chunk_offset ) = cell_offset.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;
        
        if chunk_offset.is_zero() {
            panic!();
        }

        if !self.chunks.contains_key(&new_chunk_position) {
            return Cell::default();
        }

        let is_chunk_activated = self.active_chunks.insert(new_chunk_position);

        let result = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk = result.borrow_mut();
    
        if is_chunk_activated {
            chunk.maximize_dirty_rect();
        }    
        else {
            chunk.update_dirty_rect_with_offset(&cell_position);
        }
        
        let old_cell = chunk.replace_cell(cell_position, cell.clone());
        if old_cell.element.matter != MatterType::Empty && cell.element.matter == MatterType::Empty {
            chunk.cell_count -= 1;
        }
        else if old_cell.element.matter == MatterType::Empty && cell.element.matter != MatterType::Empty {
            chunk.cell_count += 1;
        }
        
        old_cell   
    }

    pub(crate) fn release_chunk(&self, chunk_position: &Pos2) {
        self.active_chunks.remove(chunk_position);
        self.suspended_chunks.insert(*chunk_position);
    }

    pub(crate) fn refresh_chunk(&self, chunk_position: &Pos2, cell_position: &Pos2) {
        let result = self.chunks.get(chunk_position);

        if let Some(result) = result {
            let mut chunk = result.borrow_mut();

            if chunk.cell_count > 0 {
                if !self.active_chunks.contains(&chunk_position) {
                    self.active_chunks.insert(*chunk_position);
                    chunk.maximize_dirty_rect();
                }
                else {
                    chunk.update_dirty_rect_with_offset(cell_position);
                }
            }
        }
    }

    //==========
    // Updating
    //==========

    pub fn needs_update(&mut self, dt: u128) -> bool {
        self.previous_update_ms += dt;
        self.previous_update_ms >= CA_DELAY_MS
    }

    pub fn update(&mut self) -> (u128, u128) {
        let mut chunks_count = 0;
        let mut pixels_count = 0;

        while self.previous_update_ms >= CA_DELAY_MS {
            self.clock = self.clock.wrapping_add(1);    
            self.previous_update_ms -= CA_DELAY_MS;

            self.physics_step();

            let (updated_chunk_count, updated_pixels_count) = self.ca_step();
            chunks_count += updated_chunk_count;
            pixels_count += updated_pixels_count;

            if self.clock % 4 == 0 {
                self.particle_step();
            }
        }

        (chunks_count, pixels_count)
    }

    pub fn forced_update(&mut self) {
        self.ca_step();
        self.physics_step();
        self.particle_step();
    }
    
    fn physics_step(&mut self) {
        self.physics_engine.objects.iter_mut()
            .for_each(|object| {
                let points = &object.cells;
                let mut chunks = HashMap::new();

                points.into_iter()
                    .for_each(|point| {
                        chunks
                            .entry(
                                pos2!(
                                    point.world_coords.x / CHUNK_SIZE, 
                                    point.world_coords.y / CHUNK_SIZE
                                ))
                            .or_insert(vec![])
                            .push(point);
                    });

                chunks.into_iter()
                    .map(|(chunk_position, points)| {
                        (self.chunks.get(&chunk_position), points)
                    })
                    .filter_map(|(result, points)| {
                        if result.is_none() {
                            None
                        }
                        else {
                            Some((result.unwrap(), points))
                        }
                    })
                    .for_each(|(chunk_ref, points)| {
                        let mut chunk = chunk_ref.borrow_mut();

                        points.into_iter()
                            .filter_map(|point| {
                                let pos = pos2!(point.world_coords.x % CHUNK_SIZE, point.world_coords.y % CHUNK_SIZE);

                                if pos.is_between(0, CHUNK_SIZE - 1) {
                                    Some(pos)
                                }
                                else {
                                    None
                                }
                            })
                            .for_each(|pos| {
                                match chunk.get_cell(pos).simulation {
                                    SimulationType::Ca => {},
                                    SimulationType::RigidBody( .. ) => chunk.set_cell(pos, Cell::default()),
                                }
                            });
                    })    
            });
        
        self.physics_engine.step();

        let update_chunks = self.physics_engine.objects.iter_mut()
            .map(|object| {
                let points = &object.cells;
                let mut chunks = HashMap::new();

                points.into_iter()
                    .for_each(|point| {
                        chunks
                            .entry(
                                pos2!(
                                    point.world_coords.x / CHUNK_SIZE, 
                                    point.world_coords.y / CHUNK_SIZE
                                ))
                            .or_insert(vec![])
                            .push(point);
                    });

                chunks.iter()
                    .map(|(chunk_position, points)| {
                        (self.chunks.get(&chunk_position), chunk_position, points)
                    })
                    .filter_map(|(result, chunk_position, points)| {
                        if result.is_none() {
                            None
                        }
                        else {
                            Some((result.unwrap(), chunk_position, points))
                        }
                    })
                    .for_each(|(chunk_ref, chunk_position, points)| {
                        let rb = &mut self.physics_engine.rigid_body_set[object.rb_handle];
                        let mut chunk = chunk_ref.borrow_mut();

                        points.into_iter()
                            .filter_map(|point| {
                                let pos = pos2!(point.world_coords.x % CHUNK_SIZE, point.world_coords.y % CHUNK_SIZE);

                                if pos.is_between(0, CHUNK_SIZE - 1) {
                                    Some((point, pos))
                                }
                                else {
                                    None
                                }
                            })
                            .for_each(|(point, pos)| {
                                let old_cell = chunk.get_cell(pos);

                                match old_cell.element.matter {
                                    MatterType::Empty => {
                                        chunk.set_cell(pos, point.cell.clone());
                                        chunk.update_dirty_rect_with_offset(&pos);
                                    },
                                    MatterType::Static => {},
                                    MatterType::Powder | MatterType::Liquid | MatterType::Gas => {
                                        chunk.set_cell(pos, point.cell.clone());
                                        chunk.update_dirty_rect_with_offset(&pos);

                                        let x = (pos.x + chunk_position.x * CHUNK_SIZE) as f32 / CHUNK_SIZE as f32;
                                        let y = (pos.y + chunk_position.y * CHUNK_SIZE) as f32 / CHUNK_SIZE as f32;

                                        let rb_position = rb.position().translation.vector;

                                        let impulse = (rb_position - vector![x, y]) * 0.02 / rb.mass().sqrt();

                                        rb.apply_impulse_at_point(impulse, Point2::new(rb_position.x, rb_position.y), true);


                                        self.particles.push(Particle { 
                                            cell: old_cell, 
                                            x,
                                            y,
                                            dx: (point.old_world_coords.x as f32 / CHUNK_SIZE as f32 - x) * 5.0 + ((fastrand::f32() - 0.5) * 0.02), 
                                            dy: (point.old_world_coords.y as f32 / CHUNK_SIZE as f32 - y) * 5.0, 
                                            collided: false
                                        })
                                    },
                                }
                            });
                    });

                chunks.into_keys()
            })
            .flat_map(|keys| keys)
            .collect::<Vec<Pos2>>();

        update_chunks.into_iter()
            .for_each(|chunk_position| {
                if let Some(result) = self.chunks.get(&chunk_position) {
                    let mut chunk = result.borrow_mut();

                    if !self.physics_engine.has_colliders(chunk.rb_handle) && chunk.cell_count != 0 {
                        chunk.create_colliders();
                        self.physics_engine.add_colliders_to_static_body(chunk.rb_handle, &chunk.colliders);
                    }
    
                    self.active_chunks.insert(chunk_position);
                }
            })
            
    }

    fn particle_step(&mut self) {
        let mut chunks = HashMap::new();

        self.particles.drain(..)
            .for_each(|particle| {
                let chunk_position = pos2!(
                    (particle.x * CHUNK_SIZE as f32) as i32 / CHUNK_SIZE,
                    (particle.y * CHUNK_SIZE as f32) as i32 / CHUNK_SIZE
                );

                if chunk_position.x >= 0 && chunk_position.x < WORLD_WIDTH && chunk_position.y >= 0 && chunk_position.y < WORLD_HEIGHT {
                    chunks
                        .entry(chunk_position)
                        .or_insert(vec![])
                        .push(particle);
                }
            });

        chunks.into_iter()
            .for_each(|(chunk_position, particles)| {
                if let Some(result) = self.chunks.get(&chunk_position) {
                    let mut chunk = result.borrow_mut();

                    let mut particles = {
                        let mut api = ChunkApi {
                            cell_position: pos2!(0, 0),
                            chunk: &mut chunk,
                            world: &self,
                            clock: self.clock,
                        };
    
                        particles.into_iter()
                            .filter_map(|mut particle| {
                                let pos = pos2!(
                                    (particle.x * CHUNK_SIZE as f32) as i32 % CHUNK_SIZE,
                                    (particle.y * CHUNK_SIZE as f32) as i32 % CHUNK_SIZE
                                );
    
                                api.cell_position = pos;
                                particle.update(&mut api);
    
                                if particle.collided {
                                    None
                                }
                                else {
                                    Some(particle)
                                }
                            })
                            .collect::<Vec<Particle>>()
                    };

                    self.particles.append(&mut particles);
                }
            });

    }

    fn ca_step(&mut self) -> (u128, u128){
        let mut updated_pixels = 0;
        let positions: Vec<Pos2> = self.active_chunks.iter().map(|v| *v).collect();

        let mut groups: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();

        for position in positions.iter() {
            groups.entry(position.x).or_insert(BTreeSet::new()).insert(position.y);
        }

        for (x, group) in groups.iter() {   
            for y in group.iter() {
                let position = &pos2!(*x, *y);
                
                let result = self.chunks.get(&position).unwrap();
                let mut chunk = result.borrow_mut();

                updated_pixels += chunk.update(&self, self.clock);

                if chunk.cell_count == 0 {
                    self.release_chunk(position);
                    self.physics_engine.remove_collider_from_object(chunk.rb_handle);
                }
            }
        }

        (positions.len() as u128, updated_pixels)
    }

    //===========
    // Rendering
    //===========

    pub fn update_textures(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, x: i32, y: i32) {
        let chunk_textures = self.chunks.iter()
            .map(|entry| {
                let mut chunk = entry.value().borrow_mut();
                chunk.create_texture(&device, &queue);

                (
                    chunk.texture.as_ref().unwrap().create_view(&wgpu::TextureViewDescriptor::default()),
                    *entry.key(),
                )
            })
            .collect::<Vec<(wgpu::TextureView, Pos2)>>();

        let objects_textures = self.physics_engine.objects.iter()
            .map(|object| {
                let rb = &self.physics_engine.rigid_body_set[object.rb_handle];

                (
                    object.texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    rb.position().translation.vector,
                    rb.rotation().angle(),
                    object.width,
                    object.height,
                )
            })
            .collect();

        let particles = self.particles.iter()
            .map(|particle| {
                (
                    particle.x,
                    particle.y,
                    particle.cell.element.color
                )
            })
            .collect::<Vec<(f32, f32, [u8; 4])>>();

        self.renderer.update(
            x,
            y,
            device, 
            &self.physics_engine.collider_set, 
            chunk_textures, 
            objects_textures, 
            particles
        );
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        self.renderer.render(encoder, view);
    }
}