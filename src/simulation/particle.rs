use async_channel::Sender;
use bevy::{
    prelude::*,
    render::{mesh::PrimitiveTopology, render_asset::RenderAssetUsages},
    sprite::Mesh2dHandle,
    tasks::ComputeTaskPool,
    utils::HashMap,
};
use bevy_math::ivec2;
use bytemuck::{Pod, Zeroable};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::constants::{CHUNK_SIZE, PARTICLE_LAYER};

use super::{
    chunk_groups::ChunkGroup3x3,
    dirty_rect::{
        update_dirty_rects, update_dirty_rects_3x3, DirtyRects, RenderMessage, UpdateMessage,
    },
    materials::{MaterialInstance, PhysicsType},
    pixel::Pixel,
    world::ChunkManager,
};

#[derive(Component, Default)]
pub struct ParticleInstances;

#[derive(Reflect, Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum InObjectState {
    FirstFrame,
    Inside,
    Outside,
}

#[derive(Component, Reflect, Debug, Clone, Serialize, Deserialize)]
pub struct Particle {
    pub active: bool,
    pub material: MaterialInstance,
    pub pos: Vec2,
    pub vel: Vec2,
    pub in_object_state: InObjectState,
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ParticleRenderInstance {
    pub pos: Vec3,
    pub color: [f32; 4],
}

impl From<&Particle> for ParticleRenderInstance {
    fn from(val: &Particle) -> Self {
        ParticleRenderInstance {
            pos: val.pos.extend(PARTICLE_LAYER),
            color: [
                val.material.color[0] as f32 / 255.0,
                val.material.color[1] as f32 / 255.0,
                val.material.color[2] as f32 / 255.0,
                val.material.color[3] as f32 / 255.0,
            ],
        }
    }
}

impl Particle {
    pub fn new(material: MaterialInstance, pos: Vec2, vel: Vec2) -> Self {
        Self {
            active: true,
            material,
            pos,
            vel,
            in_object_state: InObjectState::FirstFrame,
        }
    }

