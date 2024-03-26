use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bevy::{
    prelude::*, sprite::Anchor, tasks::ComputeTaskPool, time::common_conditions::on_timer,
    utils::HashMap,
};
use bevy_math::{ivec2, vec2, IVec2, Rect, UVec2, Vec2, Vec3Swizzles};
use bevy_rapier2d::prelude::*;
use dashmap::DashSet;
use itertools::{Either, Itertools};
use noise::{NoiseFn, SuperSimplex};

use crate::{
    constants::{CHUNK_SIZE, WORLD_HEIGHT, WORLD_WIDTH},
    registries::Registries,
};

use super::{
    chunk::{ChunkApi, ChunkData},
    chunk_groups::ChunkGroup3x3,
    dirty_rect::{
        dirty_rects_gizmos, update_dirty_rects, update_dirty_rects_3x3, DirtyRects, RenderMessage,
        UpdateMessage,
    },
    materials::{update_gas, update_liquid, update_sand, MaterialInstance, PhysicsType},
    object::ObjectPlugin,
    particle::ParticlePlugin,
    pixel::Pixel,
};

#[derive(Resource)]
pub struct Noise(SuperSimplex);

#[derive(Component)]
pub struct Chunks;

impl FromWorld for Noise {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(SuperSimplex::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .subsec_millis(),
        ))
    }
}

#[derive(Resource)]
pub struct ChunkManager {
    pub chunks: HashMap<IVec2, ChunkData>,
    active_chunks: DashSet<IVec2, ahash::RandomState>,
    clock: u8,
}

impl FromWorld for ChunkManager {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
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
                    y as f64 / CHUNK_SIZE as f64 + chunk_position.y as f64,
                ]);

                let cell_position = ivec2(x, y);

                value *= 10.0;

                match value as i64 {
                    0..=1 => {
                        chunk[cell_position] =
                            Pixel::new(MaterialInstance::from(surface_element), 1)
                    }
                    2..=4 => {
                        chunk[cell_position] =
                            Pixel::new(MaterialInstance::from(underground_element), 1)
                    }
                    5..=10 => {
                        chunk[cell_position] = Pixel::new(MaterialInstance::from(depth_element), 1)
                    }
                    _ => {}
                }
            }
        }

        let image_handle = images.add(ChunkData::new_image());
        let mut entity_command = commands.spawn((
            RigidBody::Fixed,
            SpriteBundle {
                texture: image_handle.clone(),
                sprite: Sprite {
                    custom_size: Some(vec2(1.0, 1.0)),
                    anchor: Anchor::BottomLeft,
                    flip_y: true,
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(
                    chunk_position.x as f32,
                    chunk_position.y as f32,
                    0.,
                )),
                ..Default::default()
            },
        ));

        if let Ok(colliders) = chunk.build_colliders() {
            entity_command.with_children(|children| {
                for collider in colliders {
                    children.spawn((
                        collider,
                        TransformBundle {
                            local: Transform::IDENTITY,
                            ..Default::default()
                        },
                    ));
                }
            });
        }

        let id = entity_command.id();

        commands.entity(*chunks).push_children(&[id]);

        chunk.texture = image_handle.clone();
        chunk.entity = Some(id);

        chunk.update_all(images.get_mut(&image_handle.clone()).unwrap());

        self.chunks.insert(chunk_position, chunk);
    }

    pub fn clock(&self) -> u8 {
        self.clock
    }

    pub fn get(&self, pos: IVec2) -> Result<&Pixel, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.chunks
            .get(&chunk_position)
            .map(|chunk| &chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_mut(&mut self, pos: IVec2) -> Result<&mut Pixel, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.chunks
            .get_mut(&chunk_position)
            .map(|chunk| &mut chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_material(&self, pos: IVec2) -> Result<&MaterialInstance, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.chunks
            .get(&chunk_position)
            .map(|chunk| &chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .map(|pixel| &pixel.material)
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_material_mut(&mut self, pos: IVec2) -> Result<&mut MaterialInstance, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.chunks
            .get_mut(&chunk_position)
            .map(|chunk| &mut chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .map(|pixel| &mut pixel.material)
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn check_collision(&self, pos: IVec2) -> Option<&PhysicsType> {
        self.get(pos)
            .ok()
            .filter(|pixel| {
                matches!(
                    pixel.material.physics_type,
                    PhysicsType::Static | PhysicsType::Powder
                )
            })
            .map_or(Some(&PhysicsType::Static), |pixel| {
                Some(&pixel.material.physics_type)
            })
    }

    pub fn set(&mut self, pos: IVec2, material: MaterialInstance) -> Result<(), String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if let Some(chunk) = self.chunks.get_mut(&chunk_position) {
            chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)] = Pixel::new(material, 0);
            Ok(())
        } else {
            Err("chunk is not loaded".to_string())
        }
    }

    pub fn set_with_condition<F: Fn(Pixel) -> bool>(
        &mut self,
        pos: IVec2,
        material: MaterialInstance,
        condition: F,
    ) -> Result<bool, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        match self.chunks.get_mut(&chunk_position) {
            Some(chunk) => {
                if condition(chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)].clone()) {
                    chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)] = Pixel::new(material, 0);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Err("chunk is not loaded".to_string()),
        }
    }

    // TODO: rewrite
    pub fn displace(&mut self, pos: IVec2, material: MaterialInstance) -> bool {
        let mut succeeded = false;

        let scan_w = 32;
        let scan_h = 32;
        let mut scan_pos = IVec2::ZERO;
        let mut scan_delta_pos = IVec2::new(0, -1);
        let scan_max_i = scan_w.max(scan_h) * scan_w.max(scan_h); // the max is pointless now but could change w or h later

        for _ in 0..scan_max_i {
            if (scan_pos.x >= -scan_w / 2)
                && (scan_pos.x <= scan_w / 2)
                && (scan_pos.y >= -scan_h / 2)
                && (scan_pos.y <= scan_h / 2)
            {
                if let Ok(true) = self.set_with_condition(
                    pos + IVec2::new(scan_pos.x, scan_pos.y),
                    material.clone(),
                    |pixel| (pixel.material.physics_type == PhysicsType::Air),
                ) {
                    succeeded = true;
                    break;
                }
            }

            // update scan coordinates

            if (scan_pos.x == scan_pos.y)
                || ((scan_pos.x < 0) && (scan_pos.x == -scan_pos.y))
                || ((scan_pos.x > 0) && (scan_pos.x == 1 - scan_pos.y))
            {
                let temp = scan_delta_pos.x;
                scan_delta_pos.x = -scan_delta_pos.y;
                scan_delta_pos.y = temp;
            }

            scan_pos.x += scan_delta_pos.x;
            scan_pos.y += scan_delta_pos.y;
        }

        succeeded
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
            vec2(WORLD_WIDTH as f32, WORLD_HEIGHT as f32),
        );

        self.active_chunks
            .retain(|position| area.contains(position.as_vec2()));

        for x in area.min.x.floor() as i32..area.max.x.ceil() as i32 {
            for y in area.min.y.floor() as i32..area.max.y.ceil() as i32 {
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
}

pub struct ChunkManagerPlugin;

impl Plugin for ChunkManagerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ParticlePlugin)
            .add_plugins(ObjectPlugin)
            .add_systems(Startup, manager_setup)
            .add_systems(
                Update,
                chunks_update.run_if(on_timer(Duration::from_millis(10))),
            )
            .add_systems(PostUpdate, (dirty_rects_gizmos, render_dirty_rect_updates))
            .insert_resource(Msaa::Off)
            .insert_resource(ClearColor(Color::Rgba {
                red: 0.60,
                green: 0.88,
                blue: 1.0,
                alpha: 1.0,
            }))
            .init_resource::<ChunkManager>()
            .init_resource::<DirtyRects>()
            .init_resource::<Noise>()
            .init_resource::<Registries>();
    }
}

