#![allow(dead_code)]

use std::time::Instant;

mod cell;
mod chunk;
mod elements;
mod utils;
mod constants;

use chunk::Chunk;
use elements::Element;

use error_iter::ErrorIter as _;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::{PhysicalPosition, LogicalSize};
use winit::event::{Event, VirtualKeyCode, ElementState, MouseButton};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::platform::x11::{WindowBuilderExtX11, XWindowType};
use winit::window::{WindowBuilder, Window};

const SIZE: u32 = 100;
const DELAY: u128 = 10;

fn screen_to_world(x: f32, y:f32,  window: &Window) -> (i32, i32) {
    ((x as f32 / window.inner_size().width as f32 * SIZE as f32).round() as i32, (y as f32 / window.inner_size().height as f32 * SIZE as f32).round() as i32)
}

struct InputState {
    mouse_keys_held: [ElementState; 3],
    last_mouse_position: Option<PhysicalPosition<f64>>,
    time_instant: Instant,
    selected_element: Element,
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(SIZE * 3, SIZE * 3);
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

    let mut chunk = Chunk::new(SIZE, SIZE);

    for i in 0..SIZE {
        chunk.place(i as i32, 0, Element::Stone);
        chunk.place(i as i32, (SIZE-1) as i32, Element::Stone);
        chunk.place(0 as i32, i as i32, Element::Stone);
        chunk.place((SIZE-1) as i32 as i32, i as i32, Element::Stone);
    }

    let mut input_state = InputState{
        mouse_keys_held: [ElementState::Released; 3],
        last_mouse_position: None,
        time_instant: Instant::now(),
        selected_element: Element::Sand,
    };

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        
        match &event {
            Event::NewEvents(_) => {
                let now = Instant::now();
                if now.duration_since(input_state.time_instant).as_millis() > DELAY {
                    chunk.update(now.duration_since(input_state.time_instant).as_secs_f32());
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
                    match input.virtual_keycode.unwrap() {
                        VirtualKeyCode::Escape | VirtualKeyCode::Q  => {
                            control_flow.set_exit();
                        }
                        VirtualKeyCode::S => {
                            input_state.selected_element = Element::Sand;
                        }
                        VirtualKeyCode::W => {
                            input_state.selected_element = Element::Water;
                        }
                        VirtualKeyCode::E => {
                            input_state.selected_element = Element::Empty;
                        }
                        VirtualKeyCode::C => {
                            chunk.clear();
                        }
                        _ => {}
                    }
                },
                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    if input_state.mouse_keys_held[0] == ElementState::Pressed {
                        if let Some(last_position) = input_state.last_mouse_position {
                            let (x1, y1) = screen_to_world(last_position.x as f32, last_position.y as f32, &window);
                            let (x2, y2) = screen_to_world(position.x as f32, position.y as f32, &window);
                
                            let dx:i32 = i32::abs(x2 - x1);
                            let dy:i32 = i32::abs(y2 - y1);
                            let sx:i32 = { if x1 < x2 { 1 } else { -1 } };
                            let sy:i32 = { if y1 < y2 { 1 } else { -1 } };
                        
                            let mut error:i32 = (if dx > dy  { dx } else { -dy }) / 2 ;
                            let mut current_x:i32 = x1;
                            let mut current_y:i32 = y1;

                            loop {
                                for x in -1..2 {
                                    for y in -1..2 {
                                chunk.place(current_x + x as i32, current_y + y as i32, input_state.selected_element);
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
                chunk.draw(pixels.frame_mut());
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