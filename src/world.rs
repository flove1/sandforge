use std::{collections::{BTreeMap, BTreeSet}, time::{Duration, SystemTime, UNIX_EPOCH}};

use async_channel::Sender;

use bevy::{prelude::*, sprite::Anchor, tasks::ComputeTaskPool, time::common_conditions::on_timer, utils::HashMap};
use bevy_math::{ivec2, vec2, IVec2, Rect, URect, UVec2, Vec2, Vec3Swizzles};
use dashmap::DashSet;
use itertools::{Either, Itertools};
use noise::{NoiseFn, SuperSimplex};

use crate::{chunk::{ChunkApi, ChunkData, ChunkGroup}, constants::*, dirty_rect::{dirty_rects_gizmos, update_dirty_rects, update_dirty_rects_3x3, DirtyRects, RenderMessage, UpdateMessage}, materials::{update_gas, update_liquid, update_sand}, registries::{self, Registries}};
use super::{pixel::Pixel, materials::PhysicsType};

#[derive(Resource)]
pub struct Noise(SuperSimplex);

#[derive(Component)]
pub struct Chunks;

impl FromWorld for Noise {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(
            SuperSimplex::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap()
                .subsec_millis()
            )
        )
    }
}

#[derive(Resource)]
pub struct ChunkManager {
    pub chunks: HashMap<IVec2, ChunkData>,
    active_chunks: DashSet<IVec2, ahash::RandomState>,
    clock: u8,
}

impl FromWorld for ChunkManager {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            chunks: HashMap::new(),
            active_chunks: DashSet::with_hasher(ahash::RandomState::new()),
            clock: 0,
        }        
    }
}

impl std::ops::Index<IVec2> for ChunkManager {
    type Output = Pixel;

    fn index(&self, index: IVec2) -> &Self::Output {
        self.get(index).expect("pixel not exists")
    }
}

impl std::ops::IndexMut<IVec2> for ChunkManager {
    fn index_mut(&mut self, index: IVec2) -> &mut Self::Output {
        self.get_mut(index).expect("pixel not exists")
    }
}

