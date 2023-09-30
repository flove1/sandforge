use std::time::Instant;

use ahash::HashMap;
use egui::Context;
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use egui_winit::{EventResponse, egui};
use fps_counter::FPSCounter;
use winit::dpi::LogicalPosition;
use winit::event::ElementState;
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

use crate::constants::{SCREEN_HEIGHT, SCREEN_WIDTH, SCALE, TARGET_FPS};
use crate::helpers::line_from_pixels;
use crate::sim::cell::Cell;
use crate::sim::elements::{ELEMENTS, Element};

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
    Particle(u8)
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

pub struct MouseInput {
    pub brush: Brush,
    pub mouse_keys: [ElementState; 3],
    pub last_mouse_position: Option<LogicalPosition<f64>>,
    pub placing_queue: HashMap<(i32, i32), Cell>,
}

pub struct Frame {
    pub instant: Instant,
    pub pixels_updated: u128,
    pub chunks_updated: u128,
    pub fps: usize,
    fps_counter: FPSCounter,
}

impl MouseInput {
    pub fn new() -> Self {
        Self {
            brush: Brush { 
                element: Element::default(), 
                brush_type: BrushType::Cell, 
                shape: BrushShape::Circle, 
                size: 10
            },

            placing_queue: HashMap::default(),
            mouse_keys: [ElementState::Released; 3],
            last_mouse_position: None,
        }
    }

    #[allow(unused)]
    pub fn handle_mouse_buttons(&mut self, control_flow: &mut winit::event_loop::ControlFlow, state: &winit::event::ElementState, button: &winit::event::MouseButton) {
        match *button {
            winit::event::MouseButton::Left => {
                self.mouse_keys[0] = *state;
            },
            winit::event::MouseButton::Right => {
                self.mouse_keys[1] = *state;
            },
            winit::event::MouseButton::Middle => {
                self.mouse_keys[2] = *state;
            },
            winit::event::MouseButton::Other(_) => {},
        }
    }

    #[allow(unused)]
    pub fn handle_mouse_movement(&mut self, position: &winit::dpi::PhysicalPosition<f64>, scale: f64) {
        if self.mouse_keys[0] == ElementState::Pressed {
            if let Some(last_position) = self.last_mouse_position {
                let mut draw_operation = |x: i32, y: i32| {
                    match self.brush.brush_type {
                        BrushType::Particle(rate) => {
                            if fastrand::u8(0..255) <= rate {
                                self.placing_queue.insert((x, y), Cell::new(&self.brush.element, 0));
                            }
                        },
                        _ => {
                            self.placing_queue.insert((x, y), Cell::new(&self.brush.element, 0));
                        }
                    }
                };

                let mut function = |x: i32, y: i32| {
                    self.brush.shape.draw(x, y, self.brush.size, &mut draw_operation);
                    true
                };

                let logical_position = position.to_logical::<f64>(scale);
                
                let (x1, y1) = if last_position.x < SCREEN_WIDTH as f64 && last_position.y < SCREEN_HEIGHT as f64 {
                    ((last_position.x as f32 / SCALE).round() as i32, ((SCREEN_HEIGHT - last_position.y as f32) / SCALE).round() as i32)
                }
                else {
                    self.last_mouse_position = Some(logical_position);
                    return
                };
                
                let (x2, y2) = if logical_position.x < SCREEN_WIDTH as f64 && logical_position.y < SCREEN_HEIGHT as f64 {
                    ((logical_position.x as f32 / SCALE).round() as i32, ((SCREEN_HEIGHT - logical_position.y as f32) / SCALE).round() as i32)
                }
                else {
                    self.last_mouse_position = Some(logical_position);
                    return
                };

                line_from_pixels(x1, y1, x2, y2, &mut function);
            }
        }
        self.last_mouse_position = Some(position.to_logical::<f64>(scale));
    }
}

pub struct Gui {
    egui_ctx: egui_winit::egui::Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    paint_jobs: Vec<epaint::ClippedPrimitive>,
    textures: epaint::textures::TexturesDelta,

    mouse_input: MouseInput,
    interface: Interface,
    frame_info: Frame,
}

struct Interface {
    menu_bar_open: bool,
    info_open: bool,
    elements_open: bool,
}

