use rapier2d::{prelude::{vector, nalgebra}, na::{Matrix2, Vector2}};
use wgpu::{util::DeviceExt, Color};

use crate::{constants::{PHYSICS_SCALE, WORLD_WIDTH, WORLD_HEIGHT, CHUNK_SIZE}, vector::Pos2};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl TextureVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TextureVertex>() as wgpu::BufferAddress,
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColliderVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl ColliderVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ColliderVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x4,
                },
            ]
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleVertex {
    pub position: [f32; 3],
}

impl ParticleVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ParticleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ]
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl ParticleInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ]
        }
    }
}

pub struct Renderer {
    objects: Vec<(wgpu::BindGroup, wgpu::Buffer)>,
    chunks: Vec<(wgpu::BindGroup, wgpu::Buffer)>,
    colliders: Vec<(wgpu::Buffer, wgpu::Buffer, u32)>,

    particles: Vec<ParticleInstance>,
    particle_instance_buffer: Option<wgpu::Buffer>,
    particle_vertex_buffer: wgpu::Buffer,

    texture_render_pipeline: wgpu::RenderPipeline,
    collider_render_pipeline: wgpu::RenderPipeline,
    particle_render_pipeline: wgpu::RenderPipeline,

    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: &wgpu::TextureFormat) -> Self {
        let texture_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/texture.wgsl"));
        let collider_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/collider.wgsl"));
        let particle_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/particle.wgsl"));

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
                buffers: &[TextureVertex::desc()],
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
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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
                buffers: &[ColliderVertex::desc()],
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

        let particle_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Renderer pipeline layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            })),
            vertex: wgpu::VertexState {
                module: &particle_shader,
                entry_point: "vs_main",
                buffers: &[ParticleVertex::desc(), ParticleInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &particle_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: *format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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

        let particle_vertices = [
            ParticleVertex {
                position: [
                    0.0, 
                    0.0, 
                    0.0,
                ],
            },
            ParticleVertex {
                position: [
                    0.0,
                    0.25 / (WORLD_HEIGHT + CHUNK_SIZE) as f32, 
                    0.0,
                ],
            },
            ParticleVertex {
                position: [
                    0.25 / (WORLD_WIDTH + CHUNK_SIZE) as f32,
                    0.0,
                    0.0,
                ],
            },
            ParticleVertex {
                position: [
                    0.25 / (WORLD_WIDTH + CHUNK_SIZE) as f32,
                    0.25 / (WORLD_HEIGHT + CHUNK_SIZE) as f32,
                    0.0,
                ],
            },
        ];

        let particle_vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[particle_vertices]),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }
        );
        
        Self {
            objects: vec![],
            chunks: vec![],
            colliders: vec![],

            particles: vec![],
            particle_vertex_buffer,
            particle_instance_buffer: None,

            texture_render_pipeline,
            collider_render_pipeline,
            particle_render_pipeline,

            sampler,
            bind_group_layout,
        }
    }

    fn create_collider_buffers(
        &mut self, 
        device: &wgpu::Device,
        colliders: &rapier2d::prelude::ColliderSet, 
        screen_coords: [f32;4 ]
    ) {
        self.colliders = colliders.iter()
            .map(|(_, collider)| {
                if let Some(shape) = collider.shape().as_polyline() {
                    let vertices = shape.vertices().iter()
                        .map(|vertex| {
                            ColliderVertex {
                                position: [
                                    ((vertex.x + collider.position().translation.x) / WORLD_WIDTH as f32 - 0.5) * 2.0 * PHYSICS_SCALE - screen_coords[0],
                                    ((vertex.y + collider.position().translation.y) / WORLD_HEIGHT as f32 - 0.5) * 2.0 * PHYSICS_SCALE - screen_coords[1],
                                    0.0,
                                ],
                                color: [0.0, 0.5, 1.0, 0.5],
                            }
                        })
                        .collect::<Vec<ColliderVertex>>();

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
                                        ColliderVertex {
                                            position: [
                                                ((rotated_vertex.x + collider.position().translation.x) / WORLD_WIDTH as f32 - 0.5) * 2.0 * PHYSICS_SCALE - screen_coords[0],
                                                ((rotated_vertex.y + collider.position().translation.y) / WORLD_HEIGHT as f32 - 0.5) * 2.0 * PHYSICS_SCALE - screen_coords[1],
                                                0.0,
                                            ],
                                            color: [0.0, 1.0, 0.0, 0.5],
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
    }

    fn create_chunk_buffers(
        &mut self, 
        device: &wgpu::Device,
        chunk_textures: Vec<(wgpu::TextureView, Pos2)>, 
        screen_coords: [f32;4 ]
    ) {
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
                        TextureVertex { 
                            position: [
                                (pos.x as f32 / WORLD_WIDTH as f32 - 0.5) * 2.0 - screen_coords[0],
                                (pos.y as f32 / WORLD_HEIGHT as f32 - 0.5) * 2.0 - screen_coords[1],
                                0.0,
                            ], 
                            tex_coords: [0.0, 0.0] 
                        },
                        TextureVertex { 
                            position: [
                                (pos.x as f32 / WORLD_WIDTH as f32 - 0.5) * 2.0 - screen_coords[0],
                                ((pos.y + 1) as f32 / WORLD_HEIGHT as f32 - 0.5) * 2.0 - screen_coords[1],
                                0.0,
                            ], 
                            tex_coords: [0.0, 1.0] 
                        },
                        TextureVertex { 
                            position: [
                                ((pos.x + 1) as f32 / WORLD_WIDTH as f32 - 0.5) * 2.0  - screen_coords[0],
                                (pos.y as f32 / WORLD_HEIGHT as f32 - 0.5) * 2.0  - screen_coords[1],
                                0.0,
                            ], 
                            tex_coords: [1.0, 0.0] 
                        },
                        TextureVertex { 
                            position: [
                                ((pos.x + 1) as f32 / WORLD_WIDTH as f32 - 0.5) * 2.0 - screen_coords[0],
                                ((pos.y + 1) as f32 / WORLD_HEIGHT as f32 - 0.5) * 2.0 - screen_coords[1],
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
    }

    fn create_objects_buffers(
        &mut self, 
        device: &wgpu::Device,
        object_textures: Vec<(wgpu::TextureView, Vector2<f32>, f32, i32, i32)>, 
        screen_coords: [f32;4 ]
    ) {
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
                            - height as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                        ],
                    rotation_matrix * 
                        vector![
                            - width as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                            height as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                        ],
                    rotation_matrix * 
                        vector![
                            width as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                            - height as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                        ],
                    rotation_matrix * 
                        vector![
                            width as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                            height as f32 / (WORLD_WIDTH * CHUNK_SIZE) as f32,
                        ],
                ];

                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Renderer index buffer"),
                    contents: bytemuck::cast_slice(&[
                        TextureVertex { 
                            position: [
                                pos.x / 4.0 * PHYSICS_SCALE + points[0].x - 1.0 - screen_coords[0],
                                pos.y / 4.0 * PHYSICS_SCALE + points[0].y - 1.0 - screen_coords[1],
                                0.0,
                            ], 
                            tex_coords: [0.0, 0.0] 
                        },
                        TextureVertex { 
                            position: [
                                pos.x / 4.0 * PHYSICS_SCALE + points[1].x - 1.0 - screen_coords[0],
                                pos.y / 4.0 * PHYSICS_SCALE + points[1].y - 1.0 - screen_coords[1],
                                0.0,
                            ], 
                            tex_coords: [0.0, 1.0] 
                        },
                        TextureVertex { 
                            position: [
                                pos.x / 4.0 * PHYSICS_SCALE + points[2].x - 1.0 - screen_coords[0],
                                pos.y / 4.0 * PHYSICS_SCALE + points[2].y - 1.0 - screen_coords[1],
                                0.0,
                            ], 
                            tex_coords: [1.0, 0.0] 
                        },
                        TextureVertex { 
                            position: [
                                pos.x / 4.0 * PHYSICS_SCALE + points[3].x - 1.0 - screen_coords[0],
                                pos.y / 4.0 * PHYSICS_SCALE + points[3].y - 1.0 - screen_coords[1],
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

    fn create_particle_buffers(
        &mut self, 
        device: &wgpu::Device,
        particles: Vec<(f32, f32, [u8; 4])>, 
        screen_coords: [f32;4 ]
    ) {
        self.particles = particles.into_iter()
            .map(|(x, y, color)| {
                let color = [
                    color[0] as f32 / 255.0,
                    color[1] as f32 / 255.0,
                    color[2] as f32 / 255.0,
                    color[3] as f32 / 255.0,
                ];

                ParticleInstance { 
                    position: [
                        (x / WORLD_WIDTH as f32 - 0.5) * 2.0 - screen_coords[0],
                        (y / WORLD_HEIGHT as f32 - 0.5) * 2.0 - screen_coords[1],
                        0.0,
                    ], 
                    color,
                }
            })
            .collect();

        if !self.particles.is_empty() {
            self.particle_instance_buffer = Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Renderer index buffer"),
                    contents: bytemuck::cast_slice(&self.particles),
                    usage: wgpu::BufferUsages::VERTEX,
                })
            );
        }
        else {
            self.particle_instance_buffer = None;
        }
        
    }

    pub(crate) fn update(
        &mut self,
        mut screen_coords: [f32; 4],
        device: &wgpu::Device,
        colliders: &rapier2d::prelude::ColliderSet,
        chunk_textures: Vec<(wgpu::TextureView, Pos2)>,
        object_textures: Vec<(wgpu::TextureView, Vector2<f32>, f32, i32, i32)>,
        particles: Vec<(f32, f32, [u8; 4])>
    ) {
        screen_coords.iter_mut()
            .for_each(|cord| *cord = (*cord) / 4.0);

        self.create_chunk_buffers(device, chunk_textures, screen_coords);
        self.create_objects_buffers(device, object_textures, screen_coords);
        self.create_particle_buffers(device, particles, screen_coords);
        self.create_collider_buffers(device, colliders, screen_coords);
    }

    pub(crate) fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView
    ) {
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Renderer render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Color { r: 0.52, g: 0.8, b: 0.92, a: 1.0 }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }

        self.chunks.iter()
            .for_each(|(bind_group, bind_buffer)| {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Renderer render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
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
                rpass.draw(0..4, 0..1);
            });

        self.objects.iter()
            .for_each(|(bind_group, bind_buffer)| {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Renderer render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
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
                rpass.draw(0..4, 0..1);
            });


        if let Some(instance_buffer) = &self.particle_instance_buffer {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Renderer render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
    
            rpass.set_pipeline(&self.particle_render_pipeline);
            rpass.set_vertex_buffer(0, self.particle_vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, instance_buffer.slice(..));
            rpass.draw(0..4, 0..self.particles.len() as u32);
        }

        self.colliders.iter()
            .for_each(|(vertices, indeces, index_count)| {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Renderer render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });
        
                rpass.set_pipeline(&self.collider_render_pipeline);
                rpass.set_vertex_buffer(0, vertices.slice(..));
                rpass.set_index_buffer(indeces.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..*index_count, 0, 0..1);   
            });
    }
}