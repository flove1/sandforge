use bevy::{
    core_pipeline::core_2d::graph::{ Core2d, Node2d },
    prelude::*,
    render::{
        extract_component::{ ExtractComponentPlugin, UniformComponentPlugin }, render_graph::{ RenderGraphApp, ViewNodeRunner }, render_phase::AddRenderCommand, RenderApp
    },
};

use self::{
    light_apply::{LightApply, ApplyLightingLabel, ApplyLightingNode, ApplyLightingPipeline}, light_propagate::{ LightPropagationLabel, LightPropagationNode, LightPropagationPipeline, LightPropagationSettings }, light_calculate::{ CalculateLightingLabel, CalculateLightingNode, CalculateLightingPipeline, LightMask }
};

pub mod light_propagate;
pub mod light_calculate;
pub mod light_apply;

pub struct PostProcessPlugin;

impl Plugin for PostProcessPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<LightPropagationSettings>::default(),
            ExtractComponentPlugin::<LightMask>::default(),
            ExtractComponentPlugin::<LightApply>::default(),
        ));

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_graph_node::<ViewNodeRunner<LightPropagationNode>>(
                Core2d,
                LightPropagationLabel
            )
            .add_render_graph_node::<ViewNodeRunner<CalculateLightingNode>>(
                Core2d,
                CalculateLightingLabel
            )
            .add_render_graph_node::<ViewNodeRunner<ApplyLightingNode>>(
                Core2d,
                ApplyLightingLabel
            )
            .add_render_graph_edges(
                Core2d,
                (Node2d::Tonemapping, CalculateLightingLabel, LightPropagationLabel, ApplyLightingLabel, Node2d::EndMainPassPostProcessing)
            );
    }

    fn finish(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<CalculateLightingPipeline>()
            .init_resource::<LightPropagationPipeline>()
            .init_resource::<ApplyLightingPipeline>();
    }
}

// // //! A shader that renders a mesh multiple times in one draw call.

// // use bevy::{
// //     core_pipeline::core_2d::Transparent2d, ecs::{
// //         query::QueryItem,
// //         system::{lifetimeless::*, SystemParamItem},
// //     }, pbr::{
// //         MeshPipeline, MeshPipelineKey, RenderMeshInstances, SetMeshBindGroup, SetMeshViewBindGroup,
// //     }, prelude::*, render::{
// //         extract_component::{ExtractComponent, ExtractComponentPlugin},
// //         mesh::{GpuBufferInfo, MeshVertexBufferLayout},
// //         render_asset::RenderAssets,
// //         render_phase::{
// //             AddRenderCommand, DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult,
// //             RenderPhase, SetItemPipeline, TrackedRenderPass,
// //         },
// //         render_resource::*,
// //         renderer::RenderDevice,
// //         view::{ExtractedView, NoFrustumCulling},
// //         Render, RenderApp, RenderSet,
// //     }, utils::FloatOrd
// // };
// // use bytemuck::{Pod, Zeroable};

// // pub fn setup_post(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
// //     commands.spawn((
// //         meshes.add(Cuboid::new(0.5, 0.5, 0.5)),
// //         SpatialBundle::INHERITED_IDENTITY,
// //         InstanceMaterialData(
// //             (1..=10)
// //                 .flat_map(|x| (1..=10).map(move |y| (x as f32 / 10.0, y as f32 / 10.0)))
// //                 .map(|(x, y)| InstanceData {
// //                     position: Vec3::new(x * 10.0 - 5.0, y * 10.0 - 5.0, 0.0),
// //                     scale: 1.0,
// //                     color: Color::hsla(x * 360., y, 0.5, 1.0).as_rgba_f32(),
// //                 })
// //                 .collect(),
// //         ),
// //         // NOTE: Frustum culling is done based on the Aabb of the Mesh and the GlobalTransform.
// //         // As the cube is at the origin, if its Aabb moves outside the view frustum, all the
// //         // instanced cubes will be culled.
// //         // The InstanceMaterialData contains the 'GlobalTransform' information for this custom
// //         // instancing, and that is not taken into account with the built-in frustum culling.
// //         // We must disable the built-in frustum culling by adding the `NoFrustumCulling` marker
// //         // component to avoid incorrect culling.
// //         NoFrustumCulling,
// //     ));

