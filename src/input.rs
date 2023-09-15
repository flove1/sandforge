use std::time::Instant;

use ahash::HashSet;
use fps_counter::FPSCounter;
use winit::{dpi::PhysicalPosition, event::ElementState};

use crate::{sim::elements::Element, helpers::line_from_pixels};

pub struct InputManager {
    pub element: Element,
    pub draw_object: bool,
    pub draw_static_object: bool,
    pub render_objects: bool,

    pub mouse: Mouse,
    pub previous_frame: Frame,
    pub placing_queue: HashSet<(i32, i32)>,
}

pub struct Mouse {
    pub mouse_keys: [ElementState; 3],
    pub last_mouse_position: Option<PhysicalPosition<f64>>,
    pub brush_size: i32,
}

pub struct Frame {
    pub instant: Instant,
    pub pixels_updated: u128,
    pub chunks_updated: u128,
    pub fps: usize,
    fps_counter: FPSCounter,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            element: Element::Sand,
            draw_object: false,
            draw_static_object: false,
            render_objects: true,
            placing_queue: HashSet::default(),
            mouse: Mouse { 
                mouse_keys: [ElementState::Released; 3],
                last_mouse_position: None,
                brush_size: 2
            },
            previous_frame: Frame {
                instant: Instant::now(),
                pixels_updated: 0,
                chunks_updated: 0,
                fps_counter: FPSCounter::new(),
                fps: 0,
            },
        }
    }

    pub fn change_brush_size(&mut self, size_delta: i32) {
        if size_delta > 0 {
            self.mouse.brush_size = (self.mouse.brush_size + size_delta).min(15);
        }
        else {
            self.mouse.brush_size = (self.mouse.brush_size + size_delta).max(2);
        }
    }

    #[allow(unused)]
    pub fn handle_keyboard_input(&mut self, control_flow: &mut winit::event_loop::ControlFlow, input: &winit::event::KeyboardInput) {
        if let ElementState::Released = input.state {
            if let Some(keycode) = input.virtual_keycode {
                match keycode {
                    winit::event::VirtualKeyCode::Escape | winit::event::VirtualKeyCode::Q  => {
                        control_flow.set_exit();
                    },
                    _ => {}
                }
            }
        }
    }

    #[allow(unused)]
    pub fn handle_mouse_buttons(&mut self, control_flow: &mut winit::event_loop::ControlFlow, state: &winit::event::ElementState, button: &winit::event::MouseButton) {
        match *button {
            winit::event::MouseButton::Left => {
                self.mouse.mouse_keys[0] = *state;
            },
            winit::event::MouseButton::Right => {
                self.mouse.mouse_keys[1] = *state;
            },
            winit::event::MouseButton::Middle => {
                self.mouse.mouse_keys[2] = *state;
            },
            winit::event::MouseButton::Other(_) => {},
        }
    }

    #[allow(unused)]
    pub fn handle_mouse_movement(&mut self, pixels: &pixels::Pixels, position: &winit::dpi::PhysicalPosition<f64>) {
        if self.mouse.mouse_keys[0] == ElementState::Pressed {
            if let Some(last_position) = self.mouse.last_mouse_position {
                let mut function = |x, y| {
                    for dx in (-self.mouse.brush_size+1)..(self.mouse.brush_size) {
                        for dy in (-self.mouse.brush_size+1)..(self.mouse.brush_size) {
                            self.placing_queue.insert((x + dx, y + dy));
                        }
                    }
                };
                
                let (x1, y1) = if let Ok((x, y)) = pixels.window_pos_to_pixel((last_position.x as f32, last_position.y as f32)) {
                    (x as i32, y as i32)
                }
                else {
                    self.mouse.last_mouse_position = Some(*position);
                    return
                };

                let (x2, y2) = if let Ok((x, y)) = pixels.window_pos_to_pixel((position.x as f32, position.y as f32)) {
                    (x as i32, y as i32)
                }
                else {
                    self.mouse.last_mouse_position = Some(*position);
                    return
                };

                line_from_pixels(x1, y1, x2, y2, &mut function);
            }
        }
        self.mouse.last_mouse_position = Some(*position);
    }

    pub fn next_frame(&mut self) {
        self.previous_frame.fps = self.previous_frame.fps_counter.tick();
    }

    pub fn update_frame_info(&mut self, chunks_updated: u128, pixels_updated: u128, instant: Instant) {
        self.previous_frame.pixels_updated = pixels_updated;
        self.previous_frame.chunks_updated = chunks_updated;
        self.previous_frame.instant = instant;
    }
}