use bevy::{
    prelude::*,
    render::view::RenderLayers,
    sprite::Anchor,
    tasks::ComputeTaskPool,
    utils::HashMap,
};
use bevy_math::{ ivec2, vec2, IVec2, Rect, UVec2, Vec3Swizzles };
use bevy_rapier2d::dynamics::RigidBody;
use itertools::{ Either, Itertools };

use crate::{
    camera::{ TrackingCamera, BACKGROUND_LAYER, LIGHTING_LAYER, TERRAIN_LAYER },
    constants::{ CHUNK_SIZE, WORLD_HEIGHT, WORLD_WIDTH },
    generation::chunk::GenerationEvent,
    registries::Registries,
};

use super::{
    chunk::{ Chunk, ChunkApi, ChunkData, ChunkState },
    chunk_groups::build_chunk_group,
    dirty_rect::{
        update_dirty_rects,
        update_dirty_rects_3x3,
        DirtyRects,
        RenderMessage,
        UpdateMessage,
    },
    materials::{ update_gas, update_liquid, update_sand, MaterialInstance, PhysicsType },
    pixel::Pixel,
};

#[derive(Component)]
pub struct Terrain;

#[derive(Resource)]
pub struct ChunkManager {
    pub chunks: HashMap<IVec2, (Entity, ChunkData)>,
    clock: u8,
}

impl FromWorld for ChunkManager {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self {
            chunks: HashMap::new(),
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
    pub fn clock(&self) -> u8 {
        self.clock
    }

    pub fn get(&self, pos: IVec2) -> Result<&Pixel, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.get_chunk_data(&chunk_position)
            .filter(
                |chunk| (chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping)
            )
            .map(|chunk| &chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_mut(&mut self, pos: IVec2) -> Result<&mut Pixel, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.get_chunk_data_mut(&chunk_position)
            .filter(
                |chunk| (chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping)
            )
            .map(|chunk| &mut chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_material(&self, pos: IVec2) -> Result<&MaterialInstance, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.get_chunk_data(&chunk_position)
            .filter(
                |chunk| (chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping)
            )
            .map(|chunk| &chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .map(|pixel| &pixel.material)
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn get_material_mut(&mut self, pos: IVec2) -> Result<&mut MaterialInstance, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        self.get_chunk_data_mut(&chunk_position)
            .filter(
                |chunk| (chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping)
            )
            .map(|chunk| &mut chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)])
            .map(|pixel| &mut pixel.material)
            .ok_or("pixel not loaded yet".to_string())
    }