// //     // // camera
// //     // commands.spawn(Camera2dBundle {
// //     //     transform: Transform::from_xyz(0.0, 0.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
// //     //     ..default()
// //     // });
// // }

// // #[derive(Component, Deref)]
// // struct InstanceMaterialData(Vec<InstanceData>);

// // impl ExtractComponent for InstanceMaterialData {
// //     type QueryData = &'static InstanceMaterialData;
// //     type QueryFilter = ();
// //     type Out = Self;

// //     fn extract_component(item: QueryItem<'_, Self::QueryData>) -> Option<Self> {
// //         Some(InstanceMaterialData(item.0.clone()))
// //     }
// // }

// // pub struct CustomMaterialPlugin;

// // impl Plugin for CustomMaterialPlugin {
// //     fn build(&self, app: &mut App) {
// //         app.add_plugins(ExtractComponentPlugin::<InstanceMaterialData>::default());
// //         app.sub_app_mut(RenderApp)
// //             .add_render_command::<Transparent2d, DrawCustom>()
// //             .init_resource::<SpecializedMeshPipelines<CustomPipeline>>()
// //             .add_systems(
// //                 Render,
// //                 (
// //                     queue_custom.in_set(RenderSet::QueueMeshes),
// //                     prepare_instance_buffers.in_set(RenderSet::PrepareResources),
// //                 ),
// //             );
// //     }

// //     fn finish(&self, app: &mut App) {
// //         app.sub_app_mut(RenderApp).init_resource::<CustomPipeline>();
// //     }
// // }

// // #[derive(Clone, Copy, Pod, Zeroable)]
// // #[repr(C)]
// // struct InstanceData {
// //     position: Vec3,
// //     scale: f32,
// //     color: [f32; 4],
// // }

// // #[allow(clippy::too_many_arguments)]
// // fn queue_custom(
// //     transparent_2d_draw_functions: Res<DrawFunctions<Transparent2d>>,
// //     custom_pipeline: Res<CustomPipeline>,
// //     msaa: Res<Msaa>,
// //     mut pipelines: ResMut<SpecializedMeshPipelines<CustomPipeline>>,
// //     pipeline_cache: Res<PipelineCache>,
// //     meshes: Res<RenderAssets<Mesh>>,
// //     render_mesh_instances: Res<RenderMeshInstances>,
// //     material_meshes: Query<Entity, With<InstanceMaterialData>>,
// //     mut views: Query<(&ExtractedView, &mut RenderPhase<Transparent2d>)>,
// // ) {
// //     let draw_custom = transparent_2d_draw_functions.read().id::<DrawCustom>();

// //     let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());

// //     for (view, mut transparent_phase) in &mut views {
// //         let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
// //         for entity in &material_meshes {
// //             let Some(mesh_instance) = render_mesh_instances.get(&entity) else {
// //                 continue;
// //             };
// //             let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
// //                 continue;
// //             };
// //             let key = view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology);
// //             let pipeline = pipelines
// //                 .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
// //                 .unwrap();
// //             transparent_phase.add(Transparent2d {
// //                 entity,
// //                 pipeline,
// //                 draw_function: draw_custom,
// //                 batch_range: 0..1,
// //                 dynamic_offset: None,
// //                 sort_key: FloatOrd(1.0),
// //             });
// //         }
// //     }
// // }

// // #[derive(Component)]
// // struct InstanceBuffer {
// //     buffer: Buffer,
// //     length: usize,
// // }

