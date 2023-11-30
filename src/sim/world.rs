use std::{collections::{BTreeMap, BTreeSet}, cell::RefCell};

use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use dashmap::{DashMap, DashSet};
use rapier2d::{na::Point2, prelude::{vector, nalgebra}};

use crate::{constants::*, pos2, vector::Pos2, helpers::line_from_pixels};
use super::{chunk::{Chunk, ChunkApi}, cell::{Cell, SimulationType}, elements::{MatterType, Element, ELEMENTS}, physics::Physics, renderer::Renderer, particle::Particle};

fn terrain_fx(x: f64) -> f64 {
    let term1 = (x / 1.5).sin() * (x / 1.5).cos() * x.sin();
    let term2 = x / 8.0;
    let term3 = 0.5;
    
    term1 + term2 + term3
}

fn surface_offset_fx() -> i32 {
    let chance = fastrand::i32(0..2);

    if chance == 0 {
        let random_value = fastrand::i32(2..=4);
        return random_value;
    } else {
        return 2;
    }
}

pub struct World {
    chunks: DashMap<Pos2, RefCell<Chunk>, ahash::RandomState>,
    active_chunks: DashSet<Pos2, ahash::RandomState>,

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
            chunks: DashMap::with_hasher(ahash::RandomState::new()),
            active_chunks: DashSet::with_hasher(ahash::RandomState::new()),

            physics_engine: Physics::new(),
            renderer: Renderer::new(device, format),
            previous_update_ms: 0,
            clock: 0,

