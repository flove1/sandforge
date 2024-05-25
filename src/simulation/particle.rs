use std::mem;

use async_channel::Sender;
use bevy::{
    prelude::*,
    render::{ mesh::PrimitiveTopology, render_asset::RenderAssetUsages, view::{NoFrustumCulling, RenderLayers} },
    sprite::Mesh2dHandle,
    tasks::ComputeTaskPool,
    transform,
    utils::HashMap,
};
use bevy_egui::egui::vec2;
use bevy_math::ivec2;
use bevy_rapier2d::dynamics::Velocity;
use bytemuck::{ Pod, Zeroable };
use itertools::Itertools;
use serde::{ Deserialize, Serialize };

use crate::{ camera::PARTICLE_RENDER_LAYER, constants::{ CHUNK_SIZE, PARTICLE_Z }, helpers::WalkGrid };

use super::{
    chunk::ChunkState,
    chunk_groups::{ build_chunk_group, ChunkGroup },
    chunk_manager::ChunkManager,
    dirty_rect::{
        update_dirty_rects,
        update_dirty_rects_3x3,
        DirtyRects,
        RenderMessage,
        UpdateMessage,
    },
    materials::{ Material, PhysicsType },
    pixel::Pixel,
};

#[derive(Bundle)]
pub struct ParticleBundle {
    pub sprite: SpriteBundle,
    pub velocity: Velocity,
    pub movement: ParticleMovement,
    pub state: ParticleObjectState,
    pub particle: Particle,
    pub render_layers: RenderLayers
}

impl Default for ParticleBundle {
    fn default() -> Self {
        Self {
            sprite: SpriteBundle::default(),
            velocity: Velocity::default(),
            movement: ParticleMovement::Fall,
            state: ParticleObjectState::FirstFrame,
            particle: Particle::default(),
            render_layers: RenderLayers::layer(PARTICLE_RENDER_LAYER)
        }
    }
}

#[derive(Component, Default)]
pub struct ParticleParent;

#[derive(Component, Reflect, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum ParticleMovement {
    Fall,
    Follow(Entity),
}

#[derive(Component, Reflect, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum ParticleObjectState {
    FirstFrame,
    Inside,
    Outside,
}

#[derive(Default, Component, Debug, Clone)]
pub struct Particle {
    pub active: bool,
    pub pixel: Pixel,
}

impl Particle {
    pub fn new(pixel: Pixel) -> Self {
        Self {
            active: true,
            pixel,
        }
    }
}

pub struct ParticleApi<'a> {
    pub(super) chunk_position: IVec2,
    pub(super) chunk_group: &'a mut ChunkGroup<Pixel>,
    pub(super) update_send: &'a Sender<UpdateMessage>,
    pub(super) render_send: &'a Sender<RenderMessage>,
}