    pub fn set(&mut self, pos: IVec2, material: MaterialInstance) -> Result<(), String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if
            let Some(chunk) = self
                .get_chunk_data_mut(&chunk_position)
                .filter(
                    |chunk|
                        chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping
                )
        {
            chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)] = Pixel::new(material);
            Ok(())
        } else {
            Err("pixel not loaded yet".to_string())
        }
    }

    pub fn set_with_condition<F: Fn(Pixel) -> bool>(
        &mut self,
        pos: IVec2,
        material: MaterialInstance,
        condition: F
    ) -> Result<bool, String> {
        let chunk_position = pos.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if
            let Some(chunk) = self
                .get_chunk_data_mut(&chunk_position)
                .filter(
                    |chunk|
                        chunk.state == ChunkState::Active || chunk.state == ChunkState::Sleeping
                )
        {
            if condition(chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)].clone()) {
                chunk[pos.rem_euclid(IVec2::ONE * CHUNK_SIZE)] = Pixel::new(material);
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err("pixel not loaded yet".to_string())
        }
    }

    pub fn displace(&mut self, pos: IVec2, material: MaterialInstance) -> bool {
        let mut succeeded = false;

        let scan_w = 32;
        let scan_h = 32;
        let mut scan_pos = IVec2::ZERO;
        let mut scan_delta_pos = IVec2::new(0, -1);
        let scan_max_i = scan_w.max(scan_h) * scan_w.max(scan_h); // the max is pointless now but could change w or h later

        for _ in 0..scan_max_i {
            if
                scan_pos.x >= -scan_w / 2 &&
                scan_pos.x <= scan_w / 2 &&
                scan_pos.y >= -scan_h / 2 &&
                scan_pos.y <= scan_h / 2
            {
                if
                    let Ok(true) = self.set_with_condition(
                        pos + IVec2::new(scan_pos.x, scan_pos.y),
                        material.clone(),
                        |pixel| pixel.material.physics_type == PhysicsType::Air
                    )
                {
                    succeeded = true;
                    break;
                }
            }

            // update scan coordinates

            if
                scan_pos.x == scan_pos.y ||
                (scan_pos.x < 0 && scan_pos.x == -scan_pos.y) ||
                (scan_pos.x > 0 && scan_pos.x == 1 - scan_pos.y)
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

    pub fn get_chunk_id(&self, chunk_position: &IVec2) -> Option<Entity> {
        self.chunks.get(chunk_position).map(|chunk| chunk.0)
    }

    pub fn get_chunk_data(&self, chunk_position: &IVec2) -> Option<&ChunkData> {
        self.chunks.get(chunk_position).map(|chunk| &chunk.1)
    }

    pub fn get_chunk_data_mut(&mut self, chunk_position: &IVec2) -> Option<&mut ChunkData> {
        self.chunks.get_mut(chunk_position).map(|chunk| &mut chunk.1)
    }
}

pub fn manager_setup(mut commands: Commands) {
    commands.spawn((Name::new("Terrain"), SpatialBundle::INHERITED_IDENTITY, Terrain));
}

pub fn update_loaded_chunks(
    mut commands: Commands,
    mut ev_chunkgen: EventWriter<GenerationEvent>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut images: ResMut<Assets<Image>>,
    camera_q: Query<(&Transform, &OrthographicProjection), With<TrackingCamera>>,
    terrain_q: Query<Entity, With<Terrain>>
) {
    let DirtyRects { current, .. } = &mut *dirty_rects_resource;
    let chunks_entity = terrain_q.single();

    let (transform, projection) = camera_q.single();

    let area = Rect::from_center_size(transform.translation.xy(), projection.area.size() + 4.0);

    // suspend chunks out of bounds
    chunk_manager.chunks
        .iter_mut()
        .map(|(position, chunk)| (position, &mut chunk.1))
        .filter(|(_, chunk)| chunk.state == ChunkState::Active)
        .for_each(|(position, chunk)| {
            if !area.contains(position.as_vec2()) {
                chunk.state = ChunkState::Sleeping;
            }
        });

    for x in area.min.x.ceil() as i32..area.max.x.floor() as i32 {
        for y in area.min.y.ceil() as i32..area.max.y.floor() as i32 {
            let position = ivec2(x, y);

            match chunk_manager.get_chunk_data_mut(&position) {
                Some(chunk) => {
                    if chunk.state == ChunkState::Sleeping {
                        update_dirty_rects(current, position, UVec2::ZERO);
                        update_dirty_rects(
                            current,
                            position,
                            UVec2::splat((CHUNK_SIZE as u32) - 1)
                        );
                        chunk.state = ChunkState::Active;
                    }
                }
                None => {
                    let chunk = ChunkData {
                        pixels: vec![],
                        texture: images.add(ChunkData::new_image()),
                        background: images.add(ChunkData::new_image()),
                        lighting: images.add(ChunkData::new_image()),
                        state: ChunkState::Generating,
                        ..Default::default()
                    };

                    let entity = commands
                        .spawn((
                            Chunk,
                            RigidBody::Fixed,
                            SpriteBundle {
                                texture: chunk.texture.clone(),
                                sprite: Sprite {
                                    custom_size: Some(vec2(1.0, 1.0)),
                                    anchor: Anchor::BottomLeft,
                                    flip_y: true,
                                    ..Default::default()
                                },
                                transform: Transform::from_translation(
                                    position.as_vec2().extend(0.0)
                                ),
                                ..Default::default()
                            },
                            RenderLayers::layer(TERRAIN_LAYER),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                SpriteBundle {
                                    texture: chunk.background.clone(),
                                    sprite: Sprite {
                                        custom_size: Some(vec2(1.0, 1.0)),
                                        anchor: Anchor::BottomLeft,
                                        flip_y: true,
                                        ..Default::default()
                                    },
                                    transform: Transform::from_translation(Vec2::ZERO.extend(-1.0)),
                                    ..Default::default()
                                },
                                RenderLayers::layer(BACKGROUND_LAYER),
                            ));
                        })
                        .id();

                    commands.entity(chunks_entity).add_child(entity);

                    chunk_manager.chunks.insert(position, (entity, chunk));
                    ev_chunkgen.send(GenerationEvent(position));
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn chunks_update(
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    registries: Res<Registries>
) {
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
                        update.cell_position
                    );
                } else {
                    update_dirty_rects(
                        new_dirty_rects,
                        update.chunk_position,
                        update.cell_position
                    );
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
        let clock = chunk_manager.clock;

        let active_chunks = chunk_manager.chunks
            .iter()
            .map(|(position, chunk)| (position, &chunk.1))
            .filter(|(_, chunk)| chunk.state == ChunkState::Active)
            .map(|(position, _)| *position)
            .collect_vec();

        // it possible to iterate in checker patttern which potentially improves performance but produces worse looking simulation on chunk borders

        // to iterate from bottom to top
        for (_, group) in active_chunks
            .into_iter()
            .group_by(|position| position.y)
            .into_iter()
            .sorted_by(|(y1, _), (y2, _)| y1.cmp(y2)) {
            // to avoid data races
            for (_, group) in &group.group_by(|position| position.x % 2 == 0) {
                ComputeTaskPool::get().scope(|scope| {
                    group
                        .filter_map(|position| {
                            dirty_rects
                                .get(&position)
                                .cloned()
                                .map(|dirty_rect| (position, dirty_rect))
                        })
                        .filter_map(|(position, dirty_rect)| {
                            build_chunk_group(&mut chunk_manager, position).map(
                                |chunk_group| (position, dirty_rect, chunk_group)
                            )
                        })
                        .for_each(|(position, dirty_rect, mut chunk_group)| {
                            let reactive_materials = &registries.reactive_materials;
                            let materials = &registries.materials;

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
                                        (dirty_rect.min.x as i32..dirty_rect.max.x as i32).rev()
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
                                            PhysicsType::Disturbed( .., original_type) => {
                                                let original_position = api.cell_position;
                                                update_sand(&mut api);

                                                let original = api.get(0, 0);
                                                
                                                if original_position == api.cell_position  {
                                                    let original_material = MaterialInstance {
                                                        physics_type: *original_type,
                                                        ..original.material.clone()
                                                    };
                                                    api.update(original.with_material(original_material))
                                                }
                                            }
                                            _ => {}
                                        }

                                        let directions = [
                                            [1, 0],
                                            [-1, 0],
                                            [0, 1],
                                            [0, -1],
                                        ];

                                        {
                                            let mut pixel = api.get(0, 0);
                                            match pixel.on_fire {
                                                true => {
                                                    api.keep_alive(0, 0);
                                                    if
                                                        let Some(fire_parameters) =
                                                            pixel.material.fire_parameters.as_mut()
                                                    {
                                                        for direction in directions.iter() {
                                                            let mut pixel = api.get(
                                                                direction[0],
                                                                direction[1]
                                                            );

                                                            if
                                                                pixel.temperature <
                                                                fire_parameters.fire_temperature
                                                            {
                                                                pixel.temperature +=
                                                                    (fastrand::f32() *
                                                                        ((
                                                                            pixel.temperature.abs_diff(
                                                                                fire_parameters.fire_temperature
                                                                            ) as f32
                                                                        ) /
                                                                            8.0)) as i32;
                                                            }

                                                            api.set(
                                                                direction[0],
                                                                direction[1],
                                                                pixel
                                                            );
                                                        }

                                                        if fire_parameters.fire_hp <= 0 {
                                                            api.update(Pixel::default());
                                                            continue;
                                                        } else if fastrand::f32() > 0.75 {
                                                            fire_parameters.fire_hp -= 1;
                                                        }
                                                    }
                                                }
                                                false => {
                                                    if
                                                        let Some(fire_parameters) =
                                                            &mut pixel.material.fire_parameters
                                                    {
                                                        if
                                                            pixel.temperature >=
                                                            fire_parameters.ignition_temperature
                                                        {
                                                            pixel.on_fire = true;
                                                        } else {
                                                            pixel.temperature -=
                                                                (30 - pixel.temperature) / 16;
                                                        }
                                                    }
                                                }
                                            }
                                            api.update(pixel);
                                        }

                                        let id = &api.get(0, 0).material.id;
                                        if reactive_materials.contains(id) {
                                            let material = materials.get(id).unwrap();
                                            if let Some(reactions) = &material.reactions {
                                                for (x, y) in (-1..=1).cartesian_product(-1..=1) {
                                                    if x == 0 && y == 0 {
                                                        continue;
                                                    }

                                                    let neighbour = api.get(x, y);

                                                    if
                                                        let Some(reaction) = reactions.get(
                                                            &neighbour.material.id
                                                        )
                                                    {
                                                        if fastrand::f32() < reaction.probability {
                                                            api.set(
                                                                0,
                                                                0,
                                                                Pixel::new(
                                                                    materials
                                                                        .get(
                                                                            &reaction.output_material_1
                                                                        )
                                                                        .unwrap()
                                                                        .into()
                                                                ).with_clock(clock)
                                                            );
                                                            api.set(
                                                                x,
                                                                y,
                                                                Pixel::new(
                                                                    materials
                                                                        .get(
                                                                            &reaction.output_material_2
                                                                        )
                                                                        .unwrap()
                                                                        .into()
                                                                ).with_clock(clock)
                                                            );

                                                            break;
                                                        }

                                                        api.keep_alive(x, y);
                                                    }
                                                }
                                            }
                                        }

                                        api.mark_updated();
                                    }
                                }
                            })
                        })
                });
            }
        }

        update_send.close();
        render_send.close();
    });

    let new_positions = dirty_rects_resource.new.keys().copied().collect::<Vec<IVec2>>();

    new_positions.iter().for_each(|position| {
        if !dirty_rects_resource.current.contains_key(position) {
            update_dirty_rects(&mut dirty_rects_resource.new, *position, UVec2::ZERO);
            update_dirty_rects(
                &mut dirty_rects_resource.new,
                *position,
                UVec2::ONE * ((CHUNK_SIZE - 1) as u32)
            );
        }
    });

    dirty_rects_resource.current.clear();
    dirty_rects_resource.swap();
}
