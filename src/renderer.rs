use egui_wgpu::wgpu::{Texture, Buffer, BindGroup, TextureView, BindGroupLayout};
use pixels::{wgpu::{self, util::DeviceExt}, TextureError};
use rapier2d::{prelude::ColliderSet, na::Matrix2};

use crate::{constants::{PHYSICS_SCALE, WORLD_WIDTH, WORLD_HEIGHT}, vector::Pos2};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2], // NEW!
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
                    format: wgpu::VertexFormat::Float32x2, // NEW!
                },
            ]
        }
    }
}

pub struct MeshRenderer {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffers: Vec<wgpu::Buffer>,
    index_buffers: Vec<wgpu::Buffer>,
    sampler: wgpu::Sampler,
    index_counts: Vec<u32>,
    
    bind_buffers: Vec<wgpu::Buffer>,
    bind_indeces: Vec<wgpu::Buffer>,
    bind_group_layout: BindGroupLayout,
    bind_groups: Vec<BindGroup>,
}

impl MeshRenderer {
    pub fn new(
        pixels: &pixels::Pixels,
    ) -> Result<Self, TextureError> {
        let device = pixels.device();

        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/shader.wgsl"));

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
                    // This should match the filterable field of the
                    // corresponding Texture entry above.
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MeshRenderer pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
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
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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
        
        // let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        //     layout: &bind_group_layout,
        //     entries: &[wgpu::BindGroupEntry {
        //         binding: 0, // Match the binding in your shader
        //         resource: wgpu::BindingResource::TextureView(&texture_view),
        //     }],
        //     label: Some("Texture bind group"),
        // });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok( Self {
            render_pipeline,
            vertex_buffers: vec![],
            index_buffers: vec![],
            index_counts: vec![],
            bind_buffers: vec![],
            bind_indeces: vec![],
            bind_groups: vec![],
            bind_group_layout,
            sampler
        })
    }

    pub(crate) fn update(
        &mut self, 
        device: &wgpu::Device, 
        colliders: &ColliderSet, 
        textures: Vec<(TextureView, Pos2)>
    ) {
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
                                                -(rotated_vertex.y + collider.position().translation.y) / 4.0 * PHYSICS_SCALE + 1.0
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

        self.bind_groups = textures.iter()
            .map(|(texture, _)| {
                device.create_bind_group(
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
                )
            })
            .collect::<Vec<BindGroup>>();

        let offset = 0.5;

        // let indices: &[u16] = &[
        //     0, 1, 2,
        //     2, 1, 3
        // ];

        // self.bind_indeces = vec![
        //     device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //         label: Some("MeshRenderer index buffer"),
        //         contents: bytemuck::cast_slice(indices),
        //         usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        //     })
        // ];
            
        self.bind_buffers = textures.iter()
            .map(|(_, pos)| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("MeshRenderer index buffer"),
                    contents: bytemuck::cast_slice(&[
                        Vertex { 
                            position: [
                                (pos.x as f32 / WORLD_WIDTH as f32 - offset) * 2.0,
                                (pos.y as f32 / WORLD_HEIGHT as f32 - offset) * 2.0,
                            ], 
                            tex_coords: [0.0, 1.0] 
                        },
                        Vertex { 
                            position: [
                                ((pos.x + 1) as f32 / WORLD_WIDTH as f32 - offset) * 2.0,
                                (pos.y as f32 / WORLD_HEIGHT as f32 - offset) * 2.0,
                            ], 
                            tex_coords: [1.0, 1.0] 
                        },
                        Vertex { 
                            position: [
                                (pos.x as f32 / WORLD_WIDTH as f32 - offset) * 2.0,
                                ((pos.y + 1) as f32 / WORLD_HEIGHT as f32 - offset) * 2.0,
                            ], 
                            tex_coords: [0.0, 0.0] 
                        },
                        // Vertex { 
                        //     position: [
                        //         ((pos.x + 1) as f32 / WORLD_WIDTH as f32 - offset) * 2.0,
                        //         ((pos.y + 1) as f32 / WORLD_HEIGHT as f32 - offset) * 2.0,
                        //     ], 
                        //     tex_coords: [1.0, 0.0] 
                        // },
                    ]),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                })

            })
            .collect();
    }

    pub(crate) fn render(
        &self, 
        encoder: &mut wgpu::CommandEncoder, 
        render_target: &wgpu::TextureView
    ) {
        // for index in 0..self.index_counts.len() {
        //     let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        //         label: Some("MeshRenderer render pass"),
        //         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        //             view: render_target,
        //             resolve_target: None,
        //             ops: wgpu::Operations {
        //                 load: wgpu::LoadOp::Load,
        //                 store: true,
        //             },
        //         })],
        //         depth_stencil_attachment: None,
        //     });
    
        //     rpass.set_pipeline(&self.render_pipeline);
        //     rpass.set_vertex_buffer(0 as u32, self.vertex_buffers[index].slice(..));
        //     rpass.set_index_buffer(self.index_buffers[index].slice(..), wgpu::IndexFormat::Uint32);
        //     rpass.draw_indexed(0..self.index_counts[index], 0, 0..1);   
        // }
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("MeshRenderer render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }

        self.bind_groups.iter()
            .zip(self.bind_buffers.iter())
            .for_each(|(bind_group, bind_buffer)| {
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
                rpass.set_bind_group(0, bind_group, &[]);
                rpass.set_vertex_buffer(0, bind_buffer.slice(..));
                // rpass.set_index_buffer(self.bind_indeces[0].slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw(0..3, 0..1);
                // rpass.draw_indexed(0..6, 0, 0..1);
            });
    }
}