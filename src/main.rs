#![allow(dead_code)]

use std::time::Instant;

mod cell;
mod chunk;
mod elements;
mod utils;
mod constants;

use chunk::ChunkManager;
use elements::Element;

use error_iter::ErrorIter as _;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::{PhysicalPosition, LogicalSize};
use winit::event::{Event, VirtualKeyCode, ElementState, MouseButton};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::x11::{WindowBuilderExtX11, XWindowType};
use winit::window::WindowBuilder;

const CHUNK_SIZE: u32 = 120;
const WORLD_SIZE: u32 = 2;
const SIZE: u32 = CHUNK_SIZE * WORLD_SIZE;
const DELAY: u128 = 1;
const PRINT_DELAY: bool = true;

struct InputState {
    mouse_keys_held: [ElementState; 3],
    last_mouse_position: Option<PhysicalPosition<f64>>,
    time_instant: Instant,
    selected_element: Element,
    brush_size: i32,
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(800, 800);
        WindowBuilder::new()
            .with_title("Rust-physics")
            .with_inner_size(size)
            .with_x11_window_type(vec![XWindowType::Dialog])
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(SIZE, SIZE, surface_texture).unwrap()
    };

    let mut chunk_manager = ChunkManager::new(CHUNK_SIZE as i64, WORLD_SIZE as i64);

    for x in 0..SIZE {
        chunk_manager.place(x as i64, 0, Element::Stone);
        chunk_manager.place(x as i64, (SIZE - 1) as i64, Element::Stone);
    }

    for y in 0..SIZE {
        chunk_manager.place(0, y as i64, Element::Stone);
        chunk_manager.place((SIZE - 1) as i64, y as i64, Element::Stone);
    }
    let mut input_state = InputState{
        mouse_keys_held: [ElementState::Released; 3],
        last_mouse_position: None,
        time_instant: Instant::now(),
        selected_element: Element::Sand,
        brush_size: 1,
    };

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        
        match &event {
            Event::NewEvents(_) => {
                let now = Instant::now();
                if now.duration_since(input_state.time_instant).as_millis() > DELAY {
                    if PRINT_DELAY {
                        dbg!(now.duration_since(input_state.time_instant));
                    }
                    chunk_manager.update(now.duration_since(input_state.time_instant).as_secs_f32());
                    input_state.time_instant = now;
                }
            }

            Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::Resized( size ) => {
                    if let Err(err) = pixels.resize_surface(size.width, size.height) {
                        log_error("pixels.resize_surface", err);
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                },
                winit::event::WindowEvent::CloseRequested => {
                    control_flow.set_exit();
                    return;
                },
                winit::event::WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(keycode) = input.virtual_keycode {
                        match keycode {
                            VirtualKeyCode::Escape | VirtualKeyCode::Q  => {
                                control_flow.set_exit();
                            },
                            VirtualKeyCode::S => {
                                input_state.selected_element = Element::Sand;
                            },
                            VirtualKeyCode::W => {
                                input_state.selected_element = Element::Water;
                            },
                            VirtualKeyCode::D => {
                                input_state.selected_element = Element::Stone;
                            },
                            VirtualKeyCode::E => {
                                input_state.selected_element = Element::Empty;
                            },
                            VirtualKeyCode::G => {
                                input_state.selected_element = Element::GlowingSand;
                            },
                            VirtualKeyCode::Plus => {
                                input_state.brush_size = (input_state.brush_size + 1).min(9);
                            },
                            VirtualKeyCode::Minus => {
                                input_state.brush_size = (input_state.brush_size - 1).max(1);
                            },
                            VirtualKeyCode::C => {
                                // chunk.clear();
                            }
                            _ => {}
                        }
                    }
                },
                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    if input_state.mouse_keys_held[0] == ElementState::Pressed {
                        if let Some(last_position) = input_state.last_mouse_position {
                            let (x1, y1) = {
                                let (x, y) = pixels.window_pos_to_pixel((last_position.x as f32, last_position.y as f32)).unwrap();
                                (x as i32, y as i32)
                            };
                            let (x2, y2) = {
                                let (x, y) = pixels.window_pos_to_pixel((position.x as f32, position.y as f32)).unwrap();
                                (x as i32, y as i32)
                            };

                            let dx:i32 = i32::abs(x2 - x1);
                            let dy:i32 = i32::abs(y2 - y1);
                            let sx:i32 = { if x1 < x2 { 1 } else { -1 } };
                            let sy:i32 = { if y1 < y2 { 1 } else { -1 } };
                        
                            let mut error:i32 = (if dx > dy  { dx } else { -dy }) / 2 ;
                            let mut current_x:i32 = x1;
                            let mut current_y:i32 = y1;

                            loop {
                                for x in (-input_state.brush_size+1)..(input_state.brush_size) {
                                    for y in (-input_state.brush_size+1)..(input_state.brush_size) {
                                        chunk_manager.place((current_x + x) as i64, (current_y + y) as i64, input_state.selected_element);
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
                    input_state.last_mouse_position = Some(*position);
                },
                winit::event::WindowEvent::MouseInput {state, button, ..} => {
                    match *button {
                        MouseButton::Left => {
                            input_state.mouse_keys_held[0] = *state;
                        },
                        MouseButton::Right => {
                            input_state.mouse_keys_held[1] = *state;
                        },
                        MouseButton::Middle => {
                            input_state.mouse_keys_held[2] = *state;
                        },
                        MouseButton::Other(_) => {},
                    }
                },
                _ => {}
            }

            Event::RedrawRequested(_) => {
                chunk_manager.render(pixels.frame_mut());
                if let Err(err) = pixels.render() {
                    log_error("pixels.render", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            },
            _ => {}
        }

        window.request_redraw();
    })

}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    for source in err.sources().skip(1) {
        error!("  Caused by: {source}");
    }
}