use std::{sync::Arc, collections::{BTreeMap, BTreeSet}, cell::RefCell};

use ahash::{RandomState, HashMap, HashMapExt};
use dashmap::{DashMap, DashSet};
use parking_lot::Mutex;
use threadpool::ThreadPool;

use crate::{constants::*, pos2, vector::Pos2};
use super::{chunk::Chunk, cell::{Cell, SimulationType}, elements::{MatterType, Element}, physics::Physics, renderer::Renderer};

pub struct World {
    pub(super) chunks: DashMap<Pos2, RefCell<Chunk>, RandomState>,
    pub(super) active_chunks: DashSet<Pos2>,
    pub(super) suspended_chunks: DashSet<Pos2>,
    pub(super) physics_engine: Mutex<Physics>,
    pub(super) renderer: Mutex<Renderer>,
}

impl World {
    pub fn new(device: &wgpu::Device, format: &wgpu::TextureFormat) -> WorldApi {
        let world = Self {
            chunks: DashMap::with_hasher_and_shard_amount(RandomState::new(), 8),
            active_chunks: DashSet::new(),
            suspended_chunks: DashSet::new(),
            physics_engine: Mutex::new(Physics::new()),
            renderer: Mutex::new(Renderer::new(device, format)),
        };

        {
            let mut engine = world.physics_engine.lock();
            
            for x in 0..WORLD_WIDTH {
                for y in 0..WORLD_HEIGHT {
                    let handler = engine.new_empty_static_object(((x as f32 + 0.5) * CHUNK_SIZE as f32) as f32 / PHYSICS_TO_WORLD, ((y as f32 + 0.5) * CHUNK_SIZE as f32) as f32 / PHYSICS_TO_WORLD);
    
                    world.chunks.insert(
                        pos2!(x, y), 
                        RefCell::new(Chunk::new(pos2!(x, y), handler))
                    );
                }
            }
        }

        WorldApi {
            chunk_manager: Arc::new(world), 
            clock: 0, 
            previous_update_ms: 0, 
            pool: ThreadPool::new(4),
        }
    }

    pub(crate) fn place(&self, x: i32, y: i32, element: &Element) {
        if x < 0 || y < 0 || x >= (WORLD_WIDTH * CHUNK_SIZE) || y >= (WORLD_HEIGHT * CHUNK_SIZE) {
            return;
        }

        let chunk_position = pos2!(x / CHUNK_SIZE, y / CHUNK_SIZE);
        if let Some(chunk) = self.chunks.get(&chunk_position) {
            chunk.borrow_mut().place(x % CHUNK_SIZE, y % CHUNK_SIZE, Cell::new(element, 0));
            self.active_chunks.insert(chunk_position);
        }
    }

