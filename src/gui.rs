use std::time::Instant;

use ahash::HashMap;
use egui::Context;
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use egui_winit::{EventResponse, egui};
use fps_counter::FPSCounter;
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;
use winit_input_helper::WinitInputHelper;

use crate::constants::{SCREEN_HEIGHT, SCALE, TARGET_FPS, CHUNK_SIZE, WORLD_WIDTH, WORLD_HEIGHT};
use crate::helpers::line_from_pixels;
use crate::sim::cell::Cell;
use crate::sim::elements::{ELEMENTS, Element};

#[derive(Clone)]
pub struct Brush {
    pub element: Element,
    pub brush_type: BrushType,
    pub shape: BrushShape,
    pub size: i32, 
    pub last_mouse_position: Option<(f32, f32)>,
    pub drawing: bool,

    pub placing_queue: HashMap<(i32, i32), Cell>,
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
    pub fn draw_shape<F: FnMut(i32, i32)> (
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

pub struct Frame {
    pub instant: Instant,
    pub pixels_updated: u128,
    pub chunks_updated: u128,
    pub fps: usize,
    fps_counter: FPSCounter,
}

impl Brush {
    pub fn new() -> Self {
        Self {
            element: Element::default(), 
            brush_type: BrushType::Cell, 
            shape: BrushShape::Circle, 
            size: 10,
            drawing: false,

            placing_queue: HashMap::default(),
            last_mouse_position: None,
        }
    }

    fn draw_point(&mut self, x: i32, y: i32) {
        let mut draw_operation = |x: i32, y: i32| {
            match self.brush_type {
                BrushType::Particle(rate) => {
                    if fastrand::u8(0..255) <= rate {
                        self.placing_queue.insert((x, y), Cell::new(&self.element, 0));
                    }
                },
                _ => {
                    self.placing_queue.insert((x, y), Cell::new(&self.element, 0));
                }
            }
        };

        self.shape.draw_shape(x, y, self.size, &mut draw_operation);
    }

    fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        let mut draw_operation = |x: i32, y: i32| {
            match self.brush_type {
                BrushType::Particle(rate) => {
                    if fastrand::u8(0..255) <= rate {
                        self.placing_queue.insert((x, y), Cell::new(&self.element, 0));
                    }
                },
                _ => {
                    self.placing_queue.insert((x, y), Cell::new(&self.element, 0));
                }
            }
        };

        let mut function = |x: i32, y: i32| {
            self.shape.draw_shape(x, y, self.size, &mut draw_operation);
            true
        };

        line_from_pixels(x1, y1, x2 + x2.signum(), y2 + y2.signum(), &mut function);
    }
}

pub struct Gui {
    egui_ctx: egui_winit::egui::Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    paint_jobs: Vec<epaint::ClippedPrimitive>,
    textures: epaint::textures::TexturesDelta,

    pub screen_coords: [f32; 4],

    brush: Brush,
    interface: Interface,
    frame_info: Frame,
}

struct Interface {
    menu_bar_open: bool,
    info_open: bool,
    elements_open: bool,
    cell_info_open: bool,
    selected_cell: Cell,
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
        let brush = Brush::new();

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
            brush,
            interface,
            frame_info,
            