impl ChunkManager {
    pub fn add_chunk(
        &mut self,
        commands: &mut Commands,
        images: &mut ResMut<Assets<Image>>,
        registries: &Res<Registries>,
        noise: &Res<Noise>,
        chunk_position: IVec2,
        chunks: &Entity,
    ) {
        let mut chunk = ChunkData::new(None);

        let underground_element = registries.materials.get("dirt").unwrap();
        let surface_element = registries.materials.get("grass").unwrap();
        let depth_element = registries.materials.get("stone").unwrap();
        
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let mut value = noise.0.get([
                    x as f64 / CHUNK_SIZE as f64 + chunk_position.x as f64, 
                    y as f64 / CHUNK_SIZE as f64 + chunk_position.y as f64
                ]);

                let cell_position = ivec2(x, y);

                value *= 10.0;

                match value as i64 {
                    0..=1 => chunk[cell_position] = Pixel::new(surface_element, 1),
                    2..=4 => chunk[cell_position] = Pixel::new(underground_element, 1),
                    5..=10 => chunk[cell_position] = Pixel::new(depth_element, 1),
                    _ => {}
                }
            }
        };

        let image_handle = images.add(ChunkData::new_image());
        let entity_id = commands
            .spawn(SpriteBundle {
                texture: image_handle.clone(),
                sprite: Sprite {
                    custom_size: Some(vec2(1.0, 1.0)),
                    anchor: Anchor::BottomLeft,
                    flip_y: true,
                    ..Default::default()
                },
                global_transform: GlobalTransform::from_xyz(chunk_position.x as f32, chunk_position.y as f32, 0.),
                ..Default::default()
            })
            .id();
        
        commands.entity(*chunks).push_children(&[entity_id]);

        chunk.texture = image_handle.clone();
        chunk.entity = Some(entity_id);

        chunk.update_all(images.get_mut(&image_handle.clone()).unwrap());

        self.chunks.insert(
            chunk_position, 
            chunk
        );
    }

    pub fn get(&self, pos: IVec2) -> Option<&Pixel> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.chunks.get(&chunk_position).map(|chunk| &chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
    }

    pub fn get_mut(&mut self, pos: IVec2) -> Option<&mut Pixel> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if let Some(chunk) = self.chunks.get_mut(&chunk_position) {
            Some(&mut chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
        }
        else {
            None
        }
    }

    pub fn check_collision(&self, pos: IVec2) -> Option<&PhysicsType> {
        self.get(pos)
            .filter(|pixel| {
                matches!(pixel.material.matter_type, PhysicsType::Static | PhysicsType::Powder)
            })
            .map_or(Some(&PhysicsType::Static) , |pixel| Some(&pixel.material.matter_type))
    }

    pub fn replace_cell_at(&mut self, pos: IVec2, pixel: Pixel) {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if let Some(chunk) = self.chunks.get_mut(&chunk_position) {
            chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)] = pixel;
        }
    }

    // pub fn place_particles(
    //     &mut self, 
    //     positions: Vec<((i32, i32), Pixel)>,
    // ) {
    //     self.particles.append(&mut positions.into_iter()
    //         .map(|((x, y), pixel)| {
    //             Particle::new(
    //                 pixel,
    //                 x as f32 / CHUNK_SIZE as f32, 
    //                 - y as f32 / CHUNK_SIZE as f32,
    //                 0.0,
    //                 0.0,
    //                 false
    //             )
    //         })
    //         .collect()
    //     )
    // }    

    //=============
    // Rigidbodies
    //=============

    // pub fn place_object(
    //     &mut self, 
    //     cells: Vec<((i32, i32), Pixel)>, 
    //     static_flag: bool, 
    //     device: &wgpu::Device, 
    //     queue: &wgpu::Queue
    // ) {
    //     self.physics_engine.new_object(cells, static_flag, device, queue);
    // }

    // pub fn delete_object(
    //     &mut self,
    //     x: i32,
    //     y: i32
    // ) {
    //     if let SimulationType::RigidBody(object_id, _) = self.get_cell_by_pixel(x, y).simulation {
    //         if let Some(object) = self.physics_engine.objects.remove(&object_id) {
    //             for point in object.cells {
    //                 if let SimulationType::RigidBody(cell_object_id, _) = self.get_cell_by_pixel(x, y).simulation {
    //                     if object_id == cell_object_id {
    //                         self.set_cell_by_pixel(point.world_coords.x, point.world_coords.y, Pixel::default(), true);
    //                     }
    //                 }
    //             }
    
    //             self.physics_engine.delete_object(object.rb_handle);
    //         }
    //     }

    // }

    pub fn get_chunk(&self, chunk_position: &IVec2) -> Option<&ChunkData> {
        self.chunks.get(chunk_position)
    }

    pub fn get_chunk_mut(&mut self, chunk_position: &IVec2) -> Option<&mut ChunkData> {
        self.chunks.get_mut(chunk_position)
    }

    pub fn activate_chunk(&self, chunk_position: IVec2) -> bool {
        self.active_chunks.insert(chunk_position)
    }

    pub fn update_loaded_chunks(
        &mut self, 
        commands: &mut Commands,
        images: &mut ResMut<Assets<Image>>,
        registries: &Res<Registries>,
        noise: &Res<Noise>,
        camera_position: Vec2,
        chunks: &Entity,
    ) {
        let area = Rect::from_center_size(
            camera_position, 
            vec2(
                WORLD_WIDTH as f32,
                WORLD_HEIGHT as f32,
            )
        );

        self.active_chunks.retain(|position| {
            area.contains(position.as_vec2())
        });

        for x in area.min.x.floor() as i32 .. area.max.x.ceil() as i32 {
            for y in area.min.y.floor() as i32 .. area.max.y.ceil() as i32 {
                let position = ivec2(x, y);
                
                if !self.active_chunks.contains(&position) {
                    self.activate_chunk(position);

                    if !self.chunks.contains_key(&position) {
                        self.add_chunk(commands, images, registries, noise, position, chunks);
                    }
                }
            }
        }
    }

    pub fn step(
        &mut self, 
        dirty_rects: &HashMap<IVec2, URect>,
        update_send: &Sender<UpdateMessage>,
        render_send: &Sender<RenderMessage>
    ) {
        self.clock = self.clock.wrapping_add(1);    

        // self.physics_step(camera_position);

        // let (updated_chunk_count, updated_pixels_count) = self.ca_step();
        // chunks_count += updated_chunk_count;
        // pixels_count += updated_pixels_count;

        // if self.clock % 4 == 0 {
        //     self.particle_step();
        // }  

        self.ca_step(dirty_rects, update_send, render_send)
    }

    fn ca_step(
        &mut self, 
        dirty_rects: &HashMap<IVec2, URect>,
        update_send: &Sender<UpdateMessage>,
        render_send: &Sender<RenderMessage>
    ) {
        let positions: Vec<IVec2> = self.active_chunks.iter().map(|v| *v).collect();

        let mut groups: BTreeMap<i32, BTreeSet<i32>> = BTreeMap::new();

        for position in positions.iter() {
            groups.entry(position.x).or_default().insert(position.y);
        }

        for (x, group) in groups.iter() {   
            for y in group.iter() {
                let position = ivec2(*x, *y);

                let Some(dirty_rect) = dirty_rects.get(&position) else {
                    continue;
                };

                // self.chunks.iter_mut()
                //     .for_each(f)

                let mut chunk_group = ChunkGroup {
                    center: None,
                    sides: [None, None, None, None],
                    corners: [None, None, None, None],
                };

                for (dx, dy) in (-1..=1).cartesian_product(-1..=1) {
                    match (dx, dy) {
                        (0, 0) => {
                            let Some(chunk) = self.chunks.get_mut(&position) else {
                                continue;
                            };
    
                            let start_ptr = chunk.cells.as_mut_ptr();
                            
                            chunk_group.center = Some(start_ptr);
                        }
                        // UP and DOWN
                        (0, -1) | (0, 1) => {
                            let Some(chunk) = self.chunks.get_mut(&(position + ivec2(dx, dy))) else {
                                continue;
                            };
    
                            let start_ptr = chunk.cells.as_mut_ptr();
    
                            //change
                            chunk_group.sides[if dy == -1 { 0 } else { 3 }] =
                                Some(start_ptr);
                        }
                        //LEFT and RIGHT
                        (-1, 0) | (1, 0) => {
                            let Some(chunk) = self.chunks.get_mut(&(position + ivec2(dx, dy))) else {
                                continue;
                            };
    
                            let start_ptr = chunk.cells.as_mut_ptr();

                            chunk_group.sides[if dx == -1 { 1 } else { 2 }] =
                                Some(start_ptr);
                        }
                        //CORNERS
                        (-1, -1) | (1, -1) | (-1, 1) | (1, 1) => {
                            let Some(chunk) = self.chunks.get_mut(&(position + ivec2(dx, dy))) else {
                                continue;
                            };
    
                            let start_ptr = chunk.cells.as_mut_ptr();
    
                            let corner_idx = match (dx, dy) {
                                (1, 1) => 3,
                                (-1, 1) => 2,
                                (1, -1) => 1,
                                (-1, -1) => 0,
    
                                _ => unreachable!(),
                            };
    
                            chunk_group.corners[corner_idx] = Some(start_ptr);
                        }
    
                        _ => unreachable!(),
                    }
                }

                let mut api = ChunkApi { 
                    cell_position: ivec2(0, 0),
                    chunk_position: position,
                    chunk_group: &mut chunk_group,
                    update_send,
                    render_send,
                    clock: self.clock,
                };

                let x_range = if fastrand::bool() {
                    Either::Left(dirty_rect.min.x as i32..dirty_rect.max.x as i32)
                } else {
                    Either::Right((dirty_rect.min.x as i32..dirty_rect.max.x as i32).rev())
                };

                for x_cell in x_range {
                    for y_cell in dirty_rect.min.y as i32..dirty_rect.max.y as i32 {
                        api.switch_position(ivec2(x_cell, y_cell));

                        if api.get_counter(0, 0) == self.clock {
                            api.keep_alive(0, 0);
                            continue;
                        }

                        match api.get_matter(0, 0) {
                            PhysicsType::Powder => {
                                update_sand(&mut api);
                            },
                            PhysicsType::Gas => {
                                update_gas(&mut api);
                            }
                            PhysicsType::Liquid(..) => {
                                update_liquid(&mut api);
                            }
                            _ => {}
                        }

                        api.mark_updated();
                    }
                }
            }
        }
    }
}

