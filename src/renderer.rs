use pixels::{wgpu::{self, util::DeviceExt}, TextureError};
use rapier2d::{prelude::ColliderSet, na::Matrix2};

use crate::constants::PHYSICS_SCALE;

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

pub struct MeshRenderer {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffers: Vec<wgpu::Buffer>,
    index_buffers: Vec<wgpu::Buffer>,
    index_counts: Vec<u32>,
}

impl MeshRenderer {
    pub fn new(
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
                topology: wgpu::PrimitiveTopology::LineList,
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
            index_buffers: vec![],
            index_counts: vec![],
        })
    }

    pub(crate) fn update(&mut self, device: &wgpu::Device, colliders: &ColliderSet) {
        // ((((vertex.x + chunk_position.x as f32) * CHUNK_SIZE as f32) / (CHUNK_SIZE * WORLD_WIDTH) as f32) - 0.5) * 2.0, 
        // ((-((vertex.y + chunk_position.y as f32) * CHUNK_SIZE as f32) / (CHUNK_SIZE * WORLD_HEIGHT) as f32) + 0.5) * 2.0

        let mut new_vertex_buffers = vec![];
        let mut new_index_buffers = vec![];
        let mut new_index_counts = vec![];

        colliders.iter()
            .map(|(_, collider)| {
                if let Some(shape) = collider.shape().as_polyline() {
                    let vertices = shape.vertices().iter()
                        .map(|vertex| {
                            Vertex {
                                position: [
                                    (vertex.x + collider.position().translation.x) / 4.0 * PHYSICS_SCALE - 1.0,
                                    -(vertex.y + collider.position().translation.y) / 4.0 * PHYSICS_SCALE + 1.0
                                ]
                            }
                        })
                        .collect::<Vec<Vertex>>();

                    let mut indices = vec![];

                    for index in 0..(vertices.len() as u32 - 1) {
                        indices.push(index);
                        indices.push(index + 1);
                    }

                    indices.push(vertices.len() as u32 - 1);
                    indices.push(0);

                    (vertices, indices)
                }
                else if let Some(shapes) = collider.shape().as_compound() {
                    let mut vertices = vec![];
                    let rotation = collider.rotation();

                    let rotation_matrix = Matrix2::new(
                        rotation.angle().cos(), 
                        -rotation.angle().sin(), 
                        rotation.angle().sin(), 
                        rotation.angle().cos()
                    );
                    
                    shapes.shapes().iter()
                        .for_each(|shape| {
                            shape.1
                                .as_triangle().unwrap()
                                .vertices().iter()
                                .for_each(|vertex| {
                                    let rotated_vertex = rotation_matrix * vertex;

                                    vertices.push(
                                        Vertex {
                                            position: [
                                                (rotated_vertex.x + collider.position().translation.x) / 4.0 * PHYSICS_SCALE - 1.0,
                                                -(rotated_vertex.y + collider.position().translation.y) / 4.0 * PHYSICS_SCALE + 1.0
                                            ]
                                        }
                                    )
                                })
                        });


                    let mut indices = vec![];
                
                    for index in 0..(vertices.len() as u32 / 3) {
                        for iteration in 0..2 {
                            indices.push(index * 3 + iteration);
                            indices.push(index * 3 + iteration + 1);
                        }

                        indices.push(index * 3 + 2);
                        indices.push(index * 3);
                    }

                    (vertices, indices)

                }
                else {
                    panic!()
                }
            })
            .for_each(|(vertices, indeces)| {
                new_vertex_buffers.push(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("MeshRenderer vertex buffer"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    })
                );


                new_index_buffers.push(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("MeshRenderer index buffer"),
                        contents: bytemuck::cast_slice(&indeces),
                        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    })
                );

                new_index_counts.push(indeces.len() as u32);
            });

        self.vertex_buffers = new_vertex_buffers;
        self.index_buffers = new_index_buffers;
        self.index_counts = new_index_counts;
    }

    pub(crate) fn render(&self, encoder: &mut wgpu::CommandEncoder, render_target: &wgpu::TextureView) {
        for index in 0..self.index_counts.len() {
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
            rpass.set_index_buffer(self.index_buffers[index].slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..self.index_counts[index], 0, 0..1);   
        }
    }
}