impl<'a> ParticleApi<'a> {
    pub fn get(&self, x: i32, y: i32) -> Option<Pixel> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        self.chunk_group.get(cell_position).cloned()
    }

    pub fn set(&mut self, x: i32, y: i32, pixel: Pixel) -> Result<(), String> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        match self.chunk_group.get_mut(cell_position) {
            Some(old_pixel) => {
                *old_pixel = pixel;

                self.update_send
                    .try_send(UpdateMessage {
                        cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                        chunk_position: self.chunk_position +
                        cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        awake_surrouding: true,
                    })
                    .unwrap();

                self.render_send
                    .try_send(RenderMessage {
                        cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                        chunk_position: self.chunk_position +
                        cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                    })
                    .unwrap();

                Ok(())
            }
            None => Err("out of bounds".to_string()),
        }
    }

    pub fn set_with_condition<F: Fn(Pixel) -> bool>(
        &mut self,
        x: i32,
        y: i32,
        pixel: Pixel,
        condition: F
    ) -> Result<(), String> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        match self.chunk_group.get_mut(cell_position) {
            Some(initial_pixel) => {
                if condition(initial_pixel.clone()) {
                    *initial_pixel = pixel;

                    self.update_send
                        .try_send(UpdateMessage {
                            cell_position: cell_position
                                .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                .as_uvec2(),
                            chunk_position: self.chunk_position +
                            cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                            awake_surrouding: true,
                        })
                        .unwrap();

                    self.render_send
                        .try_send(RenderMessage {
                            cell_position: cell_position
                                .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                .as_uvec2(),
                            chunk_position: self.chunk_position +
                            cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        })
                        .unwrap();

                    Ok(())
                } else {
                    Err("condition doesn't match".to_string())
                }
            }
            None => Err("out of bounds".to_string()),
        }
    }

    // TODO: rewrite
    pub fn displace(&mut self, pos: IVec2, pixel: Pixel) -> bool {
        let scan_radius = 16;
        let mut scan_pos = IVec2::ZERO;
        let mut scan_delta_pos = IVec2::new(0, -1);

        for _ in 0..scan_radius {
            let check_scan = scan_pos.abs().cmple(IVec2::splat(scan_radius)).all();

            if
                check_scan &&
                self
                    .set_with_condition(
                        pos.x + scan_pos.x,
                        pos.y + scan_pos.y,
                        pixel.clone(),
                        |pixel| pixel.is_empty()
                    )
                    .is_ok()
            {
                return true;
            }

            // update scan coordinates
            if
                scan_pos.x == scan_pos.y ||
                (scan_pos.x < 0 && scan_pos.x == -scan_pos.y) ||
                (scan_pos.x > 0 && scan_pos.x == 1 - scan_pos.y)
            {
                mem::swap(&mut scan_delta_pos.x, &mut scan_delta_pos.y);
                scan_delta_pos.x *= -1;
            }

            scan_pos += scan_delta_pos;
        }

        false
    }
}
//
pub fn particle_setup(mut commands: Commands) {
    commands.spawn((
        Name::new("Particles"),
        SpatialBundle::INHERITED_IDENTITY,
        ParticleParent,
    ));
}

pub fn particle_set_parent(
    mut commands: Commands,
    particle_q: Query<Entity, Added<Particle>>,
    particle_parent_q: Query<Entity, With<ParticleParent>>
) {
    let particle_parent = particle_parent_q.single();

    for entity in particle_q.iter() {
        commands.entity(particle_parent).add_child(entity);
    }
}

pub fn particle_modify_velocity(
    mut particle_q: Query<
        (&Particle, &Transform, &mut Velocity, &mut ParticleMovement),
        With<Particle>
    >,
    transform_q: Query<&GlobalTransform, Without<Particle>>,
    time: Res<Time>
) {
    for (_, transform, mut velocity, mut movement) in particle_q
        .iter_mut()
        .filter(|(particle, ..)| particle.active) {
        match *movement {
            ParticleMovement::Fall => {
                velocity.linvel.y -= (0.2 / (CHUNK_SIZE as f32)) * time.delta_seconds() * 100.0;
            }
            ParticleMovement::Follow(target_entity) => {
                let Ok(target_transform) = transform_q.get(target_entity) else {
                    *movement = ParticleMovement::Fall;
                    continue;
                };

                let distance = target_transform.translation().xy() - transform.translation.xy();

                if distance.length() * CHUNK_SIZE as f32 > 24.0 {
                    *movement = ParticleMovement::Fall;
                    continue;
                }

                let angle = distance.normalize_or_zero().to_angle();
                let magnitude = distance.length_recip().sqrt();

                velocity.linvel = (
                    (Vec2::new(angle.cos(), angle.sin()) * magnitude +
                        (Vec2::new(fastrand::f32(), fastrand::f32()) / 2.0 - 0.5)) /
                    (CHUNK_SIZE as f32)
                ).clamp(-Vec2::ONE, Vec2::ONE);
            }
        }
    }
}