pub fn chunks_gizmos(mut gizmos: Gizmos, chunk_manager: Res<ChunkManager>) {
    chunk_manager.chunks.iter().for_each(|entry| {
        gizmos.rect_2d(
            entry.0.as_vec2() + Vec2::ONE * 0.5,
            0.0,
            Vec2::ONE,
            Color::BLUE,
        );
    });
}

pub fn manager_setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Chunks"),
        SpatialBundle::INHERITED_IDENTITY,
        Chunks,
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
    let camera_transform = camera_query.single();
    let chunks = chunks_query.single();

    chunk_manager.update_loaded_chunks(
        &mut commands,
        &mut images,
        &registries,
        &noise,
        camera_transform.translation.xy(),
        &chunks,
    );

    let DirtyRects {
        current: dirty_rects,
        new: new_dirty_rects,
        render: render_rects,
    } = &mut *dirty_rects_resource;

    let (update_send, update_recv) = async_channel::unbounded::<UpdateMessage>();
    let (render_send, render_recv) = async_channel::unbounded::<RenderMessage>();

    chunk_manager.clock = chunk_manager.clock.wrapping_add(1);

    ComputeTaskPool::get().scope(|scope| {
        scope.spawn(async move {
            while let Ok(update) = update_recv.recv().await {
                if update.awake_surrouding {
                    update_dirty_rects_3x3(
                        new_dirty_rects,
                        update.chunk_position,
                        update.cell_position,
                    );
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

        let update_send = &update_send;
        let render_send = &render_send;

        let mut groups: [Vec<IVec2>; 4] = [vec![], vec![], vec![], vec![]];

        chunk_manager
            .active_chunks
            .iter()
            .map(|v| *v)
            .for_each(|position| {
                let index = (position.x.abs() % 2 + (position.y.abs() % 2) * 2) as usize;

                unsafe { groups.get_unchecked_mut(index) }.push(position);
            });

        fastrand::shuffle(&mut groups);

        for group in groups {
            ComputeTaskPool::get().scope(|scope| {
                let clock = chunk_manager.clock;

                group
                    .into_iter()
                    .filter_map(|position| {
                        dirty_rects
                            .get(&position)
                            .cloned()
                            .map(|dirty_rect| (position, dirty_rect))
                    })
                    .map(|(position, dirty_rect)| {
                        let mut chunk_group = ChunkGroup3x3 {
                            center: None,
                            sides: [None, None, None, None],
                            corners: [None, None, None, None],
                        };

                        for (dx, dy) in (-1..=1).cartesian_product(-1..=1) {
                            match (dx, dy) {
                                (0, 0) => {
                                    let Some(chunk) = chunk_manager.chunks.get_mut(&position)
                                    else {
                                        continue;
                                    };

                                    let start_ptr = chunk.cells.as_mut_ptr();

                                    chunk_group.center = Some(start_ptr);
                                }
                                // UP and DOWN
                                (0, -1) | (0, 1) => {
                                    let Some(chunk) =
                                        chunk_manager.chunks.get_mut(&(position + ivec2(dx, dy)))
                                    else {
                                        continue;
                                    };

                                    let start_ptr = chunk.cells.as_mut_ptr();

                                    chunk_group.sides[if dy == -1 { 0 } else { 3 }] =
                                        Some(start_ptr);
                                }
                                //LEFT and RIGHT
                                (-1, 0) | (1, 0) => {
                                    let Some(chunk) =
                                        chunk_manager.chunks.get_mut(&(position + ivec2(dx, dy)))
                                    else {
                                        continue;
                                    };

                                    let start_ptr = chunk.cells.as_mut_ptr();

                                    chunk_group.sides[if dx == -1 { 1 } else { 2 }] =
                                        Some(start_ptr);
                                }
                                //CORNERS
                                (-1, -1) | (1, -1) | (-1, 1) | (1, 1) => {
                                    let Some(chunk) =
                                        chunk_manager.chunks.get_mut(&(position + ivec2(dx, dy)))
                                    else {
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
                        (position, dirty_rect, chunk_group)
                    })
                    .for_each(|(position, dirty_rect, mut chunk_group)| {
                        scope.spawn(async move {
                            let mut api = ChunkApi {
                                cell_position: ivec2(0, 0),
                                chunk_position: position,
                                chunk_group: &mut chunk_group,
                                update_send,
                                render_send,
                                clock,
                            };

                            let x_range = if fastrand::bool() {
                                Either::Left(dirty_rect.min.x as i32..dirty_rect.max.x as i32)
                            } else {
                                Either::Right(
                                    (dirty_rect.min.x as i32..dirty_rect.max.x as i32).rev(),
                                )
                            };

                            for x_cell in x_range {
                                for y_cell in dirty_rect.min.y as i32..dirty_rect.max.y as i32 {
                                    api.switch_position(ivec2(x_cell, y_cell));

                                    if api.get_counter(0, 0) == clock {
                                        api.keep_alive(0, 0);
                                        continue;
                                    }

                                    match api.get_physics_type(0, 0) {
                                        PhysicsType::Powder => {
                                            update_sand(&mut api);
                                        }
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
                        })
                    })
            });
        }

        update_send.close();
        render_send.close();
    });

    let new_positions = dirty_rects_resource
        .new
        .keys()
        .copied()
        .collect::<Vec<IVec2>>();
    new_positions.iter().for_each(|position| {
        if !dirty_rects_resource.current.contains_key(position) {
            update_dirty_rects(&mut dirty_rects_resource.new, *position, UVec2::ZERO);
            update_dirty_rects(
                &mut dirty_rects_resource.new,
                *position,
                UVec2::ONE * (CHUNK_SIZE - 1) as u32,
            );
        }
    });

    dirty_rects_resource.current.clear();
    dirty_rects_resource.swap();
}

pub fn render_dirty_rect_updates(
    mut dirty_rects_resource: ResMut<DirtyRects>,
    chunk_manager: Res<ChunkManager>,
    mut images: ResMut<Assets<Image>>,
) {
    dirty_rects_resource
        .render
        .iter_mut()
        .for_each(|(position, rect)| {
            if let Some(chunk) = chunk_manager.get_chunk(position) {
                let image = images.get_mut(chunk.texture.clone()).unwrap();
                chunk.update_rect(image, *rect);
            }
        });

    dirty_rects_resource.render.clear();
}