pub struct ChunkManagerPlugin;

impl Plugin for ChunkManagerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, manager_setup)
            .add_systems(Update, chunks_update.run_if(on_timer(Duration::from_millis(10))))
            // .add_systems(PostUpdate, update_textures)
            // .add_systems(Update, update_manager_pos)
            .add_systems(PostUpdate, chunks_gizmos)
            .add_systems(PostUpdate, dirty_rects_gizmos)
            // .add_systems(PreUpdate, clear_render_rect)
            .insert_resource(Msaa::Off)
            .insert_resource(ClearColor(Color::rgb(0.60, 0.88, 1.0)))
            .init_resource::<ChunkManager>()
            .init_resource::<DirtyRects>()
            .init_resource::<Noise>()
            .init_resource::<Registries>();
    }
}

pub fn chunks_gizmos(
    mut gizmos: Gizmos,
    chunk_manager: Res<ChunkManager>,
) {
    chunk_manager.chunks.iter()
        .for_each(|entry| {
            gizmos.rect_2d(entry.0.as_vec2() + Vec2::ONE * 0.5, 0.0, Vec2::ONE, Color::BLUE);
        });
}

pub fn manager_setup(
    mut commands: Commands,
    // mut images: ResMut<Assets<Image>>,
    // mut chunk_manager: ResMut<ChunkManager>,
    // noise: Res<Noise>,
) {
    commands.spawn((
        Name::new("Chunks"),
        GlobalTransform::IDENTITY,
        InheritedVisibility::VISIBLE,
        Chunks
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn chunks_update(
    registries: Res<Registries>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    noise: Res<Noise>,
    camera_query: Query<&Transform, With<Camera>>,
    chunks_query: Query<Entity, With<Chunks>>,
) {
    let camera_transform = camera_query.get_single().unwrap();
    let chunks = chunks_query.get_single().unwrap();

    chunk_manager.update_loaded_chunks(&mut commands, &mut images, &registries, &noise, camera_transform.translation.xy(), &chunks);

    let DirtyRects {
        current: dirty_rects,
        new: new_dirty_rects,
        render: render_rects,
    } = &mut *dirty_rects_resource;

    let (update_send, update_recv) = async_channel::unbounded::<UpdateMessage>();
    let (render_send, render_recv) = async_channel::unbounded::<RenderMessage>();

    ComputeTaskPool::get().scope(|scope| {
        scope.spawn(async move {
            new_dirty_rects.clear();
            while let Ok(update) = update_recv.recv().await {
                if update.awake_surrouding {
                    update_dirty_rects_3x3(new_dirty_rects, update.chunk_position, update.cell_position);
                } else {
                    update_dirty_rects(new_dirty_rects, update.chunk_position, update.cell_position)
                }
            }
        });

        scope.spawn(async move {
            while let Ok(update) = render_recv.recv().await {
                update_dirty_rects(render_rects, update.chunk_position, update.cell_position);
            }
        });

        chunk_manager.step(dirty_rects, &update_send, &render_send);         
        update_send.close();
        render_send.close();
    });

    dirty_rects_resource.render.iter_mut()
        .for_each(|(position, rect)| {
            if let Some(chunk) = chunk_manager.get_chunk(position) {
                let image = images.get_mut(chunk.texture.clone()).unwrap();
                chunk.update_rect(image, *rect);
            }
        }); 

    let new_positions = dirty_rects_resource.new.keys().copied().collect::<Vec<IVec2>>();
    new_positions.iter()
        .for_each(|position| {
            if !dirty_rects_resource.current.contains_key(position) {
                update_dirty_rects(&mut dirty_rects_resource.new, *position, UVec2::ZERO);
                update_dirty_rects(&mut dirty_rects_resource.new, *position, UVec2::ONE * (CHUNK_SIZE - 1) as u32);
            }
        });

    dirty_rects_resource.render.clear();
    dirty_rects_resource.swap();
}