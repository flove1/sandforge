use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use egui_winit::egui::Context;
use egui_winit::{EventResponse, egui};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;
use winit_input_helper::WinitInputHelper;

use crate::painter::{Brush, BrushShape, BrushType};
use crate::sim::cell::Cell;
use crate::sim::elements::{Element, ELEMENTS};

pub struct Gui {
    egui_ctx: egui_winit::egui::Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    paint_jobs: Vec<epaint::ClippedPrimitive>,
    textures: epaint::textures::TexturesDelta,

    pub widget_data: WidgetData,
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

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures,

            widget_data: WidgetData::new(),
        }
    }

    pub fn handle_event(
        &mut self, 
        input: &WinitInputHelper,
        event: &winit::event::WindowEvent,
    ) -> EventResponse {        
        let response = self.egui_state.on_event(&self.egui_ctx, event);

        if input.key_pressed(winit::event::VirtualKeyCode::Grave) {
            self.widget_data.menu_bar_open = !self.widget_data.menu_bar_open;
        }

        if input.key_pressed(winit::event::VirtualKeyCode::F1) {
            self.widget_data.info_open = !self.widget_data.info_open;
        }

        if input.key_pressed(winit::event::VirtualKeyCode::F2) {
            self.widget_data.elements_open = !self.widget_data.elements_open;
        }

        if input.key_pressed(winit::event::VirtualKeyCode::F3) {
            self.widget_data.cell_info_open = !self.widget_data.cell_info_open;
        }

        response
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    pub fn prepare(&mut self, window: &Window, brush: &mut Brush) {
        let raw_input = self.egui_state.take_egui_input(window);

        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            self.widget_data.render(egui_ctx, brush);
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
}

pub struct WidgetData {
    pub menu_bar_open: bool,
    pub info_open: bool,
    pub elements_open: bool,
    pub cell_info_open: bool,

    pub selected_cell: Cell,

    pub pixels_updated: u128,
    pub chunks_updated: u128,
    pub fps: usize,
    pub mouse_posititon: Option<(i32, i32)>
}

impl WidgetData {
    pub fn new() -> Self {
        Self { 
            menu_bar_open: true,
            info_open: true, 
            elements_open: true,
            cell_info_open: true,

            selected_cell: Cell::default(),

            pixels_updated: 0,
            chunks_updated: 0,
            fps: 0,

            mouse_posititon: None,
        }
    }

    pub fn render(&mut self, ctx: &Context, brush: &mut Brush) {
        self.ui_menu_bar(ctx);
        self.ui_cell_info(ctx);
        self.ui_info(ctx);
        self.ui_elements(ctx, brush);
    }

    fn ui_menu_bar(&mut self, ctx: &Context) {
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
                    self.cell_info_open = !self.cell_info_open;
                }
            });
        });
    }

    fn ui_info(&mut self, ctx: &Context) {
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
                    format!("Frame count: {}", self.fps)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Chunks updated: {}", self.chunks_updated)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Pixels updated: {}", self.pixels_updated)
                );
            });
    }

    fn ui_cell_info(&mut self, ctx: &Context) {
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

                if self.mouse_posititon.is_none() {
                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("Position: NaN")
                    );
                }
                else {
                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("Position: {}, {}", self.mouse_posititon.unwrap().0, self.mouse_posititon.unwrap().1)
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
                            crate::sim::cell::SimulationType::Displaced(dx, dy) => format!("simulation: displaced({}, {})", dx, dy),
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
    }

    fn ui_elements(&mut self, ctx: &Context, brush: &mut Brush) {
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
                            BrushType::StaticObject => "Static object",
                            BrushType::Particle(_) => "Particle",
                            BrushType::Force(_) => "Force",
                            BrushType::ObjectEraser => "Object eraser",
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
                        ui.selectable_value(
                            &mut brush.brush_type, 
                            BrushType::Force(0.25), 
                            "Force",
                        );
                        ui.selectable_value(
                            &mut brush.brush_type, 
                            BrushType::ObjectEraser, 
                            "Object eraser",
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

                if let BrushType::Force(value) = &mut brush.brush_type {
                    ui.add_space(ctx.pixels_per_point() * 8.0);

                    ui.label("Velocity change");

                    ui.add(
                        egui::widgets::Slider::new(value,0.0..=1.0)
                            .show_value(true)
                            .trailing_fill(true)
                    );
                }

                ui.add_space(ctx.pixels_per_point() * 4.0);
            });
    }
}