impl Gui {
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>, 
        width: u32, 
        height: u32, 
        scale_factor: f32, 
        device: &wgpu::Device,
        format: &wgpu::TextureFormat
    ) -> Self {
        let max_texture_size = device.limits().max_texture_dimension_2d as usize;

        let egui_ctx = Context::default();

        let mut fonts = egui::FontDefinitions::default();

        fonts.font_data.insert(
            "pixel font".to_owned(),
            egui::FontData::from_static(include_bytes!(
                "../assets/PeaberryBase.ttf"
            )),
        );

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "pixel font".to_owned());
        
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("pixel font".to_owned());

        egui_ctx.set_fonts(fonts);

        let mut egui_state = egui_winit::State::new(event_loop);
        egui_state.set_max_texture_side(max_texture_size);
        egui_state.set_pixels_per_point(scale_factor);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };

        let renderer = Renderer::new(
            device, 
            *format, 
            None, 
            1
        );

        let textures = epaint::textures::TexturesDelta::default();
        let interface = Interface::new();
        let mouse_input = MouseInput::new();

        let frame_info = Frame {
            instant: Instant::now(),
            pixels_updated: 0,
            chunks_updated: 0,
            fps_counter: FPSCounter::new(),
            fps: 0,
        };

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures,
            mouse_input,
            interface,
            frame_info
        }
    }

    pub fn handle_event(
        &mut self, 
        event: &winit::event::WindowEvent, 
        control_flow: &mut winit::event_loop::ControlFlow, 
        scale_factor: f64
    ) -> EventResponse {
        match event {
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                self.mouse_input.handle_mouse_movement(position, scale_factor);
            },

            winit::event::WindowEvent::MouseInput {state, button, ..} => {
                self.mouse_input.handle_mouse_buttons(control_flow, state, button);
            },

            winit::event::WindowEvent::KeyboardInput { input, .. } => {
                if let winit::event::ElementState::Released = input.state {
                    if let Some(keycode) = input.virtual_keycode {
                        match keycode {
                            winit::event::VirtualKeyCode::F1 => {
                                self.interface.menu_bar_open = !self.interface.menu_bar_open;
                            },
                            winit::event::VirtualKeyCode::F2 => {
                                self.interface.info_open = !self.interface.info_open;
                            },
                            winit::event::VirtualKeyCode::F3 => {
                                self.interface.elements_open = !self.interface.elements_open;
                            },
                            winit::event::VirtualKeyCode::Escape | winit::event::VirtualKeyCode::Q  => {
                                control_flow.set_exit();
                            },
                            _ => {}
                        }
                    }
                }  
            },
            _ => {}
        }

        self.egui_state.on_event(&self.egui_ctx, event)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    pub fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
    }

    pub fn prepare(&mut self, window: &Window) {
        let raw_input = self.egui_state.take_egui_input(window);
        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            self.interface.ui(egui_ctx, &self.frame_info, &mut self.mouse_input);
        });

        self.textures.append(output.textures_delta);
        self.egui_state.handle_platform_output(window, &self.egui_ctx, output.platform_output);
        self.paint_jobs = self.egui_ctx.tessellate(output.shapes);
    }

    pub fn render(
        &mut self, 
        encoder: &mut wgpu::CommandEncoder, 
        render_target: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        for (id, image_delta) in &self.textures.set {
            self.renderer.update_texture(device, queue, *id, image_delta);
        }

        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
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

            self.renderer.render(&mut rpass, &self.paint_jobs, &self.screen_descriptor);
        }

        let textures = std::mem::take(&mut self.textures);
        for id in &textures.free {
            self.renderer.free_texture(id);
        }
    }

    //===================
    // Mouse interaction
    //===================
    
    pub fn is_key_released(&self, key: usize) -> bool {
        self.mouse_input.mouse_keys[key] == ElementState::Released
    }
    
    pub fn get_brush(&self) -> Brush {
        self.mouse_input.brush.clone()
    }

    pub fn is_update_required(&self) -> bool {
        self.ms_from_previous_update() > (1000 / TARGET_FPS)
    }

    pub fn drain_placing_queue(&mut self) -> Vec<((i32, i32), Cell)> {
        self.mouse_input.placing_queue.drain().collect()
    }

    pub fn is_cells_queued(&mut self) -> bool {
        !self.mouse_input.placing_queue.is_empty()
    }

    //===================
    // Frame info
    //===================

    pub fn ms_from_previous_update(&self) -> u128 {
        let now = Instant::now();
        now.duration_since(self.frame_info.instant).as_millis()
    }
    
    pub fn next_frame(&mut self) {
        self.frame_info.pixels_updated = 0;
        self.frame_info.chunks_updated = 0;
        self.frame_info.fps = self.frame_info.fps_counter.tick();
        self.frame_info.instant = Instant::now();
    }

    pub fn update_frame_info(&mut self, chunks_updated: u128, pixels_updated: u128) {
        self.frame_info.pixels_updated += pixels_updated;
        self.frame_info.chunks_updated += chunks_updated;
    }
}

