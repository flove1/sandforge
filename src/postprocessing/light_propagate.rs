use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::ExtractComponent, render_graph::{ NodeRunError, RenderGraphContext, RenderLabel, ViewNode }, render_resource::{ binding_types::{ sampler, texture_2d, uniform_buffer }, * }, renderer::{ RenderContext, RenderDevice }, texture::BevyDefault, view::ViewTarget
    },
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub(crate) struct LightPropagationLabel;

#[derive(Default)]
pub(crate) struct LightPropagationNode;

impl ViewNode for LightPropagationNode {
    type ViewQuery = (&'static ViewTarget, &'static LightPropagationSettings);

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, propagation_settings): QueryItem<Self::ViewQuery>,
        world: &World
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<LightPropagationPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let Some(pipeline_id) = pipeline_cache.get_render_pipeline(
            pipeline.pipeline_id
        ) else {
            return Ok(());
        };

        for step in 0..propagation_settings.passes {
            let post_process = view_target.post_process_write();

            let offset_buffer = render_context.render_device().create_buffer_with_data(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&(0.5 + step as f32 * propagation_settings.offset)),
                usage: BufferUsages::UNIFORM,
            });
    
            let bind_group = render_context
                .render_device()
                .create_bind_group(
                    "propagate_bind_group",
                    &pipeline.layout,
                    &BindGroupEntries::sequential((
                        post_process.source,
                        &pipeline.sampler,
                        offset_buffer.as_entire_binding()
                    ))
                );
    
            let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("propagate_process_pass"),
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
    
            render_pass.set_render_pipeline(pipeline_id);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}

#[derive(Resource)]
pub(crate) struct LightPropagationPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for LightPropagationPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "propagate_bind_group_layout",
            &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<f32>(false),
            ))
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });
        let shader = world.resource::<AssetServer>().load("shaders/light_propagate.wgsl");

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            .queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("propagate_pipeline".into()),
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

#[repr(C)]
#[derive(Component, Default, Clone, Copy, ExtractComponent)]
pub struct LightPropagationSettings {
    pub offset: f32,
    pub passes: u32,
}
