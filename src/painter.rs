use ahash::{HashMap, HashSet};

use crate::{sim::{elements::Element, cell::Cell}, helpers::line_from_pixels};

pub struct Painter {
    pub brush: Brush,
    pub placing_queue: HashSet<(i32, i32)>,
    active: bool,
}

impl Painter {
    pub fn new() -> Self {
        Self { 
            brush: Brush::new(), 
            placing_queue: HashSet::default(),
            active: false,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn drain_placing_queue(&mut self) -> Vec<(i32, i32)> {
        self.placing_queue.drain().collect()
    }

    pub fn is_cells_queued(&mut self) -> bool {
        !self.placing_queue.is_empty()
    }
    
    pub fn draw_point(&mut self, x: i32, y: i32) {
        if self.active {
            let mut draw_operation = |x: i32, y: i32| {
                match self.brush.brush_type {
                    BrushType::Particle(rate) => {
                        if fastrand::u8(0..255) <= rate {
                            self.placing_queue.insert((x, y));
                        }
                    },
                    _ => {
                        self.placing_queue.insert((x, y));
                    }
                }
            };
    
            self.brush.shape.draw(x, y, self.brush.size, &mut draw_operation);
        }
    }

    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        if self.active {
            let mut draw_operation = |x: i32, y: i32| {
                match self.brush.brush_type {
                    BrushType::Particle(rate) => {
                        if fastrand::u8(0..255) <= rate {
                            self.placing_queue.insert((x, y));
                        }
                    },
                    BrushType::ObjectEraser => {},
                    _ => {
                        self.placing_queue.insert((x, y));
                    }
                }
            };
    
            let mut function = |x: i32, y: i32| {
                self.brush.shape.draw(x, y, self.brush.size, &mut draw_operation);
                true
            };
    
            line_from_pixels(x1, y1, x2 + x2.signum(), y2 + y2.signum(), &mut function);
        }

    }
}

#[derive(Clone)]
pub struct Brush {
    pub element: Element,
    pub brush_type: BrushType,
    pub shape: BrushShape,
    pub size: i32, 
}

#[derive(Clone, PartialEq)]
pub enum BrushType {
    Cell,
    Object,
    StaticObject,
    Particle(u8),
    Force(f32),
    ObjectEraser,
}

#[derive(Clone, PartialEq)]
pub enum BrushShape {
    Circle,
    Square,
}

impl BrushShape {
    pub fn draw<F: FnMut(i32, i32)> (
        &self,
        x: i32, 
        y: i32, 
        size: i32,
        operation: &mut F
    ) {
        match self {
            BrushShape::Circle => {
                for dx in -size..=size {
                    for dy in -size..=size {
                        if (dx).pow(2) + (dy).pow(2) > size.pow(2) {
                            continue;
                        }

                        operation(x + dx, y + dy);
                    }
                }
            },
            BrushShape::Square => {
                for dx in -size..=size {
                    for dy in -size..=size {
                        operation(x + dx, y + dy);
                    }
                }
            },
        }
    }    
}

impl Brush {
    pub fn new() -> Self {
        Self {
            element: Element::default(), 
            brush_type: BrushType::Cell, 
            shape: BrushShape::Circle, 
            size: 10,
        }
    }
}

