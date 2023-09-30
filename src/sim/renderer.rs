use rapier2d::{prelude::{vector, nalgebra}, na::{Matrix2, Vector2}};
use wgpu::util::DeviceExt;

use crate::{constants::{PHYSICS_SCALE, WORLD_WIDTH, WORLD_HEIGHT, CHUNK_SIZE}, vector::Pos2};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ]
        }
    }
}

pub struct Renderer {
    objects: Vec<(wgpu::BindGroup, wgpu::Buffer)>,
    chunks: Vec<(wgpu::BindGroup, wgpu::Buffer)>,
    colliders: Vec<(wgpu::Buffer, wgpu::Buffer, u32)>,

    texture_indices: wgpu::Buffer,

    texture_render_pipeline: wgpu::RenderPipeline,
    collider_render_pipeline: wgpu::RenderPipeline,

    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: &wgpu::TextureFormat) -> Self {
        let texture_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/texture.wgsl"));
        let collider_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/collider.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

        let texture_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Renderer pipeline layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            })),
            vertex: wgpu::VertexState {
                module: &texture_shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &texture_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: *format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multiview: None,
            multisample: wgpu::MultisampleState::default(),
        });

        let collider_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Renderer pipeline layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            })),
            vertex: wgpu::VertexState {
                module: &collider_shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &collider_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: *format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multiview: None,
            multisample: wgpu::MultisampleState::default(),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let texture_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Renderer index buffer"),
            contents: bytemuck::cast_slice(&[
                0u32, 1u32, 2u32,
                2u32, 1u32, 3u32
            ]),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            objects: vec![],
            chunks: vec![],
            colliders: vec![],

            texture_render_pipeline,
            collider_render_pipeline,
            bind_group_layout,
            sampler,
            texture_indices
        }
    }

    pub(crate) fn update(
        &mut self, 
        device: &wgpu::Device,
        colliders: &rapier2d::prelude::ColliderSet,
        chunk_textures: Vec<(wgpu::TextureView, Pos2)>,
        object_textures: Vec<(wgpu::TextureView, Vector2<f32>, f32, i32, i32)>,
    ) {
        self.colliders = colliders.iter()
            .map(|(_, collider)| {
                if let Some(shape) = collider.shape().as_polyline() {
                    let vertices = shape.vertices().iter()
                        .map(|vertex| {
                            Vertex {
                                position: [
                                    (vertex.x + collider.position().translation.x) / 4.0 * PHYSICS_SCALE - 1.0,
                                    (vertex.y + collider.position().translation.y) / 4.0 * PHYSICS_SCALE - 1.0,
                                    0.0,
                                ],
                                tex_coords: [0.0; 2],
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
                                                (rotated_vertex.y + collider.position().translation.y) / 4.0 * PHYSICS_SCALE - 1.0,
                                                0.0,
                                            ],
                                            tex_coords: [0.0; 2],
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
            .map(|(vertices, indeces)| {
                let vertices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Renderer vertex buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });


                let indeces_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Renderer index buffer"),
                    contents: bytemuck::cast_slice(&indeces),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                });

                (vertices_buffer, indeces_buffer, indeces.len() as u32)
            })
            .collect();

        self.chunks = chunk_textures.into_iter()
            .map(|(texture, pos)| {
                let bind_group = device.create_bind_group(
                    &wgpu::BindGroupDescriptor {
                        layout: &self.bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&texture),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                        ],
                        label: Some("diffuse_bind_group"),
                    }
                );

                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Renderer index buffer"),
                    contents: bytemuck::cast_slice(&[
                        Vertex { 
                            position: [
                                (pos.x as f32 / WORLD_WIDTH as f32 - 0.5) * 2.0,
                                (pos.y as f32 / WORLD_HEIGHT as f32 - 0.5) * 2.0,
                                0.0,
                            ], 
                            tex_coords: [0.0, 0.0] 
                        },
                        Vertex { 
                            position: [
                                (pos.x as f32 / WORLD_WIDTH as f32 - 0.5) * 2.0,
                                ((pos.y + 1) as f32 / WORLD_HEIGHT as f32 - 0.5) * 2.0,
                                0.0,
                            ], 
                            tex_coords: [0.0, 1.0] 
                        },
                        Vertex { 
                            position: [
                                ((pos.x + 1) as f32 / WORLD_WIDTH as f32 - 0.5) * 2.0,
                                (pos.y as f32 / WORLD_HEIGHT as f32 - 0.5) * 2.0,
                                0.0,
                            ], 
                            tex_coords: [1.0, 0.0] 
                        },
                        Vertex { 
                            position: [
                                ((pos.x + 1) as f32 / WORLD_WIDTH as f32 - 0.5) * 2.0,
                                ((pos.y + 1) as f32 / WORLD_HEIGHT as f32 - 0.5) * 2.0,
                                0.0,
                            ], 
                            tex_coords: [1.0, 1.0] 
                        },
                    ]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                (bind_group, buffer)
            })
            .collect();

        self.objects = object_textures.into_iter()
            .map(|(texture, pos, angle, width, height)| {
                let bind_group = device.create_bind_group(
                    &wgpu::BindGroupDescriptor {
                        layout: &self.bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&texture),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                        ],
                        label: Some("diffuse_bind_group"),
                    }
                );

                let rotation_matrix = rapier2d::na::Matrix2::new(
                    angle.cos(), 
                    -angle.sin(), 
                    angle.sin(), 
                    angle.cos()
                );


                let points = [
                    rotation_matrix * 
                        vector![
                            - width as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                            - height as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32
                        ],
                    rotation_matrix * 
                        vector![
                            - width as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                            height as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32
                        ],
                    rotation_matrix * 
                        vector![
                            width as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                            - height as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32
                        ],
                    rotation_matrix * 
                        vector![
                            width as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                            height as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32
                        ],
                ];

                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Renderer index buffer"),
                    contents: bytemuck::cast_slice(&[
                        Vertex { 
                            position: [
                                pos.x / 4.0 * PHYSICS_SCALE + points[0].x - 1.0,
                                pos.y / 4.0 * PHYSICS_SCALE + points[0].y - 1.0,
                                0.0,
                            ], 
                            tex_coords: [0.0, 0.0] 
                        },
                        Vertex { 
                            position: [
                                pos.x / 4.0 * PHYSICS_SCALE + points[1].x - 1.0,
                                pos.y / 4.0 * PHYSICS_SCALE + points[1].y - 1.0,
                                0.0,
                            ], 
                            tex_coords: [0.0, 1.0] 
                        },
                        Vertex { 
                            position: [
                                pos.x / 4.0 * PHYSICS_SCALE + points[2].x - 1.0,
                                pos.y / 4.0 * PHYSICS_SCALE + points[2].y - 1.0,
                                0.0,
                            ], 
                            tex_coords: [1.0, 0.0] 
                        },
                        Vertex { 
                            position: [
                                pos.x / 4.0 * PHYSICS_SCALE + points[3].x - 1.0,
                                pos.y / 4.0 * PHYSICS_SCALE + points[3].y - 1.0,
                                0.0,
                            ], 
                            tex_coords: [1.0, 1.0] 
                        },
                    ]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                (bind_group, buffer)

            })
            .collect()
    }

    pub(crate) fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView
    ) {

        self.chunks.iter()
            .for_each(|(bind_group, bind_buffer)| {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Renderer render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });
        
                rpass.set_pipeline(&self.texture_render_pipeline);
                rpass.set_bind_group(0, bind_group, &[]);
                rpass.set_vertex_buffer(0, bind_buffer.slice(..));
                rpass.set_index_buffer(self.texture_indices.slice(..), wgpu::IndexFormat::Uint32);
                // rpass.draw(0..4, 0..1);
                rpass.draw_indexed(0..6, 0, 0..1);
            });


        self.objects.iter()
            .for_each(|(bind_group, bind_buffer)| {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Renderer render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });
        
                rpass.set_pipeline(&self.texture_render_pipeline);
                rpass.set_bind_group(0, bind_group, &[]);
                rpass.set_vertex_buffer(0, bind_buffer.slice(..));
                rpass.set_index_buffer(self.texture_indices.slice(..), wgpu::IndexFormat::Uint32);
                // rpass.draw(0..4, 0..1);
                rpass.draw_indexed(0..6, 0, 0..1);
            });

        self.colliders.iter()
            .for_each(|(vertices, indeces, index_count)| {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Renderer render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });
        
                rpass.set_pipeline(&self.collider_render_pipeline);
                rpass.set_vertex_buffer(0 as u32, vertices.slice(..));
                rpass.set_index_buffer(indeces.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..*index_count, 0, 0..1);   
            });
    }
}