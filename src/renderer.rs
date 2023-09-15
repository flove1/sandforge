use pixels::{wgpu::{self, util::DeviceExt}, TextureError};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

pub(crate) struct MeshRenderer {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffers: Vec<wgpu::Buffer>,
    vertex_counts: Vec<u32>,
}

impl MeshRenderer {
    pub(crate) fn new(
        pixels: &pixels::Pixels,
    ) -> Result<Self, TextureError> {
        let device = pixels.device();

        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/shader.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MeshRenderer pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: pixels.render_texture_format(),
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multiview: None,
            multisample: wgpu::MultisampleState::default(),
        });

        Ok(Self {
            render_pipeline,
            vertex_buffers: vec![],
            vertex_counts: vec![],
        })
    }

    pub(crate) fn update(&mut self, device: &wgpu::Device, boundaries: &[Vec<Vertex>]) {
        let mut new_vertex_buffers = vec![];
        let mut new_vertex_count = vec![];

        for mut boundary in boundaries.to_vec().into_iter() {
            boundary.push(boundary[0]);
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("MeshRenderer vertex buffer"),
                contents: bytemuck::cast_slice(&boundary),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });   

            new_vertex_buffers.push(vertex_buffer);
            new_vertex_count.push(boundary.len() as u32);
        }

        self.vertex_buffers = new_vertex_buffers;
        self.vertex_counts = new_vertex_count;
    }

    pub(crate) fn render(&self, encoder: &mut wgpu::CommandEncoder, render_target: &wgpu::TextureView) {
        for index in 0..self.vertex_counts.len() {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("MeshRenderer render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
    
            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_vertex_buffer(0 as u32, self.vertex_buffers[index].slice(..));
            rpass.draw(0..self.vertex_counts[index] as u32, 0..1);   
        }
    }
}