    pub fn place_batch(&self, cells: Vec<((i32, i32), Cell)>) {
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
                if let Some(chunk) = self.chunks.get(&chunk_position) {
                    chunk.borrow_mut().place_batch(cells);
                    self.active_chunks.insert(chunk_position);
                };
            });   
    }

    pub fn place_particles(&self, _positions: Vec<((i32, i32), Cell)>) {

        // let mut groups_by_chunks: HashMap<Pos2, Vec<((i32, i32), Cell)>> = HashMap::default();
        
        // positions.into_iter()
        //     .filter(|(pos, cell)| {
        //         pos.0 >= 0 && pos.1 >= 0 && pos.0 < (WORLD_WIDTH * CHUNK_SIZE) && pos.1 < (WORLD_HEIGHT * CHUNK_SIZE)
        //     })
        //     .for_each(|(pos, cell)| {
        //         groups_by_chunks
        //             .entry(pos2!(pos.0 / CHUNK_SIZE, pos.1 / CHUNK_SIZE))
        //             .or_insert(vec![])
        //             .push(((pos.0 % CHUNK_SIZE, pos.1 % CHUNK_SIZE), cell));
        //     });

        // groups_by_chunks.into_iter()
        //     .for_each(|(chunk_position, cells)| {
        //         if let Some(chunk) = self.chunks.get(&chunk_position) {
        //             chunk.place_particles(cells, &cells);
        //             self.active_chunks.insert(chunk_position);
        //         };
        //     });   
    }

    //=============
    // Rigidbodies
    //=============

    pub fn place_object(
        &self, 
        cells: Vec<((i32, i32), Cell)>, 
        static_flag: bool, 
        device: &wgpu::Device, 
        queue: &wgpu::Queue
    ) {
        self.physics_engine.lock().new_object(cells, static_flag, device, queue);
        // self.place_batch(cells);
    }

    // pub fn modify_object(&self, object_id: usize, cell_index: usize, cell: Cell) {
    //     self.physics_engine.lock().modify_object(object_id, cell_index, cell);
    // }

    //=======================================
    // Interaction of chunks with each other
    //=======================================

    pub(crate) fn update_cell(&self, chunk_position: Pos2, cell_position: Pos2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return;
        }

        let result = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk = result.borrow_mut();

        chunk.chunk_data.set_cell(cell_position, cell);
    }    

    pub(crate) fn set_cell(&self, chunk_position: Pos2, cell_position: Pos2, cell: Cell) {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return;
        }

        let is_chunk_activated = self.active_chunks.insert(new_chunk_position);        
        
        let result = self.chunks.get(&new_chunk_position).unwrap();
        let mut chunk = result.borrow_mut();

        if is_chunk_activated {
            chunk.chunk_data.maximize_dirty_rect();
        }
        else {
            chunk.chunk_data.update_dirty_rect(&cell_position);
        }

        chunk.chunk_data.set_cell(cell_position, cell);
    }    

    pub(crate) fn get_cell(&self, chunk_position: Pos2, cell_position: Pos2) -> Cell {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return Cell::default();
        }

        self.chunks.get(&new_chunk_position).unwrap().borrow_mut()
            .chunk_data.get_cell(cell_position).clone()
    }
        

    pub(crate) fn match_cell(&self, chunk_position: Pos2, cell_position: Pos2, element: &Element) -> bool {
        let (cell_position, chunk_offset) = cell_position.wrap(0, CHUNK_SIZE);
        let new_chunk_position = chunk_position + chunk_offset;

        if !self.chunks.contains_key(&new_chunk_position) {
            return true;
        }

        self.chunks.get(&new_chunk_position).unwrap().borrow_mut()
            .chunk_data.match_cell(cell_position, &element)
    }

    pub(crate) fn replace_cell(&self, chunk_position: Pos2, cell_offset: Pos2, cell: Cell) -> Cell {
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
            chunk.chunk_data.maximize_dirty_rect();
        }    
        else {
            chunk.chunk_data.update_dirty_rect(&cell_position);
        }
        
        let old_cell = chunk.chunk_data.replace_cell(cell_position, cell.clone());
        if old_cell.element.matter != MatterType::Empty && cell.element.matter == MatterType::Empty {
            chunk.cell_count -= 1;
        }
        else if old_cell.element.matter == MatterType::Empty && cell.element.matter != MatterType::Empty {
            chunk.cell_count += 1;
        }
        
        old_cell   
    }

    // pub fn move_particle(&self, chunk_position: Pos2, mut particle: Cell) {
    //     match &mut particle.simulation {
    //         SimulationType::Particle { x, y, .. } => {
    //             let ix = x.floor() as i32;
    //             let iy = y.floor() as i32;
    //             let cell_ix = (*x * CHUNK_SIZE as f32).floor() as i32;
    //             let cell_iy = (*y * CHUNK_SIZE as f32).floor() as i32;

    //             *x -= ix as f32;
    //             *y -= iy as f32;

    //             let cell_position = pos2!(cell_ix, cell_iy);
    //             let new_chunk_position = chunk_position + pos2!(ix, iy);

    //             if let Some(chunk) = self.chunks.get(&new_chunk_position) { 
    //                 let mut chunk_data = chunk.chunk_data.write().unwrap();
    
    //                 let is_chunk_activated = self.active_chunks.insert(new_chunk_position);
        
    //                 if is_chunk_activated {
    //                     chunk_data.maximize_dirty_rect();
    //                 }    
    //                 else {
    //                     chunk_data.update_dirty_rect(&cell_position);
    //                 }
    
    //                 chunk.particles.lock().unwrap().push(particle);
    //             }
    //         },
    //         _ => panic!()
    //     } 
    // }

    pub fn release_chunk(&self, chunk_position: &Pos2) {
        self.active_chunks.remove(chunk_position);
        self.suspended_chunks.insert(*chunk_position);
    }

    pub fn refresh_chunk(&self, chunk_position: &Pos2, cell_position: &Pos2) {
        let result = self.chunks.get(chunk_position);

        if let Some(result) = result {
            let mut chunk = result.borrow_mut();

            if chunk.cell_count > 0 {
                if !self.active_chunks.contains(&chunk_position) {
                    self.active_chunks.insert(*chunk_position);
                    chunk.maximize_dirty_rect();
                }
                else {
                    chunk.update_dirty_rect(cell_position);
                }
            }
        }
    }

    pub fn physics_step(&self) {
        let mut engine = self.physics_engine.lock();
        
        engine.rb_to_ca().into_iter()
            .for_each(|(_, cells)| {
                let mut chunks = HashMap::new();

                cells.into_iter()
                    .for_each(|cell| {
                        chunks.entry(pos2!(cell.1.x / CHUNK_SIZE, cell.1.y / CHUNK_SIZE))
                            .or_insert(vec![])
                            .push(cell);
                    });

                chunks.into_iter()
                    .for_each(|(chunk_position, cells)| {
                        if let Some(result) = self.chunks.get(&chunk_position) {
                            let mut chunk = result.borrow_mut();

                            cells.into_iter()
                                .for_each(|point| {
                                    let pos = pos2!(point.1.x % CHUNK_SIZE, point.1.y % CHUNK_SIZE);
                                    let old_cell = chunk.chunk_data.get_cell(pos);
    
                                    if let SimulationType::RigidBody(_, _) = old_cell.simulation {
                                        chunk.chunk_data.set_cell(pos, Cell::default())
                                    }
                                });

                        }
                    })    
            });
        
        engine.step();

        engine.rb_to_ca().into_iter()
            .for_each(|(_, cells)| {
                let mut chunks = HashMap::new();

                cells.into_iter()
                    .for_each(|cell| {
                        chunks.entry(pos2!(cell.1.x / CHUNK_SIZE, cell.1.y / CHUNK_SIZE))
                            .or_insert(vec![])
                            .push(cell);
                    });

                chunks.into_iter()
                    .for_each(|(chunk_position, cells)| {
                        if let Some(result) = self.chunks.get(&chunk_position) {
                            let mut chunk = result.borrow_mut();

                            cells.into_iter()
                                .for_each(|point| {
                                    let pos = pos2!(point.1.x % CHUNK_SIZE, point.1.y % CHUNK_SIZE);
                                    let old_cell = chunk.chunk_data.get_cell(pos);
    
                                    // if let SimulationType::RigidBody(_, _) = old_cell.simulation {
                                    //     chunk.chunk_data.set_cell(pos, EMPTY_CELL.clone())
                                    // }
    
                                    if old_cell.element.matter == MatterType::Empty {
                                        chunk.chunk_data.set_cell(pos, point.0.cell.clone());
                                        chunk.chunk_data.update_dirty_rect(&pos);
                                    }
                                });
    
                            self.active_chunks.insert(chunk_position);
                        }
                    })
            });
    }
}

