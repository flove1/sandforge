use std::time::Instant;

use fps_counter::FPSCounter;
use winit::{dpi::PhysicalPosition, event::ElementState};

use crate::sim::elements::Element;

pub struct StateManager {
    pub mouse_keys: [ElementState; 3],
    pub element: Element,
    pub brush_size: i32,
    pub previous_frame: Frame,
}

pub struct Frame {
    pub last_mouse_position: Option<PhysicalPosition<f64>>,
    pub instant: Instant,
    pub pixels_updated: u128,
    pub chunks_updated: u128,
    pub fps: usize,
    fps_counter: FPSCounter,
}

impl StateManager {
    pub fn new() -> Self {
        Self {
            mouse_keys: [ElementState::Released; 3],
            element: Element::Sand,
            brush_size: 1,
            previous_frame: Frame {
                last_mouse_position: None,
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
            self.brush_size = (self.brush_size + size_delta).min(15);
        }
        else {
            self.brush_size = (self.brush_size + size_delta).max(1);
        }
    }

}

impl Frame {
    pub fn update(&mut self, chunks_updated: u128, pixels_updated: u128, instant: Instant) {
        self.pixels_updated = pixels_updated;
        self.chunks_updated = chunks_updated;
        self.instant = instant;
    }
    
    pub fn tick(&mut self) {
        self.fps = self.fps_counter.tick();
    }
}