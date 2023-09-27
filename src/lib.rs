#![allow(dead_code)]

use std::thread;
use std::time::{Instant, Duration};

mod sim;
mod utils;
mod vector;
mod constants;
mod ui;
mod input;
mod renderer;
mod helpers;

use crate::sim::elements::MatterType;

use input::InputManager;
use log::error;
use parking_lot::deadlock;
use pixels::{Pixels, SurfaceTexture};
use renderer::MeshRenderer;
use sim::elements::Element;
use sim::world::{World, WorldApi};
use ui::Framework;
use winit::dpi::LogicalSize;
use winit::event::{Event, ElementState};
use winit::event_loop::EventLoop;
use winit::platform::x11::{WindowBuilderExtX11, XWindowType};
use winit::window::WindowBuilder;
use crate::constants::*;

pub fn run() {
    let event_loop = EventLoop::new();

    let window = {
        let size = LogicalSize::new(((WORLD_WIDTH * CHUNK_SIZE) as f32 * SCALE) as i32, ((WORLD_HEIGHT * CHUNK_SIZE) as f32 * SCALE) as i32);
        WindowBuilder::new()
            .with_title("Rust-physics")
            .with_inner_size(size)
            .with_x11_window_type(vec![XWindowType::Dialog])
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let (mut pixels, mut interface) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new((WORLD_WIDTH * CHUNK_SIZE) as u32, (WORLD_HEIGHT * CHUNK_SIZE) as u32, surface_texture).unwrap();
        let interface = Framework::new(
            &event_loop,
            window_size.width,
            window_size.height,
            scale_factor,
            &pixels,
        );

        (pixels, interface)
    };

    let mut world = World::new();
    let mut input_manager = InputManager::new();

    let mut mesh_renderer = MeshRenderer::new(&pixels).unwrap();

    event_loop.run(move |event, _, control_flow| {        
        control_flow.set_poll();

        match &event {
            Event::WindowEvent { event, .. } => {
                if !interface.handle_event(event).consumed {
                    match event {
                        winit::event::WindowEvent::Resized( size ) => {
                            if let Err(err) = pixels.resize_surface(size.width, size.height) {
                                error!("{}", err.to_string());
                                control_flow.set_exit();
                            }
                            interface.resize(size.width, size.height);
                        },

                        winit::event::WindowEvent::CloseRequested => {
                            control_flow.set_exit();
                        },

                        winit::event::WindowEvent::KeyboardInput { input, .. } => {
                            input_manager.handle_keyboard_input(control_flow, input);
                        },

                        winit::event::WindowEvent::CursorMoved { position, .. } => {
                            input_manager.handle_mouse_movement(&pixels, position);
                        },

                        winit::event::WindowEvent::MouseInput {state, button, ..} => {
                            input_manager.handle_mouse_buttons(control_flow, state, button);
                        },
                        _ => {}
                    }
                }
            },

            Event::MainEventsCleared => {
                let now = Instant::now();

                if now.duration_since(input_manager.previous_frame.instant).as_millis() > 30 {
                    if world.needs_update(now.duration_since(input_manager.previous_frame.instant).as_millis()) {
                        if input_manager.brush.element.matter == MatterType::Empty {
                            world.place_batch(
                                input_manager.placing_queue.drain().collect(),
                            );
                        }
                        else {
                            match input_manager.brush.brush_type {
                                input::BrushType::Cell => {
                                    if !input_manager.placing_queue.is_empty() {
                                        world.place_batch(
                                            input_manager.placing_queue.drain().collect(),
                                        );
                                    }
                                },
                                input::BrushType::Object => {
                                    if !input_manager.placing_queue.is_empty() && input_manager.mouse.mouse_keys[0] == ElementState::Released {
                                        world.place_object(
                                            input_manager.placing_queue.drain().collect(),
                                            false
                                        );
                                    }
                                },
                                input::BrushType::StaticObject => {
                                    if !input_manager.placing_queue.is_empty() && input_manager.mouse.mouse_keys[0] == ElementState::Released {
                                        world.place_object(
                                            input_manager.placing_queue.drain().collect(),
                                            true
                                        );
                                    }
                                },
                                input::BrushType::Particle(_) => {
                                    if !input_manager.placing_queue.is_empty() {
                                        world.place_particles(
                                            input_manager.placing_queue.drain().collect(),
                                        );
                                    }
                                },
                            }          
                        }

                        let (chunks_updated, pixels_updated) = world.update();
                        input_manager.update_frame_info(chunks_updated, pixels_updated, now);

                        world.render(&mut pixels, &mut mesh_renderer);    
                    }
                    
                    window.request_redraw();
                }

            },
            
            Event::RedrawRequested(_) => {
                input_manager.next_frame();
                
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    // context.scaling_renderer.render(encoder, render_target);
    
                    mesh_renderer.render(encoder, render_target);

                    interface.prepare(&mut input_manager, &window);
                    interface.render(encoder, render_target, context);

                    Ok(())
                });

                if let Err(err) = render_result {
                    error!("{}", err.to_string());
                    control_flow.set_exit();
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

pub fn bench_init() -> WorldApi {
    World::new()
}

pub fn bench_fill(world: &mut WorldApi) {
    let element = Element {
        name: "Bench".to_string(), 
        color: [0, 0, 0, 0], 
        color_offset: 0,
        matter: MatterType::Powder,
    };

    for x in 0..(WORLD_WIDTH * CHUNK_SIZE) {
        for y in 0..CHUNK_SIZE {
            world.place(x, y, &element);
        }
    }

    for x in 0..(WORLD_WIDTH * CHUNK_SIZE) {
        for y in (WORLD_HEIGHT / 2 * CHUNK_SIZE)..((WORLD_HEIGHT / 2 + 1) * CHUNK_SIZE) {
            world.place(x, y, &element);
        }
    }
}

pub fn bench_until_empty(world: &mut WorldApi) {
    loop {
        
        let (_, pixels_count) = world.update_iteration();
        if pixels_count == 0 {
            return ;
        }
    };
}

pub fn bench_number_of_updated(world: &mut WorldApi, n: usize) {
    for _ in 0..n {
        world.update_iteration();
    }
}