pub struct WorldApi {
    pub chunk_manager: Arc<World>,
    previous_update_ms: u128,
    clock: u8,
    pool: ThreadPool,
}

impl WorldApi {
    //============
    // Simulation
    //============
    pub fn needs_update(&mut self, dt: u128) -> bool {
        self.previous_update_ms += dt;
        self.previous_update_ms >= CA_DELAY_MS
    }

    pub fn update(&mut self) -> (u128, u128) {
        let mut chunks_count = 0;
        let mut pixels_count = 0;
        while self.previous_update_ms >= CA_DELAY_MS {
            self.previous_update_ms -= CA_DELAY_MS;
            let (updated_chunk_count, updated_pixels_count) = self.update_iteration();
            chunks_count += updated_chunk_count;
            pixels_count += updated_pixels_count;
        }

        (chunks_count, pixels_count)
    }

    pub fn update_iteration(&mut self) -> (u128, u128){
        self.clock = self.clock.wrapping_add(1);

        let mut updated_pixels = 0;
        let positions: Vec<Pos2> = self.chunk_manager.active_chunks.iter().map(|v| *v).collect();

        let mut groups: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();

        for position in positions.iter() {
            groups.entry(position.x).or_insert(BTreeSet::new()).insert(position.y);
        }

        for (x, group) in groups.iter() {   
            for y in group.iter() {
                let position = &pos2!(*x, *y);
                
                let result = self.chunk_manager.chunks.get(&position).unwrap();
                let mut chunk = result.borrow_mut();

                updated_pixels += chunk.update(self.chunk_manager.clone(), self.clock);
            }
        }

        {
            let mut engine = self.chunk_manager.physics_engine.lock();
    
            for (x, group) in groups.iter() {   
                for y in group.iter() {
                    let position = &pos2!(*x, *y);
                
                    let result = self.chunk_manager.chunks.get(&position).unwrap();
                    let mut chunk = result.borrow_mut();

                    let rb_handle = chunk.chunk_data.rb_handle.unwrap();
    
                    engine.remove_collider_from_object(rb_handle);

                    if chunk.cell_count == 0 && chunk.particles.len() == 0 {
                        self.chunk_manager.release_chunk(position);
                    }
                    else if chunk.cell_count != 0 {
                        chunk.create_colliders();
                        engine.replace_colliders_to_static_body(rb_handle, &chunk.chunk_data.colliders);
                    }
                }
            }
        }

        self.chunk_manager.physics_step();

        (positions.len() as u128, updated_pixels)
    }