            screen_coords: [0.0, 0.0, WORLD_WIDTH as f32, WORLD_HEIGHT as f32]
        }
    }

    pub fn handle_event(
        &mut self, 
        input: &WinitInputHelper,
        event: &winit::event::WindowEvent, 
        control_flow: &mut winit::event_loop::ControlFlow, 
        _scale_factor: f64
    ) -> EventResponse {
        if input.mouse_released(0) {
            self.brush.drawing = false;
        }

        let response = self.egui_state.on_event(&self.egui_ctx, event);

        if response.consumed {
            return response;
        }

        let new_mouse_position = input.mouse();

        if input.mouse_pressed(0) {
            self.brush.drawing = true;

            if let Some((x, y)) = new_mouse_position {
                let (x, y) = self.get_world_position_from_pixel(x, y);
                self.brush.draw_point(x, y);
            }
        }

        if input.mouse_held(0) && self.brush.drawing {
            if let Some((new_x, new_y)) = new_mouse_position {
                let (x1, y1) = match self.brush.last_mouse_position {
                    Some((x, y)) => {
                        self.get_world_position_from_pixel(x, y)
                    },
                    None => {
                        self.get_world_position_from_pixel(new_x, new_y)
                    },
                };

                let (x2, y2) = self.get_world_position_from_pixel(new_x, new_y);
                // dbg!(x1 + x_offset);
                self.brush.draw_line(x1, y1, x2, y2);
            }
        }

        if input.mouse_held(1) {
            let dx = - input.mouse_diff().0 / (WORLD_WIDTH * CHUNK_SIZE) as f32 / 10.0;
            let dy = input.mouse_diff().1 / (WORLD_HEIGHT * CHUNK_SIZE) as f32 / 10.0;

            self.screen_coords[0] += dx;
            self.screen_coords[2] += dx;

            self.screen_coords[1] += dy;
            self.screen_coords[3] += dy;
        }

        self.brush.last_mouse_position = new_mouse_position;

        if input.key_pressed(winit::event::VirtualKeyCode::Grave) {
            self.interface.menu_bar_open = !self.interface.menu_bar_open;
        }

        if input.key_pressed(winit::event::VirtualKeyCode::F1) {
            self.interface.info_open = !self.interface.info_open;
        }

        if input.key_pressed(winit::event::VirtualKeyCode::F2) {
            self.interface.elements_open = !self.interface.elements_open;
        }

        if input.key_pressed(winit::event::VirtualKeyCode::F3) {
            self.interface.cell_info_open = !self.interface.cell_info_open;
        }
        
        if input.key_pressed_os(winit::event::VirtualKeyCode::Left) {
            self.screen_coords[0] -= 0.1;
            self.screen_coords[2] -= 0.1;
        }
        
        if input.key_pressed_os(winit::event::VirtualKeyCode::Right) {
            self.screen_coords[0] += 0.1;
            self.screen_coords[2] += 0.1;
        }
        
        if input.key_pressed_os(winit::event::VirtualKeyCode::Up) {
            self.screen_coords[1] += 0.1;
            self.screen_coords[3] += 0.1;
        }
        
        if input.key_pressed_os(winit::event::VirtualKeyCode::Down) {
            self.screen_coords[1] -= 0.1;
            self.screen_coords[3] -= 0.1;
        }

        if input.key_pressed(winit::event::VirtualKeyCode::Escape) || input.key_pressed(winit::event::VirtualKeyCode::Q) {
            control_flow.set_exit();
        }

        response
    }

    pub fn get_world_position_from_pixel(&self, x: f32, y: f32) -> (i32, i32) {
        (
            (x / SCALE / self.screen_descriptor.pixels_per_point + (self.screen_coords[0] * CHUNK_SIZE as f32)).round() as i32, 
            ((SCREEN_HEIGHT - (y / self.screen_descriptor.pixels_per_point)) / SCALE + (self.screen_coords[1] * CHUNK_SIZE as f32)).round() as i32
        )
    }

    pub fn update_selected_cell(&mut self, cell: Cell) {
        self.interface.selected_cell = cell;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    pub fn prepare(&mut self, window: &Window) {
        let raw_input = self.egui_state.take_egui_input(window);
        let coordinates = self.brush.last_mouse_position.map(|(x, y)| self.get_world_position_from_pixel(x, y));

        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            self.interface.ui(
                egui_ctx, 
                &self.frame_info, 
                &mut self.brush, 
                coordinates
            );
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

    pub fn get_last_position(&self) -> Option<(i32, i32)> {
        if let Some((x, y)) = self.brush.last_mouse_position {
            Some(self.get_world_position_from_pixel(x, y))
        }
        else {
            None
        }
    }
    
    pub fn get_brush(&self) -> Brush {
        self.brush.clone()
    }

    pub fn is_update_required(&self) -> bool {
        self.ms_from_previous_update() > (1000 / TARGET_FPS)
    }

    pub fn drain_placing_queue(&mut self) -> Vec<((i32, i32), Cell)> {
        self.brush.placing_queue.drain().collect()
    }

    pub fn is_cells_queued(&mut self) -> bool {
        !self.brush.placing_queue.is_empty()
    }

    //============
    // Frame info
    //============

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
            menu_bar_open: true,
            info_open: true, 
            elements_open: true,
            cell_info_open: true,
            selected_cell: Cell::default()
        }
    }
    fn ui(&mut self, ctx: &Context, frame_info: &Frame, brush: &mut Brush, coordinates: Option<(i32, i32)>) {
        egui::TopBottomPanel::top("menubar_container")
            .show_animated(ctx, self.menu_bar_open, |ui| {
            egui::menu::bar(ui, |ui| {
                if ui.button("F1: Info").clicked() {
                    self.info_open = !self.info_open;
                }

                ui.separator();

                if ui.button("F2: Elements").clicked() {
                    self.elements_open = !self.elements_open;
                }

                ui.separator();

                if ui.button("F3: Selected cell").clicked() {
                    self.elements_open = !self.elements_open;
                }
            });
        });

        egui::Window::new("Info")
            .open(&mut self.info_open)
            .auto_sized()
            .title_bar(false)
            .frame(
                egui::Frame{
                    inner_margin: ctx.style().spacing.window_margin,
                    rounding: ctx.style().visuals.window_rounding,
                    shadow: ctx.style().visuals.window_shadow,
                    stroke: ctx.style().visuals.window_stroke(),
                    fill: egui::Color32::from_rgba_unmultiplied(27, 27, 27, 127),
                    ..Default::default()
                })
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


        egui::Window::new("Selected cell")
            .open(&mut self.cell_info_open)
            .auto_sized()
            .title_bar(false)
            .frame(
                egui::Frame{
                    inner_margin: ctx.style().spacing.window_margin,
                    rounding: ctx.style().visuals.window_rounding,
                    shadow: ctx.style().visuals.window_shadow,
                    stroke: ctx.style().visuals.window_stroke(),
                    fill: egui::Color32::from_rgba_unmultiplied(27, 27, 27, 127),
                    ..Default::default()
                })
            .anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2 { x: ctx.pixels_per_point() * 8.0, y: - ctx.pixels_per_point() * 8.0 })
            .show(ctx, |ui| {
                ui.set_max_width(ctx.pixels_per_point() * 80.0);

                if coordinates.is_none() {
                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("Position: NaN")
                    );
                }
                else {
                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("Position: {}, {}", coordinates.unwrap().0, coordinates.unwrap().1)
                    );
                }

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Element name: {}", ELEMENTS.get(&self.selected_cell.element_id.to_string()).unwrap().value().ui_label.to_string())
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("ra: {}", self.selected_cell.ra)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("rb: {}", self.selected_cell.rb)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    {
                        match self.selected_cell.simulation {
                            crate::sim::cell::SimulationType::Ca => format!("simulation: ca"),
                            crate::sim::cell::SimulationType::RigidBody(object_id, cell_id) => format!("simulation: rb({}, {})", object_id, cell_id),
                            crate::sim::cell::SimulationType::Particle(dx, dy) => format!("simulation: particle({}, {})", dx, dy),
                        }
                    }
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Matter type: {}", &self.selected_cell.matter_type.to_string())
                );

                match self.selected_cell.matter_type {
                    crate::sim::elements::MatterType::Liquid(parameters) => {
                        ui.separator();
                        
                        ui.colored_label(
                            egui::Color32::WHITE,
                            format!("Volume: {}", parameters.volume)
                        );

                        ui.colored_label(
                            egui::Color32::WHITE,
                            format!("Density: {}", parameters.density)
                        );
                    },
                    _ => {},
                }

                if let Some(fire_parameters) = &self.selected_cell.fire_parameters {
                    ui.separator();

                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("burning: {}", self.selected_cell.on_fire)
                    );

                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("temperature: {}", self.selected_cell.temperature)
                    );

                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("fire_hp: {}", fire_parameters.fire_hp)
                    );

                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("fire temperature: {}", fire_parameters.fire_temperature)
                    );

                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("ignition temperature: {}", fire_parameters.ignition_temperature)
                    );
                }
            });

        egui::Window::new("Elements")
        .open(&mut self.elements_open)
        .auto_sized()
        .title_bar(false)
        .frame(
            egui::Frame{
                inner_margin: ctx.style().spacing.window_margin,
                rounding: ctx.style().visuals.window_rounding,
                shadow: ctx.style().visuals.window_shadow,
                stroke: ctx.style().visuals.window_stroke(),
                fill: egui::Color32::from_rgba_unmultiplied(27, 27, 27, 127),
                ..Default::default()
            })
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2 { x: - ctx.pixels_per_point() * 8.0, y: ctx.pixels_per_point() * 8.0 })
        .show(ctx, |ui| {
            ui.set_max_width(ctx.pixels_per_point() * 80.0);

            let mut elements = ELEMENTS.iter()
                .map(|entry| entry.value().clone())
                .collect::<Vec<Element>>();

            elements.sort_by(|a, b| {
                a.id.to_lowercase().cmp(&b.id.to_lowercase())
            });

            let mut empty = true;
            for element in elements.into_iter() {
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

                            if brush.element == element {
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
                                            if brush.element == element {
                                                egui::Color32::GOLD
                                            }
                                            else {
                                                egui::Color32::WHITE
                                            } 
                                        },

                                        element.ui_label.to_string()
                                    );
                                });
                            });
                            
                        });
                    })
                });

                if response.clicked() {
                    brush.element = element.clone();
                }
            }

            ui.add_space(ctx.pixels_per_point() * 8.0);

            egui::ComboBox::from_label("Shape")
                .selected_text( 
                    match brush.shape {
                        BrushShape::Circle => "Circle",
                        BrushShape::Square => "Square",
                    }
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut brush.shape, 
                        BrushShape::Square, 
                        "Square",
                    );
                    ui.selectable_value(
                        &mut brush.shape, 
                        BrushShape::Circle, 
                        "Circle",
                    );
                }
            );

            ui.add_space(ctx.pixels_per_point() * 8.0);
            
            ui.label("Brush size");

            ui.add(
                egui::widgets::Slider::new(&mut brush.size, 2..=32)
                    .show_value(true)
                    .trailing_fill(true)
            );

            ui.add_space(ctx.pixels_per_point() * 8.0);

            egui::ComboBox::from_label("Type")
                .selected_text( 
                    match brush.brush_type {
                        BrushType::Cell => "Cell",
                        BrushType::Object => "Object",
                        BrushType::StaticObject => "Static Object",
                        BrushType::Particle(_) => "Particle",
                    }
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut brush.brush_type, 
                        BrushType::Cell, 
                        "Cell",
                    );
                    ui.selectable_value(
                        &mut brush.brush_type, 
                        BrushType::Particle(1), 
                        "Particle",
                    );
                    ui.selectable_value(
                        &mut brush.brush_type, 
                        BrushType::Object, 
                        "Object",
                    );
                    ui.selectable_value(
                        &mut brush.brush_type, 
                        BrushType::StaticObject, 
                        "Static Object",
                    );
                }
            );

            if let BrushType::Particle(size) = &mut brush.brush_type {
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