impl Interface {
    fn new() -> Self {
        Self { 
            menu_bar_open: false,
            info_open: true, 
            elements_open: true,
        }
    }
    fn ui(&mut self, ctx: &Context, frame_info: &Frame, mouse_input: &mut MouseInput) {
        egui::TopBottomPanel::top("menubar_container")
            .show_animated(ctx, self.menu_bar_open, |ui| {
            egui::menu::bar(ui, |ui| {
                if ui.button("Info").clicked() {
                    self.info_open = !self.info_open;
                }

                ui.separator();

                if ui.button("Elements").clicked() {
                    self.elements_open = !self.elements_open;
                }
            });
        });

        egui::Window::new("Info")
            .open(&mut self.info_open)
            .auto_sized()
            .title_bar(false)
            .anchor(egui::Align2::LEFT_TOP, egui::Vec2 { x: ctx.pixels_per_point() * 8.0, y: ctx.pixels_per_point() * 8.0 })
            .show(ctx, |ui| {
                ui.set_max_width(ctx.pixels_per_point() * 80.0);

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Frame count: {}", frame_info.fps)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Chunks updated: {}", frame_info.chunks_updated)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Pixels updated: {}", frame_info.pixels_updated)
                );
            });

        egui::Window::new("Elements")
        .open(&mut self.elements_open)
        .auto_sized()
        .title_bar(false)
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2 { x: - ctx.pixels_per_point() * 8.0, y: ctx.pixels_per_point() * 8.0 })
        .show(ctx, |ui| {
            ui.set_max_width(ctx.pixels_per_point() * 80.0);

            let mut empty = true;
            for element in ELEMENTS.iter() {
                if !empty {
                    ui.separator();
                }
                else {
                    empty = false;
                }

                let color = element.color;

                let (rect, response) = ui.allocate_exact_size(egui::Vec2 { x: ui.available_width(), y: ctx.pixels_per_point() * 16.0 }, egui::Sense { click: true, drag: false, focusable: true });
                
                ui.allocate_ui_at_rect(rect, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.horizontal_top(|ui| {    
                            ui.add_space(ctx.pixels_per_point() * 4.0);

                            let (rect, _) = ui.allocate_exact_size(egui::Vec2 { x: ctx.pixels_per_point() * 12.0, y: ctx.pixels_per_point() * 12.0 }, egui::Sense { click: false, drag: false, focusable: false });
        
                            ui.painter().rect_filled(
                                rect,
                                egui::Rounding::default().at_most(0.5), 
                                egui::Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3])
                            );

                            if &mouse_input.brush.element == element {
                                ui.painter().rect_stroke(
                                    rect,
                                    egui::Rounding::default().at_most(0.5), 
                                    egui::Stroke::new(ctx.pixels_per_point(), egui::Color32::GOLD)
                                );
                            }

                            
                            ui.vertical(|ui| {
                                ui.add_space(ctx.pixels_per_point() * 4.0);
                                ui.horizontal_top(|ui| {
                                    ui.add_space(ctx.pixels_per_point() * 4.0);
                                    
                                    ui.colored_label(
                                        {
                                            if &mouse_input.brush.element == element {
                                                egui::Color32::GOLD
                                            }
                                            else {
                                                egui::Color32::WHITE
                                            } 
                                        },


                                        element.name.clone()
                                    );
                                });
                            });
                            
                        });
                    })
                });

                if response.clicked() {
                    mouse_input.brush.element = element.clone();
                }
            }

            ui.add_space(ctx.pixels_per_point() * 8.0);

            egui::ComboBox::from_label("Shape")
                .selected_text( 
                    match mouse_input.brush.shape {
                        BrushShape::Circle => "Circle",
                        BrushShape::Square => "Square",
                    }
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut mouse_input.brush.shape, 
                        BrushShape::Square, 
                        "Square",
                    );
                    ui.selectable_value(
                        &mut mouse_input.brush.shape, 
                        BrushShape::Circle, 
                        "Circle",
                    );
                }
            );

            ui.add_space(ctx.pixels_per_point() * 8.0);
            
            ui.label("Brush size");

            ui.add(
                egui::widgets::Slider::new(&mut mouse_input.brush.size, 2..=32)
                    .show_value(true)
                    .trailing_fill(true)
            );

            ui.add_space(ctx.pixels_per_point() * 8.0);

            egui::ComboBox::from_label("Type")
                .selected_text( 
                    match mouse_input.brush.brush_type {
                        BrushType::Cell => "Cell",
                        BrushType::Object => "Object",
                        BrushType::StaticObject => "Static Object",
                        BrushType::Particle(_) => "Particle",
                    }
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut mouse_input.brush.brush_type, 
                        BrushType::Cell, 
                        "Cell",
                    );
                    ui.selectable_value(
                        &mut mouse_input.brush.brush_type, 
                        BrushType::Particle(1), 
                        "Particle",
                    );
                    ui.selectable_value(
                        &mut mouse_input.brush.brush_type, 
                        BrushType::Object, 
                        "Object",
                    );
                    ui.selectable_value(
                        &mut mouse_input.brush.brush_type, 
                        BrushType::StaticObject, 
                        "Static Object",
                    );
                }
            );

            if let BrushType::Particle(size) = &mut mouse_input.brush.brush_type {
                ui.add_space(ctx.pixels_per_point() * 8.0);

                ui.label("Particle spawn rate");

                ui.add(
                    egui::widgets::Slider::new(size, 1..=255)
                        .show_value(true)
                        .trailing_fill(true)
                );
            }

            ui.add_space(ctx.pixels_per_point() * 4.0);
        });
    }
}