    //=====================
    // Interaction with ui
    //=====================

    pub fn place(&self, x: i32, y: i32, element: &Element) {
        self.chunk_manager.place(x, y, element);
    }

    pub fn place_batch(&self, cells: Vec<((i32, i32), Cell)>) {
        self.chunk_manager.place_batch(cells);
    }

    pub fn place_particles(&self, cells: Vec<((i32, i32), Cell)>) {
        self.chunk_manager.place_particles(cells);
    }

    pub fn place_object(
        &self, 
        cells: Vec<((i32, i32), Cell)>, 
        static_flag: bool,
        device: &wgpu::Device,
        queue: &wgpu::Queue
    ) {
        self.chunk_manager.place_object(cells, static_flag, device, queue);
    }

    //===========
    // Rendering
    //===========

    pub fn update_textures(&self, device: &wgpu::Device, queue: &wgpu::Queue) {                
        let chunk_textures = self.chunk_manager.chunks.iter()
            .map(|entry| {
                let mut chunk = entry.value().borrow_mut();
                chunk.create_texture(&device, &queue);

                (
                    chunk.texture.as_ref().unwrap().create_view(&wgpu::TextureViewDescriptor::default()),
                    *entry.key(),
                )
            })
            .collect::<Vec<(wgpu::TextureView, Pos2)>>();

        let physics_engine = self.chunk_manager.physics_engine.lock();
        let objects_textures = physics_engine.objects.iter()
            .map(|object| {
                let rb = &physics_engine.rigid_body_set[object.rb_handle];

                (
                    object.texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    rb.position().translation.vector,
                    rb.rotation().angle(),
                    object.width,
                    object.height,
                )
            })
            .collect();

        self.chunk_manager.renderer.lock().update(device, &physics_engine.collider_set, chunk_textures, objects_textures);
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        self.chunk_manager.renderer.lock().render(encoder, view);
    }
}