            particles: vec![],
        };

        for x in 0..WORLD_WIDTH {
            for y in 0..WORLD_HEIGHT {
                world.add_chunk(pos2!(x, y));
            }
        }
        
        world
    }
    
    //=====================
    // Interaction with ui
    //=====================

    pub fn add_chunk(&mut self, position: Pos2) {
        let handler = self.physics_engine.new_empty_static_object(((position.x as f32 + 0.5) * CHUNK_SIZE as f32) / PHYSICS_TO_WORLD, ((position.y as f32 + 0.5) * CHUNK_SIZE as f32) / PHYSICS_TO_WORLD);

        let mut chunk = Chunk::new(position, handler);

        let underground_element = ELEMENTS.get("dirt").unwrap().value().clone();
        let surface_element = ELEMENTS.get("grass").unwrap().value().clone();

        for x in 0..CHUNK_SIZE {
            let max_y = terrain_fx(x as f64 / CHUNK_SIZE as f64 + position.x as f64);

            let surface_level = ((max_y - position.y as f64) * CHUNK_SIZE as f64).floor() as i32;

            let surface_offset_1 = surface_offset_fx();
            let surface_offset_2 = surface_offset_fx();

            for y in 0..(surface_level - surface_offset_1) {
                chunk.place(x, y, Cell::new(&underground_element, 1), self.clock);
            }

            for y in (surface_level - surface_offset_1)..(surface_level + surface_offset_2) {
                chunk.place(x, y, Cell::new(&surface_element, 1), self.clock);
            }
        };

        self.chunks.insert(
            position, 
            RefCell::new(chunk)
        );
    }

    pub fn set_cell_by_pixel(
        &mut self, 
        x: i32, 
        y: i32, 
        element: &Element
    ) {
        let chunk_position = pos2!(x.div_euclid(CHUNK_SIZE) , y.div_euclid(CHUNK_SIZE));

        let chunk_reference = {
            if !self.chunks.contains_key(&chunk_position) {
                self.add_chunk(chunk_position);
            }
            self.chunks.get(&chunk_position).unwrap()
        };

        chunk_reference.borrow_mut().place(x.rem_euclid(CHUNK_SIZE), y.rem_euclid(CHUNK_SIZE), Cell::new(element, 0), self.clock);
        self.active_chunks.insert(chunk_position);
    }

    pub fn get_cell_by_pixel(
        &mut self, 
        x: i32, 
        y: i32, 
    ) -> Cell {
        let chunk_position = pos2!(x.div_euclid(CHUNK_SIZE) , y.div_euclid(CHUNK_SIZE));

        if let Some(chunk) = self.chunks.get(&chunk_position) {
            chunk.borrow_mut().get_cell(pos2!(x.rem_euclid(CHUNK_SIZE), y.rem_euclid(CHUNK_SIZE)))
        }
        else {
            Cell::default()
        }
    }

    pub fn place_batch(&mut self, cells: Vec<((i32, i32), Cell)>) {
        let mut groups_by_chunks = HashMap::default();
        
        cells.into_iter()
            .for_each(|(pos, cell)| {
                let chunk_position = pos2!(pos.0.div_euclid(CHUNK_SIZE), pos.1.div_euclid(CHUNK_SIZE));
                self.chunks.contains_key(&chunk_position);

                groups_by_chunks
                    .entry(chunk_position)
                    .or_insert(vec![])
                    .push(((pos.0.rem_euclid(CHUNK_SIZE), pos.1.rem_euclid(CHUNK_SIZE)), cell));
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

    pub fn get_chunk(&self, chunk_position: &Pos2) -> Option<dashmap::mapref::one::Ref<'_, Pos2, RefCell<Chunk>, ahash::RandomState>> {
        self.chunks.get(chunk_position)
    }

    pub fn activate_chunk(&self, chunk_position: Pos2) -> bool {
        self.active_chunks.insert(chunk_position)
    }

    pub(crate) fn refresh_chunk(&self, chunk_position: &Pos2, cell_position: &Pos2) {
        let result = self.chunks.get(chunk_position);

        if let Some(result) = result {
            let mut chunk = result.borrow_mut();

            if chunk.cell_count > 0 {
                if !self.active_chunks.contains(chunk_position) {
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

    pub fn update(&mut self, visible_world: [f32; 4]) -> (u128, u128) {
        let mut chunks_count = 0;
        let mut pixels_count = 0;

        let bl_corner = [visible_world[0].floor() as i32, visible_world[1].floor() as i32];
        let tr_corner = [visible_world[2].ceil() as i32, visible_world[3].ceil() as i32];

        self.active_chunks.retain(|position| {
            position.x >= bl_corner[0] && position.x < tr_corner[0] && position.y >= bl_corner[1] && position.y < bl_corner[1]
        });

        for x in bl_corner[0]..tr_corner[0] {
            for y in bl_corner[1]..tr_corner[1] {
                let position = pos2!(x, y);
                if !self.active_chunks.contains(&position) {
                    self.activate_chunk(position);

                    if !self.chunks.contains_key(&position) {
                        self.add_chunk(position);
                    }
                }
            }
        }

        while self.previous_update_ms >= CA_DELAY_MS {
            self.clock = self.clock.wrapping_add(1);    
            self.previous_update_ms -= CA_DELAY_MS;

            self.physics_step(visible_world);

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
        self.physics_step([0.0; 4]);
        self.particle_step();
    }
    
    fn physics_step(&mut self, visible_world: [f32; 4]) {
        self.physics_engine.objects.iter_mut()
            .for_each(|object| {
                let points = &object.cells;
                let mut chunks = HashMap::new();

                points.iter()
                    .for_each(|point| {
                        chunks
                            .entry(
                                pos2!(
                                    point.world_coords.x.div_euclid(CHUNK_SIZE), 
                                    point.world_coords.y.div_euclid(CHUNK_SIZE)
                                ))
                            .or_insert(vec![])
                            .push(
                                pos2!(
                                    point.world_coords.x.rem_euclid(CHUNK_SIZE),
                                    point.world_coords.y.rem_euclid(CHUNK_SIZE)
                                )
                            );
                    });

                chunks.into_iter()
                    .map(|(chunk_position, points)| {
                        (self.chunks.get(&chunk_position), points)
                    })
                    .filter_map(|(result, points)| {
                        result.map(|result| (result, points))
                    })
                    .for_each(|(chunk_ref, points)| {
                        let mut chunk = chunk_ref.borrow_mut();

                        points.into_iter()
                            .filter(|pos| pos.is_between(0, CHUNK_SIZE - 1))
                            .for_each(|pos| {
                                match chunk.get_cell(pos).simulation {
                                    SimulationType::Ca => {},
                                    SimulationType::RigidBody( .. ) => chunk.set_cell(pos, Cell::default()),
                                    SimulationType::Particle(_, _) => {},
                                }
                            });
                    })    
            });
        
        self.physics_engine.step(visible_world);

        let mut updated_chunks_pos = HashSet::new();
        let mut displaced_cells = HashMap::new();

        self.physics_engine.objects.iter_mut()
            .for_each(|object| {
                let points = &object.cells;
                let mut chunks = HashMap::new();
        
                points.iter()
                    .for_each(|point| {
                        chunks
                            .entry(
                                pos2!(
                                    point.world_coords.x.div_euclid(CHUNK_SIZE), 
                                    point.world_coords.y.div_euclid(CHUNK_SIZE)
                                ))
                            .or_insert(vec![])
                            .push(
                                (
                                    pos2!(
                                        point.world_coords.x.rem_euclid(CHUNK_SIZE),
                                        point.world_coords.y.rem_euclid(CHUNK_SIZE)
                                    ),
                                    point.cell.clone()
                                )
                            );
                    });
    
                chunks.iter()
                    .map(|(chunk_position, points)| {
                        (self.chunks.get(chunk_position), chunk_position, points)
                    })
                    .filter_map(|(result, chunk_position, points)| {
                        result.map(|result| (result, chunk_position, points))
                    })
                    .for_each(|(chunk_ref, chunk_position, points)| {
                        let mut chunk = chunk_ref.borrow_mut();
                        let rb_handle = object.rb_handle;
    
                        points.into_iter()
                            .filter(|(pos, _)| pos.is_between(0, CHUNK_SIZE - 1))
                            .for_each(|(pos, cell)| {
                                let mut old_cell = chunk.get_cell(pos.clone());
    
                                match old_cell.matter_type {
                                    MatterType::Empty => {
                                        chunk.set_cell(pos.clone(), cell.clone());
                                        chunk.update_dirty_rect_with_offset(&pos);
                                    },
                                    MatterType::Static => {},
                                    MatterType::Powder | MatterType::Liquid | MatterType::Gas => {
                                        chunk.set_cell(pos.clone(), cell.clone());
                                        chunk.update_dirty_rect_with_offset(&pos);
    
                                        let x = (pos.x + chunk_position.x * CHUNK_SIZE) as f32 / CHUNK_SIZE as f32;
                                        let y = (pos.y + chunk_position.y * CHUNK_SIZE) as f32 / CHUNK_SIZE as f32;
    
                                        let rb = &mut self.physics_engine.rigid_body_set[rb_handle];
                                        let rb_position = rb.position().translation.vector;
    
                                        let impulse = (rb_position - vector![x, y]) * 0.02 / rb.mass().sqrt();
    
                                        rb.apply_impulse_at_point(impulse, Point2::new(rb_position.x, rb_position.y), true);

                                        old_cell.simulation = SimulationType::Particle(fastrand::f32() - 0.5, 1.0);
    
                                        displaced_cells
                                            .entry(
                                                chunk_position.clone())
                                            .or_insert(vec![])
                                            .push(
                                                (pos.clone(), old_cell)
                                            );
                                    },
                                }
                            });
                    });
    
                updated_chunks_pos.extend(chunks.into_keys());
    
            });

        /*
            TODO: Rework cell displacement on collision with rigidbodies
            Looks like shit, but it's fine for now
            Ideally should be similart to this:
            https://user-images.githubusercontent.com/13819558/205466863-e7aabf18-e122-4302-afb2-71b2953c236f.gif
        */
        displaced_cells.into_iter()
            .for_each(|(chunk_pos, particles)| {
                let chunk_ref = self.chunks.get(&chunk_pos).unwrap();
                let mut chunk = chunk_ref.borrow_mut();
                
                let mut api = ChunkApi {
                    cell_position: pos2!(0, 0),
                    chunk: &mut chunk,
                    world: self,
                    clock: self.clock,
                };

                particles.into_iter()
                    .for_each(|(pos, mut cell)| {
                        if let SimulationType::Particle(dx, dy) = &mut cell.simulation {
                                api.cell_position = pos;
                                let mut last_x = 0;
                                let mut last_y = 0;
                        
                                let mut operation = |current_dx, current_dy| {
                                    let current_cell = api.get(last_x, last_y);
                        
                                    if !matches!(current_cell.matter_type, MatterType::Empty) {
                                        last_x = current_dx;
                                        last_y = current_dy;
                                    }

                                    !matches!(current_cell.matter_type, MatterType::Empty)
                                };

                                let return_to_ca = line_from_pixels(
                                    0, 
                                    0, 
                                    (*dx * CHUNK_SIZE as f32).round() as i32, 
                                    (*dy * CHUNK_SIZE as f32).round() as i32, 
                                    &mut operation
                                );
                        
                                if return_to_ca {
                                    api.set(last_x, last_y, Cell { 
                                        simulation: SimulationType::Ca,
                                        ..cell.clone()
                                    });
                                }
                                else {
                                    let mut break_loop = false;
                                    for dx in -1..=1 {
                                        if break_loop {
                                            break;
                                        }
                                        for dy in -1..=1 {
                                            if api.get(last_x + dx, last_y + dy).matter_type == MatterType::Empty {
                                                api.set(last_x + dx, last_y + dy, Cell { 
                                                    simulation: SimulationType::Ca,
                                                    ..cell.clone()
                                                });
                                                break_loop = true;
                                            }
                                        }
                                    }
                                }

                            }
                        else {
                            panic!()
                        }
                    })
            });

        updated_chunks_pos.into_iter()
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
                            world: self,
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
                                    let mut placed = false;
                                    for dx in -1..=1 {
                                        for dy in -1..=1 {
                                            if !placed && api.get(dx, dy).matter_type == MatterType::Empty {
                                                placed = true;
                                                api.set(dx, dy, particle.cell.clone());
                                            }
                                        }  
                                    }

                                    if placed {
                                        None
                                    }
                                    else {
                                        Some(particle)
                                    }
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

                let result = self.chunks.get(position).unwrap();
                let mut chunk = result.borrow_mut();

                updated_pixels += chunk.update(self, self.clock);

                if chunk.cell_count == 0 {
                    self.physics_engine.remove_collider_from_object(chunk.rb_handle);
                }
            }
        }

        (positions.len() as u128, updated_pixels)
    }

    //===========
    // Rendering
    //===========

    pub fn update_textures(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, screen_coords: [f32; 4]) {
        let chunk_textures = self.active_chunks.iter()
            .map(|position| {
                let chunk_reference = self.chunks.get(&position).unwrap();
                let mut chunk = chunk_reference.value().borrow_mut();
                chunk.create_texture(device, queue);

                (
                    chunk.texture.as_ref().unwrap().create_view(&wgpu::TextureViewDescriptor::default()),
                    *position,
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
                    particle.cell.color
                )
            })
            .collect::<Vec<(f32, f32, [u8; 4])>>();

        self.renderer.update(
            screen_coords,
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