use std::time::Instant;

use ahash::HashMap;
use fps_counter::FPSCounter;
use rand::{SeedableRng, Rng};
use winit::{dpi::PhysicalPosition, event::ElementState};

use crate::{helpers::line_from_pixels, sim::{elements::Element, cell::Cell}, constants::{WORLD_HEIGHT, CHUNK_SIZE}};

pub struct Brush {
    pub element: Element,
    pub brush_type: BrushType,
    pub shape: BrushShape,
    pub size: i32, 
}

#[derive(PartialEq)]
pub enum BrushType {
    Cell,
    Object,
    StaticObject,
    Particle(u8)
}

#[derive(PartialEq)]
pub enum BrushShape {
    Circle,
    Square,
}

pub struct InputManager {
    pub brush: Brush,

    pub mouse: Mouse,
    pub previous_frame: Frame,
    pub placing_queue: HashMap<(i32, i32), Cell>,
}

pub struct Mouse {
    pub mouse_keys: [ElementState; 3],
    pub last_mouse_position: Option<PhysicalPosition<f64>>,
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
            brush: Brush { 
                element: Element::default(), 
                brush_type: BrushType::Cell, 
                shape: BrushShape::Circle, 
                size: 10
            },
            placing_queue: HashMap::default(),
            mouse: Mouse { 
                mouse_keys: [ElementState::Released; 3],
                last_mouse_position: None,
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

    // pub fn change_brush_size(&mut self, size_delta: i32) {
    //     if size_delta > 0 {
    //         self.mouse.brush_size = (self.mouse.brush_size + size_delta).min(15);
    //     }
    //     else {
    //         self.mouse.brush_size = (self.mouse.brush_size + size_delta).max(2);
    //     }
    // }

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
                let mut rng = rand::rngs::SmallRng::from_entropy();

                let mut function = |x: i32, y: i32| {
                    match self.brush.shape {
                        BrushShape::Circle => {
                            for dx in -self.brush.size..=self.brush.size {
                                for dy in -self.brush.size..=self.brush.size {
                                    if (dx).pow(2) + (dy).pow(2) <= self.brush.size.pow(2) {
                                        match self.brush.brush_type {
                                            BrushType::Particle(rate) => {
                                                if rng.gen_range(0..255) <= rate {
                                                    self.placing_queue.insert((x + dx, y + dy), Cell::new(&self.brush.element, 0));
                                                }
                                            },
                                            _ => {
                                                self.placing_queue.insert((x + dx, y + dy), Cell::new(&self.brush.element, 0));
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        BrushShape::Square => {
                            for dx in -self.brush.size..=self.brush.size {
                                for dy in -self.brush.size..=self.brush.size {
                                    match self.brush.brush_type {
                                        BrushType::Particle(rate) => {
                                            if rng.gen_range(0..255) <= rate {
                                                self.placing_queue.insert((x + dx, y + dy), Cell::new(&self.brush.element, 0));
                                            }
                                        },
                                        _ => {
                                            self.placing_queue.insert((x + dx, y + dy), Cell::new(&self.brush.element, 0));
                                        }
                                    }
                                    
                                }
                            }
                        },
                    }
                    true
                };
                
                let (x1, y1) = if let Ok((x, y)) = pixels.window_pos_to_pixel((last_position.x as f32, last_position.y as f32)) {
                    (x as i32, WORLD_HEIGHT * CHUNK_SIZE - y as i32)
                }
                else {
                    self.mouse.last_mouse_position = Some(*position);
                    return
                };

                let (x2, y2) = if let Ok((x, y)) = pixels.window_pos_to_pixel((position.x as f32, position.y as f32)) {
                    (x as i32, WORLD_HEIGHT * CHUNK_SIZE - y as i32)
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
        self.previous_frame.pixels_updated = 0;
        self.previous_frame.chunks_updated = 0;
        self.previous_frame.fps = self.previous_frame.fps_counter.tick();
    }

    pub fn update_frame_info(&mut self, chunks_updated: u128, pixels_updated: u128, instant: Instant) {
        self.previous_frame.pixels_updated += pixels_updated;
        self.previous_frame.chunks_updated += chunks_updated;
        self.previous_frame.instant = instant;
    }
}