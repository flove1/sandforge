use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::{ ComponentUniforms, ExtractComponent }, render_asset::RenderAssets, render_graph::{ NodeRunError, RenderGraphContext, RenderLabel, ViewNode }, render_resource::{ binding_types::{ sampler, texture_2d, uniform_buffer }, * }, renderer::{ RenderContext, RenderDevice }, texture::{BevyDefault, GpuImage}, view::ViewTarget
    },
};

use crate::camera::LightingTexture;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct ApplyLightingLabel;

#[derive(Default)]
pub(crate) struct ApplyLightingNode;

impl ViewNode for ApplyLightingNode {
    type ViewQuery = (&'static ViewTarget, &'static LightApply);

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _): QueryItem<Self::ViewQuery>,
        world: &World
    ) -> Result<(), NodeRunError> {
        let post_process_pipeline = world.resource::<ApplyLightingPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let Some(pipeline) = pipeline_cache.get_render_pipeline(
            post_process_pipeline.pipeline_id
        ) else {
            return Ok(());
        };

        let Some(lighting) = world.get_resource::<LightingTexture>() else {
            return Ok(());
        };

        let Some(gpu_images) = world.get_resource::<RenderAssets<Image>>() else {
            return Ok(());
        };

        let Some(image) = gpu_images.get(&lighting.texture) else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();
        let bind_group = render_context
            .render_device()
            .create_bind_group(
                "apply_bind_group",
                &post_process_pipeline.layout,
                &BindGroupEntries::sequential((
                    post_process.source,
                    &image.texture_view,
                    &post_process_pipeline.sampler,
                ))
            );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("apply_process_pass"),
            color_attachments: &[
                Some(RenderPassColorAttachment {
                    view: post_process.destination,
                    resolve_target: None,
                    ops: Operations::default(),
                }),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);


        Ok(())
    }
}

#[derive(Resource)]
pub(crate) struct ApplyLightingPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for ApplyLightingPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "apply_bind_group_layout",
            &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ))
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());
        let shader = world.resource::<AssetServer>().load("shaders/light_apply.wgsl");

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            .queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("apply_pipeline".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader,
                    shader_defs: vec![],
                    entry_point: "fragment".into(),
                    targets: vec![
                        Some(ColorTargetState {
                            format: TextureFormat::bevy_default(),
                            blend: Some(BlendState {
                                color: BlendComponent::OVER,
                                alpha: BlendComponent::OVER,
                            }),
                            write_mask: ColorWrites::ALL,
                        })
                    ],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
            });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

#[derive(Component, Default, Clone, Copy, ExtractComponent)]
pub struct LightApply;