    pub fn update(&mut self, api: &mut ParticleApi) -> bool {
        let lx = self.pos.x;
        let ly = self.pos.y;

        self.vel.y -= 0.5;

        let dx = self.vel.x;
        let dy = self.vel.y;

        let steps = (dx.abs() + dy.abs()).sqrt() as u32 + 1;
        for s in 0..steps {
            let thru = (s + 1) as f32 / steps as f32;

            self.pos.x = lx + dx * thru;
            self.pos.y = ly + dy * thru;

            let Some(pixel) = api.get(self.pos.x as i32, self.pos.y as i32) else {
                continue;
            };

            if pixel.material.physics_type == PhysicsType::Air {
                self.in_object_state = InObjectState::Outside;
                continue;
            }

            let is_object = matches!(
                pixel.material.physics_type,
                PhysicsType::Rigidbody | PhysicsType::Actor
            );

            match self.in_object_state {
                InObjectState::FirstFrame => {
                    if is_object {
                        self.in_object_state = InObjectState::Inside;
                    } else {
                        self.in_object_state = InObjectState::Outside;
                    }
                }
                InObjectState::Inside => {
                    if !is_object {
                        self.in_object_state = InObjectState::Outside;
                    }
                }
                InObjectState::Outside => {}
            }

            if !is_object || self.in_object_state == InObjectState::Outside {
                match api.get_material(lx as i32, ly as i32) {
                    Some(material) if material.physics_type != PhysicsType::Air => {
                        let succeeded = api.displace(
                            IVec2::new(self.pos.x as i32, self.pos.y as i32),
                            self.material.clone(),
                        );

                        if succeeded {
                            return false;
                        }

                        // upwarp if completely blocked
                        self.vel.y = 1.0;
                        self.pos.y += 16.0;

                        break;
                    }
                    _ => {
                        if api
                            .set(lx as i32, ly as i32, Pixel::new(self.material.clone(), 0))
                            .is_ok()
                        {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }
}

pub struct ParticleApi<'a> {
    pub(super) chunk_position: IVec2,
    pub(super) chunk_group: &'a mut ChunkGroup3x3,
    pub(super) update_send: &'a Sender<UpdateMessage>,
    pub(super) render_send: &'a Sender<RenderMessage>,
}

impl<'a> ParticleApi<'a> {
    pub fn get(&self, x: i32, y: i32) -> Option<Pixel> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        self.chunk_group.get(cell_position).cloned()
    }

    pub fn get_material(&self, x: i32, y: i32) -> Option<MaterialInstance> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        self.chunk_group
            .get(cell_position)
            .map(|pixel| pixel.material.clone())
    }

    pub fn set(&mut self, x: i32, y: i32, pixel: Pixel) -> Result<(), String> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        match self.chunk_group.get_mut(cell_position) {
            Some(old_pixel) => {
                *old_pixel = pixel;

                self.update_send
                    .try_send(UpdateMessage {
                        cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                        chunk_position: self.chunk_position
                            + cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        awake_surrouding: true,
                    })
                    .unwrap();

                self.render_send
                    .try_send(RenderMessage {
                        cell_position: cell_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2(),
                        chunk_position: self.chunk_position
                            + cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
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
        material: MaterialInstance,
        condition: F,
    ) -> Result<(), String> {
        let cell_position = ivec2(x, y) - self.chunk_position * CHUNK_SIZE;

        match self.chunk_group.get_mut(cell_position) {
            Some(pixel) => {
                if condition(pixel.clone()) {
                    *pixel = Pixel::new(material, 0);

                    self.update_send
                        .try_send(UpdateMessage {
                            cell_position: cell_position
                                .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                .as_uvec2(),
                            chunk_position: self.chunk_position
                                + cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                            awake_surrouding: true,
                        })
                        .unwrap();

                    self.render_send
                        .try_send(RenderMessage {
                            cell_position: cell_position
                                .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                .as_uvec2(),
                            chunk_position: self.chunk_position
                                + cell_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
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
    pub fn displace(&mut self, pos: IVec2, material: MaterialInstance) -> bool {
        let mut succeeded = false;

        let scan_w = 32;
        let scan_h = 32;
        let mut scan_pos = IVec2::ZERO;
        let mut scan_delta_pos = IVec2::new(0, -1);
        let scan_max_i = scan_w.max(scan_h) * scan_w.max(scan_h);

        for _ in 0..scan_max_i {
            let check_scan = (scan_pos.x >= -scan_w / 2)
                && (scan_pos.x <= scan_w / 2)
                && (scan_pos.y >= -scan_h / 2)
                && (scan_pos.y <= scan_h / 2);

            if check_scan
                && self
                    .set_with_condition(
                        pos.x + scan_pos.x,
                        pos.y + scan_pos.y,
                        material.clone(),
                        |pixel| (pixel.material.physics_type == PhysicsType::Air),
                    )
                    .is_ok()
            {
                succeeded = true;
                break;
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
}

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, particle_setup)
            .add_systems(
                Update,
                particles_update
                    .run_if(|chunk_manager: Res<ChunkManager>| chunk_manager.clock() % 4 == 0),
            )
            .add_systems(PostUpdate, update_partcile_meshes);
    }
}

pub fn particle_setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mut rect = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );

    let vertices = vec![
        [0.0, 0.0, PARTICLE_LAYER],
        [1.0 / CHUNK_SIZE as f32, 0.0, PARTICLE_LAYER],
        [
            1.0 / CHUNK_SIZE as f32,
            1.0 / CHUNK_SIZE as f32,
            PARTICLE_LAYER,
        ],
        [0.0, 1.0 / CHUNK_SIZE as f32, PARTICLE_LAYER],
    ];

    rect.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

    commands.spawn((
        Name::new("Particle instances"),
        Mesh2dHandle(meshes.add(rect)),
        SpatialBundle::INHERITED_IDENTITY,
        // NoFrustumCulling,
        ParticleInstances,
    ));
}

fn particles_update(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut particles: Query<(Entity, &mut Particle)>,
    particles_instances: Query<Entity, With<ParticleInstances>>,
) {
    let particles_instances = particles_instances.single();

    let DirtyRects {
        new: new_dirty_rects,
        render: render_rects,
        ..
    } = &mut *dirty_rects_resource;

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

        scope.spawn(async move {
            while let Ok(entity) = particle_recv.recv().await {
                commands
                    .entity(particles_instances)
                    .remove_children(&[entity]);
                commands.entity(entity).despawn();
            }
        });

        let update_send = &update_send;
        let render_send = &render_send;
        let particle_send = &particle_send;

        let mut particles_maps = [
            HashMap::default(),
            HashMap::default(),
            HashMap::default(),
            HashMap::default(),
        ];

        for (entity, particle) in particles.iter_mut().filter(|(_, particle)| particle.active) {
            let chunk_position = particle.pos.as_ivec2().div_euclid(IVec2::ONE * CHUNK_SIZE);

            unsafe {
                particles_maps.get_unchecked_mut(
                    (chunk_position.x.abs() % 2 + (chunk_position.y.abs() % 2 * 2)) as usize,
                )
            }
            .entry(chunk_position)
            .or_insert_with(Vec::new)
            .push((entity, particle));
        }

        particles_maps.into_iter().for_each(|map| {
            ComputeTaskPool::get().scope(|scope| {
                map.into_iter()
                    .filter_map(|(position, particles)| {
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
                                        return None;
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

                        Some((position, particles, chunk_group))
                    })
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
                                .filter_map(|(entity, mut particle)| {
                                    let alive = particle.update(&mut api);

                                    if alive {
                                        None
                                    } else {
                                        Some(entity)
                                    }
                                })
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

pub fn update_partcile_meshes(mut particles: Query<(&mut Particle, &mut Transform)>) {
    for (particle, mut transform) in particles.iter_mut() {
        transform.translation.x = particle.pos.x / CHUNK_SIZE as f32;
        transform.translation.y = particle.pos.y / CHUNK_SIZE as f32;
    }
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
