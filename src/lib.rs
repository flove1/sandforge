#![allow(dead_code)]

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use std::thread;
use std::time::Duration;

mod sim;
mod utils;
mod vector;
mod constants;
mod gui;
mod helpers;

use crate::sim::elements::MatterType;

use parking_lot::deadlock;
use sim::elements::Element;
use sim::world::World;
use gui::Gui;
use winit::dpi::LogicalSize;
use winit::event::Event;
use winit::event_loop::EventLoop;
use winit::window::{WindowBuilder, Window};
use crate::constants::*;

pub struct State {
    pub surface: wgpu::Surface,
    pub config: wgpu::SurfaceConfiguration,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl State {
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();
    
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
    
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
    
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                None, // Trace path
            )
            .await
            .unwrap();
    
        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);
        
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
    
        Self {
            surface,
            config,
            device,
            queue,
        }
    }

    pub fn render_with<F>(&self, render_function: F) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>
    where
        F: FnOnce(
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            &wgpu::Device,
            &wgpu::Queue,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>,
    {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Renderer render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }
        
        (render_function)(&mut encoder, &view, &self.device, &self.queue)?;

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}


pub async fn run() {
    let event_loop = EventLoop::new();

    let window = {
        let size = LogicalSize::new(SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);

        // #[cfg(target_os = "unix")]

        if cfg!(windows) {
            WindowBuilder::new()
                .with_title("Rust-physics")
                .with_inner_size(size)
                .with_min_inner_size(size)
                .with_max_inner_size(size)
                .build(&event_loop)
                .unwrap()
        }
        else {
            use winit::platform::x11::{WindowBuilderExtX11, XWindowType};

            WindowBuilder::new()
                .with_title("Rust-physics")
                .with_inner_size(size)
                .with_x11_window_type(vec![XWindowType::Dialog])
                .with_min_inner_size(size)
                .with_max_inner_size(size)
                .build(&event_loop)
                .unwrap()   
        }
    };

    let state = State::new(&window).await;

    let mut world = World::new(&state.device, &state.config.format);

    let mut gui = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        
        Gui::new(
            &event_loop,
            window_size.width,
            window_size.height,
            scale_factor,
            &state.device,
            &state.config.format
        )
    };

    event_loop.run(move |event, _, control_flow| {        
        control_flow.set_poll();

        match &event {
            Event::WindowEvent { event, .. } => {
                if !gui.handle_event(event, control_flow, window.scale_factor()).consumed {
                    match event {
                        winit::event::WindowEvent::Resized( size ) => {
                            gui.resize(size.width, size.height);
                        },

                        winit::event::WindowEvent::CloseRequested => {
                            control_flow.set_exit();
                        },

                        _ => {}
                    }
                }
            },

            Event::MainEventsCleared => {

                if gui.is_update_required() {
                    if world.needs_update(gui.ms_from_previous_update()) {
                        if gui.get_brush().element.matter == MatterType::Empty {
                            world.place_batch(gui.drain_placing_queue());
                        }
                        else {
                            match gui.get_brush().brush_type {
                                gui::BrushType::Cell => {
                                    world.place_batch(gui.drain_placing_queue());
                                },
                                gui::BrushType::Object => {
                                    if gui.is_cells_queued() && gui.is_key_released(0) {
                                        world.place_object(
                                            gui.drain_placing_queue(),
                                            false,
                                            &state.device,
                                            &state.queue
                                        );
                                    }
                                },
                                gui::BrushType::StaticObject => {
                                    if gui.is_cells_queued() && gui.is_key_released(0) {
                                        world.place_object(
                                            gui.drain_placing_queue(),
                                            true,
                                            &state.device,
                                            &state.queue
                                        );
                                    }
                                },
                                gui::BrushType::Particle(_) => {
                                    if gui.is_cells_queued() {world.place_particles(gui.drain_placing_queue());}
                                },
                            }          
                        }

                        let (chunks_updated, pixels_updated) = world.update();
                        gui.update_frame_info(chunks_updated, pixels_updated);

                        world.update_textures(&state.device, &state.queue);    
                    }
                    
                    window.request_redraw();
                }
            },
            
            Event::RedrawRequested(_) => {
                gui.next_frame();

                if let Ok(_) = state.render_with(|encoder, view, device, queue| {
                    world.render(encoder, view);

                    gui.prepare(&window);
                    gui.render(encoder, view, device, queue);

                    Ok(())
                }) {

                }

            },
            _ => {}
        }
    });
}

pub fn deadlock_checker() {
    loop {
        thread::sleep(Duration::from_secs(10));
        let deadlocks = deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        println!("{} deadlocks detected", deadlocks.len());
        for (i, threads) in deadlocks.iter().enumerate() {
            println!("Deadlock #{}", i);
            for t in threads {
                println!("Thread Id {:#?}", t.thread_id());
                println!("{:#?}", t.backtrace());
            }
        }
    }
}

// pub fn bench_init() -> WorldApi {
//     World::new()
// }

pub fn bench_fill(world: &mut World) {
    let element = Element {
        name: "Bench".to_string(), 
        color: [0, 0, 0, 0], 
        color_offset: 0,
        matter: MatterType::Powder,
    };

    for x in 0..(WORLD_WIDTH * CHUNK_SIZE) {
        for y in 0..CHUNK_SIZE {
            world.set_cell_by_pixel(x, y, &element);
        }
    }

    for x in 0..(WORLD_WIDTH * CHUNK_SIZE) {
        for y in (WORLD_HEIGHT / 2 * CHUNK_SIZE)..((WORLD_HEIGHT / 2 + 1) * CHUNK_SIZE) {
            world.set_cell_by_pixel(x, y, &element);
        }
    }
}

pub fn bench_until_empty(world: &mut World) {
    loop {
        world.forced_update();
    };
}

pub fn bench_number_of_updated(world: &mut World, n: usize) {
    for _ in 0..n {
        world.forced_update();
    }
}