pub fn particles_update(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut particles: Query<
        (
            Entity,
            &mut Transform,
            &mut Velocity,
            &mut Particle,
            &mut ParticleObjectState,
            &mut ParticleMovement,
        )
    >,
    transform_q: Query<&GlobalTransform, Without<Particle>>,
    particles_instances: Query<Entity, With<ParticleParent>>
) {
    let particles_instances = particles_instances.single();

    let DirtyRects { new: new_dirty_rects, render: render_rects, .. } = &mut *dirty_rects_resource;

    let (update_send, update_recv) = async_channel::unbounded::<UpdateMessage>();
    let (render_send, render_recv) = async_channel::unbounded::<RenderMessage>();
    let (particle_send, particle_recv) = async_channel::unbounded::<Entity>();

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

        scope.spawn(async move {
            while let Ok(entity) = particle_recv.recv().await {
                commands.entity(particles_instances).remove_children(&[entity]);
                commands.entity(entity).despawn();
            }
        });

        let update_send = &update_send;
        let render_send = &render_send;
        let particle_send = &particle_send;
        let transform_q = &transform_q;

        let mut particles_maps = [
            HashMap::default(),
            HashMap::default(),
            HashMap::default(),
            HashMap::default(),
        ];

        for (entity, transform, velocity, particle, object_state, movement) in particles
            .iter_mut()
            .filter(|(_, transform, _, particle, ..)| particle.active) {
            let chunk_position = transform.translation.xy().as_ivec2();

            (
                unsafe {
                    particles_maps.get_unchecked_mut(
                        ((chunk_position.x.abs() % 2) + (chunk_position.y.abs() % 2) * 2) as usize
                    )
                }
            )
                .entry(chunk_position)
                .or_insert_with(Vec::new)
                .push((entity, transform, velocity, particle, object_state, movement));
        }

        particles_maps.into_iter().for_each(|map| {
            ComputeTaskPool::get().scope(|scope| {
                map.into_iter()
                    .filter_map(|(position, particles)|
                        build_chunk_group(&mut chunk_manager, position).map(|chunk_group| (
                            position,
                            particles,
                            chunk_group,
                        ))
                    )
                    .for_each(|(position, particles, mut chunk_group)| {
                        scope.spawn(async move {
                            let mut api = ParticleApi {
                                chunk_position: position,
                                chunk_group: &mut chunk_group,
                                update_send,
                                render_send,
                            };

                            particles
                                .into_iter()
                                .filter_map(
                                    |(
                                        entity,
                                        mut transform,
                                        mut velocity,
                                        particle,
                                        mut object_state,
                                        mut movement,
                                    )| {
                                        let initial = transform.translation.xy();
                                        let delta = velocity.linvel;

                                        for position in WalkGrid::new(
                                            (initial * (CHUNK_SIZE as f32)).as_ivec2(),
                                            ((initial + delta) * (CHUNK_SIZE as f32)).as_ivec2()
                                        ) {
                                            let Some(pixel) = api.get(
                                                position.x as i32,
                                                position.y as i32
                                            ) else {
                                                continue;
                                            };

                                            match *movement {
                                                ParticleMovement::Fall => {
                                                    if
                                                        pixel.is_empty()
                                                    {
                                                        *object_state =
                                                            ParticleObjectState::Outside;
                                                        continue;
                                                    }

                                                    let is_object = matches!(
                                                        pixel.physics_type,
                                                        PhysicsType::Rigidbody
                                                    );

                                                    match *object_state {
                                                        ParticleObjectState::FirstFrame => {
                                                            if is_object {
                                                                *object_state =
                                                                    ParticleObjectState::Inside;
                                                            } else {
                                                                *object_state =
                                                                    ParticleObjectState::Outside;
                                                            }
                                                        }
                                                        ParticleObjectState::Inside => {
                                                            if !is_object {
                                                                *object_state =
                                                                    ParticleObjectState::Outside;
                                                            }
                                                        }
                                                        ParticleObjectState::Outside => {}
                                                    }

                                                    if
                                                        !is_object ||
                                                        *object_state ==
                                                            ParticleObjectState::Outside
                                                    {
                                                        match pixel.physics_type {
                                                            PhysicsType::Air | PhysicsType::Gas => {
                                                                if
                                                                    api
                                                                        .set(
                                                                            position.x as i32,
                                                                            position.y as i32,
                                                                            particle.pixel.clone()
                                                                        )
                                                                        .is_ok()
                                                                {
                                                                    return Some(entity);
                                                                }
                                                            }
                                                            _ => {
                                                                let succeeded = api.displace(
                                                                    IVec2::new(
                                                                        position.x as i32,
                                                                        position.y as i32
                                                                    ),
                                                                    particle.pixel.clone()
                                                                );

                                                                if succeeded {
                                                                    return Some(entity);
                                                                }

                                                                // upwarp if completely blocked
                                                                velocity.linvel.y =
                                                                    1.0 / (CHUNK_SIZE as f32);
                                                                transform.translation.y +=
                                                                    4.0 / (CHUNK_SIZE as f32);

                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                                ParticleMovement::Follow(target_entity) => {
                                                    let Ok(target_transform) =
                                                        transform_q.get(target_entity) else {
                                                        *movement = ParticleMovement::Fall;
                                                        break;
                                                    };

                                                    if
                                                        (
                                                            position.as_vec2() / (CHUNK_SIZE as f32)
                                                        ).distance(
                                                            target_transform.translation().xy()
                                                        ) < 4.0 / (CHUNK_SIZE as f32)
                                                    {
                                                        return Some(entity);
                                                    }
                                                }
                                            }
                                        }

                                        transform.translation.x += delta.x;
                                        transform.translation.y += delta.y;

                                        None
                                    }
                                )
                                .for_each(|entity| {
                                    particle_send.try_send(entity).unwrap();
                                });
                        });
                    });
            });
        });

        update_send.close();
        render_send.close();
        particle_send.close();
    });
}

// #[allow(clippy::too_many_arguments)]
// fn queue_custom(
//     transparent_2d_draw_functions: Res<DrawFunctions<Transparent2d>>,
//     custom_pipeline: Res<CustomPipeline>,
//     msaa: Res<Msaa>,
//     mut pipelines: ResMut<SpecializedRenderPipelines<CustomPipeline>>,
//     pipeline_cache: Res<PipelineCache>,
//     meshes: Res<RenderAssets<Mesh>>,
//     render_mesh_instances: Res<RenderMesh2dInstances>,
//     material_meshes: Query<Entity, With<ParticleInstances>>,
//     mut views: Query<(&ExtractedView, &mut RenderPhase<Transparent2d>)>,
// ) {
//     let draw_custom = transparent_2d_draw_functions.read().id::<DrawCustom>();

//     let msaa_key = Mesh2dPipelineKey::from_msaa_samples(msaa.samples());

//     for (view, mut transparent_phase) in &mut views {
//         let view_key = msaa_key | Mesh2dPipelineKey::from_hdr(view.hdr);

//         for entity in &material_meshes {
//             let Some(mesh_instance) = render_mesh_instances.get(&entity) else {
//                 continue
//             };

//             let mesh2d_handle = mesh_instance.mesh_asset_id;

//             let mut mesh2d_key = view_key;
//             if let Some(mesh) = meshes.get(mesh2d_handle) {
//                 mesh2d_key |=
//                     Mesh2dPipelineKey::from_primitive_topology(mesh.primitive_topology);
//             }

//             let pipeline = pipelines.specialize(&pipeline_cache, &custom_pipeline, mesh2d_key);
//             transparent_phase.add(Transparent2d {
//                 entity,
//                 pipeline,
//                 draw_function: draw_custom,
//                 batch_range: 0..1,
//                 dynamic_offset: None,
//                 sort_key: FloatOrd(PARTICLE_LAYER),
//             });
//         }
//     }
// }

// #[derive(Component)]
// struct InstanceBuffer {
//     buffer: Buffer,
//     length: usize,
// }

// fn prepare_instance_buffers(
//     mut commands: Commands,
//     query: Query<&Particle>,
//     query_instances: Query<Entity, With<ParticleInstances>>,
//     render_device: Res<RenderDevice>,
// ) {
//     let array = query.iter()
//     .map(|particle| particle.into())
//     .collect::<Vec<ParticleRenderInstance>>();

//     let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
//         label: Some("instance data buffer"),
//         contents: bytemuck::cast_slice(&array),
//         usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
//     });

//     commands.entity(query_instances.single()).insert(InstanceBuffer {
//         buffer,
//         length: array.length(),
//     });
// }

// #[derive(Resource)]
// struct CustomPipeline {
//     shader: Handle<Shader>,
//     mesh_pipeline: Mesh2dPipeline,
// }

// impl FromWorld for CustomPipeline {
//     fn from_world(world: &mut World) -> Self {
//         let asset_server = world.resource::<AssetServer>();
//         let shader = asset_server.load("shaders/particle.wgsl");

//         let mesh_pipeline = world.resource::<Mesh2dPipeline>();

//         CustomPipeline {
//             shader,
//             mesh_pipeline: mesh_pipeline.clone(),
//         }
//     }
// }

// impl SpecializedRenderPipeline for CustomPipeline {
//     type Key = Mesh2dPipelineKey;

//     fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
//         let format = match key.contains(Mesh2dPipelineKey::HDR) {
//             true => ViewTarget::TEXTURE_FORMAT_HDR,
//             false => TextureFormat::bevy_default(),
//         };

//         RenderPipelineDescriptor {
//             vertex: VertexState {
//                 // Use our custom shader
//                 shader: self.shader.clone_weak(),
//                 entry_point: "vs_main".into(),
//                 shader_defs: vec![],
//                 // Use our custom vertex buffer
//                 buffers: vec![
//                     VertexBufferLayout::from_vertex_formats(
//                         VertexStepMode::Vertex,
//                         [
//                             VertexFormat::Float32x3,
//                             VertexFormat::Float32x3,
//                             VertexFormat::Float32x4,
//                         ],
//                     ),
//                 ],
//             },
//             fragment: Some(FragmentState {
//                 // Use our custom shader
//                 shader: self.shader.clone_weak(),
//                 shader_defs: vec![],
//                 entry_point: "fs_main".into(),
//                 targets: vec![Some(ColorTargetState {
//                     format,
//                     blend: Some(BlendState::ALPHA_BLENDING),
//                     write_mask: ColorWrites::ALL,
//                 })],
//             }),
//             // Use the two standard uniforms for 2d meshes
//             layout: vec![
//                 self.mesh_pipeline.view_layout.clone(),
//                 self.mesh_pipeline.mesh_layout.clone(),
//             ],
//             push_constant_ranges: Vec::new(),
//             primitive: PrimitiveState {
//                 topology: key.primitive_topology(),
//                 strip_index_format: None,
//                 front_face: FrontFace::Ccw,
//                 cull_mode: Some(Face::Back),
//                 polygon_mode: PolygonMode::Fill,
//                 unclipped_depth: false,
//                 conservative: false,
//             },
//             depth_stencil: None,
//             multisample: MultisampleState {
//                 count: key.msaa_samples(),
//                 mask: !0,
//                 alpha_to_coverage_enabled: false,
//             },
//             label: Some("particle_pipeline".into()),
//         }

//     }

//     // fn specialize(
//     //     &self,
//     //     key: Self::Key,
//     //     layout: &MeshVertexBufferLayout,
//     // ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
//         // let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

//     //     descriptor.vertex.shader = self.shader.clone();
//     //     descriptor.vertex.buffers.push(VertexBufferLayout {
//     //         array_stride: std::mem::size_of::<ParticleRenderInstance>() as u64,
//     //         step_mode: VertexStepMode::Instance,
//     //         attributes: vec![
//     //             VertexAttribute {
//     //                 format: VertexFormat::Float32x3,
//     //                 offset: 0,
//     //                 shader_location: 3, // shader locations 0-2 are taken up by Position, Normal and UV attributes
//     //             },
//     //             VertexAttribute {
//     //                 format: VertexFormat::Float32x4,
//     //                 offset: VertexFormat::Float32x3.size(),
//     //                 shader_location: 4,
//     //             },
//     //         ],
//     //     });
//     //     descriptor.fragment.as_mut().unwrap().shader = self.shader.clone();
//     //     Ok(descriptor)
//     // }
// }

// type DrawCustom = (
//     SetItemPipeline,
//     SetMeshViewBindGroup<0>,
//     SetMeshBindGroup<1>,
//     DrawMeshInstanced,
// );

// struct DrawMeshInstanced;

// impl<P: PhaseItem> RenderCommand<P> for DrawMeshInstanced {
//     type Param = (SRes<RenderAssets<Mesh>>, SRes<RenderMesh2dInstances>);
//     type ViewQuery = ();
//     type ItemQuery = Read<InstanceBuffer>;

//     #[inline]
//     fn render<'w>(
//         item: &P,
//         _view: (),
//         instance_buffer: Option<&'w InstanceBuffer>,
//         (meshes, render_mesh2d_instances): SystemParamItem<'w, '_, Self::Param>,
//         pass: &mut TrackedRenderPass<'w>,
//     ) -> RenderCommandResult {
//         let meshes = meshes.into_inner();
//         let render_mesh2d_instances = render_mesh2d_instances.into_inner();

//         let Some(RenderMesh2dInstance { mesh_asset_id, .. }) =
//             render_mesh2d_instances.get(&item.entity())
//         else {
//             return RenderCommandResult::Failure;
//         };

//         let Some(gpu_mesh) = meshes.get(*mesh_asset_id) else {
//             return RenderCommandResult::Failure;
//         };

//         let Some(instance_buffer) = instance_buffer else {
//             dbg!("non succ 3");
//             return RenderCommandResult::Failure;
//         };

//         pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
//         pass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));

//         match &gpu_mesh.buffer_info {
//             GpuBufferInfo::Indexed {
//                 buffer,
//                 index_format,
//                 count,
//             } => {
//                 pass.set_index_buffer(buffer.slice(..), 0, *index_format);
//                 pass.draw_indexed(0..*count, 0, 0..instance_buffer.length as u32);
//             }
//             GpuBufferInfo::NonIndexed => {
//                 pass.draw(0..gpu_mesh.vertex_count, 0..instance_buffer.length as u32);
//             }
//         }

//         dbg!("succ");

//         RenderCommandResult::Success
//     }
// }

// pub fn extract_particles(
//     mut commands: Commands,
//     // mut previous_len: Local<usize>,
//     query: Extract<Query<(Entity, &Mesh2dHandle, &GlobalTransform), With<ParticleInstances>>>,
//     mut render_mesh_instances: ResMut<RenderMesh2dInstances>,
// ) {
//     if let Ok((entity, mesh, transform)) = query.get_single() {
//         let transforms = Mesh2dTransforms {
//             transform: (&transform.affine()).into(),
//             flags: MeshFlags::empty().bits(),
//         };

//         commands.insert_or_spawn_batch([(entity, ParticleInstances)]);

//         render_mesh_instances.insert(
//             entity,
//             RenderMesh2dInstance {
//                 mesh_asset_id: mesh.0.id(),
//                 transforms,
//                 material_bind_group_id: Material2dBindGroupId::default(),
//                 automatic_batching: false,
//             },
//         );
//     }
// }