// // fn prepare_instance_buffers(
// //     mut commands: Commands,
// //     query: Query<(Entity, &InstanceMaterialData)>,
// //     render_device: Res<RenderDevice>,
// // ) {
// //     for (entity, instance_data) in &query {
// //         let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
// //             label: Some("instance data buffer"),
// //             contents: bytemuck::cast_slice(instance_data.as_slice()),
// //             usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
// //         });
// //         commands.entity(entity).insert(InstanceBuffer {
// //             buffer,
// //             length: instance_data.len(),
// //         });
// //     }
// // }

// // #[derive(Resource)]
// // struct CustomPipeline {
// //     shader: Handle<Shader>,
// //     mesh_pipeline: MeshPipeline,
// // }

// // impl FromWorld for CustomPipeline {
// //     fn from_world(world: &mut World) -> Self {
// //         let asset_server = world.resource::<AssetServer>();
// //         let shader = asset_server.load("shaders/instancing.wgsl");

// //         let mesh_pipeline = world.resource::<MeshPipeline>();

// //         CustomPipeline {
// //             shader,
// //             mesh_pipeline: mesh_pipeline.clone(),
// //         }
// //     }
// // }

// // impl SpecializedMeshPipeline for CustomPipeline {
// //     type Key = MeshPipelineKey;

// //     fn specialize(
// //         &self,
// //         key: Self::Key,
// //         layout: &MeshVertexBufferLayout,
// //     ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
// //         let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

// //         descriptor.vertex.shader = self.shader.clone();
// //         descriptor.vertex.buffers.push(VertexBufferLayout {
// //             array_stride: std::mem::size_of::<InstanceData>() as u64,
// //             step_mode: VertexStepMode::Instance,
// //             attributes: vec![
// //                 VertexAttribute {
// //                     format: VertexFormat::Float32x4,
// //                     offset: 0,
// //                     shader_location: 3, // shader locations 0-2 are taken up by Position, Normal and UV attributes
// //                 },
// //                 VertexAttribute {
// //                     format: VertexFormat::Float32x4,
// //                     offset: VertexFormat::Float32x4.size(),
// //                     shader_location: 4,
// //                 },
// //             ],
// //         });
// //         descriptor.fragment.as_mut().unwrap().shader = self.shader.clone();
// //         Ok(descriptor)
// //     }
// // }

// // type DrawCustom = (
// //     SetItemPipeline,
// //     SetMeshViewBindGroup<0>,
// //     SetMeshBindGroup<1>,
// //     DrawMeshInstanced,
// // );

// // struct DrawMeshInstanced;

// // impl<P: PhaseItem> RenderCommand<P> for DrawMeshInstanced {
// //     type Param = (SRes<RenderAssets<Mesh>>, SRes<RenderMeshInstances>);
// //     type ViewQuery = ();
// //     type ItemQuery = Read<InstanceBuffer>;

// //     #[inline]
// //     fn render<'w>(
// //         item: &P,
// //         _view: (),
// //         instance_buffer: Option<&'w InstanceBuffer>,
// //         (meshes, render_mesh_instances): SystemParamItem<'w, '_, Self::Param>,
// //         pass: &mut TrackedRenderPass<'w>,
// //     ) -> RenderCommandResult {
// //         let Some(mesh_instance) = render_mesh_instances.get(&item.entity()) else {
// //             return RenderCommandResult::Failure;
// //         };
// //         let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id) else {
// //             return RenderCommandResult::Failure;
// //         };
// //         let Some(instance_buffer) = instance_buffer else {
// //             return RenderCommandResult::Failure;
// //         };

// //         pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
// //         pass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));

// //         match &gpu_mesh.buffer_info {
// //             GpuBufferInfo::Indexed {
// //                 buffer,
// //                 index_format,
// //                 count,
// //             } => {
// //                 pass.set_index_buffer(buffer.slice(..), 0, *index_format);
// //                 pass.draw_indexed(0..*count, 0, 0..instance_buffer.length as u32);
// //             }
// //             GpuBufferInfo::NonIndexed => {
// //                 pass.draw(0..gpu_mesh.vertex_count, 0..instance_buffer.length as u32);
// //             }
// //         }
// //         RenderCommandResult::Success
// //     }
// // }
