#![allow(dead_code)]

use std::time::Instant;

mod sim;
mod utils;
mod vector;
mod constants;
mod ui;
mod input;

use crate::sim::elements::Element;

use error_iter::ErrorIter as _;
use input::StateManager;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use sim::world::World;
use ui::Framework;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode, ElementState, MouseButton};
use winit::event_loop::EventLoop;
use winit::platform::x11::{WindowBuilderExtX11, XWindowType};
use winit::window::WindowBuilder;
use crate::constants::*;

fn main() {
    env_logger::init();
    run();
}

fn run() {
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(((CHUNK_SIZE * WORLD_SIZE) as f64 * SCALE) as i32, ((CHUNK_SIZE * WORLD_SIZE) as f64 * SCALE) as i32);
        WindowBuilder::new()
            .with_title("Rust-physics")
            .with_inner_size(size)
            .with_x11_window_type(vec![XWindowType::Dialog])
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new((CHUNK_SIZE * WORLD_SIZE) as u32, (CHUNK_SIZE * WORLD_SIZE) as u32, surface_texture).unwrap();
        let framework = Framework::new(
            &event_loop,
            window_size.width,
            window_size.height,
            scale_factor,
            &pixels,
        );

        (pixels, framework)
    };
    
    let mut world = World::new();
    let mut state_manager = StateManager::new();

    event_loop.run(move |event, _, control_flow| {        
        control_flow.set_poll();

        match &event {
            Event::WindowEvent { event, .. } => {
                if !framework.handle_event(event).consumed {
                    match event {
                        winit::event::WindowEvent::Resized( size ) => {
                            if let Err(err) = pixels.resize_surface(size.width, size.height) {
                                for source in err.sources() {
                                    error!("{source}");
                                }
                                control_flow.set_exit();
                            }
                            framework.resize(size.width, size.height);
                        },

                        winit::event::WindowEvent::CloseRequested => {
                            control_flow.set_exit();
                        },

                        winit::event::WindowEvent::KeyboardInput { input, .. } => {
                            match input.state {
                                ElementState::Released => {
                                    if let Some(keycode) = input.virtual_keycode {
                                        match keycode {
                                            VirtualKeyCode::Escape | VirtualKeyCode::Q  => {
                                                control_flow.set_exit();
                                            },
                                            VirtualKeyCode::S => {
                                                state_manager.element = Element::Sand;
                                            },
                                            VirtualKeyCode::W => {
                                                state_manager.element = Element::Water;
                                            },
                                            VirtualKeyCode::D => {
                                                state_manager.element = Element::Stone;
                                            },
                                            VirtualKeyCode::E => {
                                                state_manager.element = Element::Empty;
                                            },
                                            VirtualKeyCode::G => {
                                                state_manager.element = Element::GlowingSand;
                                            },
                                            VirtualKeyCode::Plus => {
                                                state_manager.change_brush_size(1);
                                            },
                                            VirtualKeyCode::Minus => {
                                                state_manager.change_brush_size(-1);
                                            },
                                            VirtualKeyCode::C => {
                                                // chunk.clear();
                                            }
                                            _ => {}
                                        }
                                    }
                                },
                                ElementState::Pressed => {},
                            }
                        },

                        winit::event::WindowEvent::CursorMoved { position, .. } => {
                            if state_manager.mouse_keys[0] == ElementState::Pressed {
                                if let Some(last_position) = state_manager.previous_frame.last_mouse_position {
                                    let (x1, y1) = if let Ok((x, y)) = pixels.window_pos_to_pixel((last_position.x as f32, last_position.y as f32)) {
                                        (x as i32, y as i32)
                                    }
                                    else {
                                        state_manager.previous_frame.last_mouse_position = Some(*position);
                                        return
                                    };

                                    let (x2, y2) = if let Ok((x, y)) = pixels.window_pos_to_pixel((position.x as f32, position.y as f32)) {
                                        (x as i32, y as i32)
                                    }
                                    else {
                                        state_manager.previous_frame.last_mouse_position = Some(*position);
                                        return
                                    };

                                    let dx:i32 = i32::abs(x2 - x1);
                                    let dy:i32 = i32::abs(y2 - y1);
                                    let sx:i32 = { if x1 < x2 { 1 } else { -1 } };
                                    let sy:i32 = { if y1 < y2 { 1 } else { -1 } };
                                
                                    let mut error:i32 = (if dx > dy  { dx } else { -dy }) / 2 ;
                                    let mut current_x:i32 = x1;
                                    let mut current_y:i32 = y1;

                                    loop {
                                        for x in (-state_manager.brush_size+1)..(state_manager.brush_size) {
                                            for y in (-state_manager.brush_size+1)..(state_manager.brush_size) {
                                                world.place((current_x + x) as i64, (current_y + y) as i64, state_manager.element);
                                            }
                                        }
                                
                                        if current_x == x2 && current_y == y2 { break; }
                                        let error2:i32 = error;
                                
                                        if error2 > -dx {
                                            error -= dy;
                                            current_x += sx;
                                        }
                                        if error2 < dy {
                                            error += dx;
                                            current_y += sy;
                                        }
                                    }
                                }
                            }
                            state_manager.previous_frame.last_mouse_position = Some(*position);
                        },

                        winit::event::WindowEvent::MouseInput {state, button, ..} => {
                            match *button {
                                MouseButton::Left => {
                                    state_manager.mouse_keys[0] = *state;
                                },
                                MouseButton::Right => {
                                    state_manager.mouse_keys[1] = *state;
                                },
                                MouseButton::Middle => {
                                    state_manager.mouse_keys[2] = *state;
                                },
                                MouseButton::Other(_) => {},
                            }
                        },
                        _ => {}
                    }
                }
            }

            Event::RedrawRequested(_) => {
                let now = Instant::now();
                if world.needs_update(now.duration_since(state_manager.previous_frame.instant).as_millis()) {
                    let (chunks_updated, pixels_updated) = world.update();
                    state_manager.previous_frame.update(chunks_updated, pixels_updated, now);
                }

                state_manager.previous_frame.tick();

                world.render(pixels.frame_mut());
                framework.prepare(&state_manager, &window);

                let framework_result = pixels.render_with(|encoder, render_target, context| {
                    context.scaling_renderer.render(encoder, render_target);
                    framework.render(encoder, render_target, context);

                    Ok(())
                });

                if let Err(err) = framework_result {
                    for source in err.sources() {
                        error!("{source}");
                    }
                    control_flow.set_exit();
                }
            },
            _ => {}
        }

        window.request_redraw();
    });
}