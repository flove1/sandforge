use egui::{ClippedPrimitive, Context, TexturesDelta};
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use egui_winit::EventResponse;
use pixels::{wgpu, PixelsContext};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

use crate::input::InputManager;
use crate::sim::elements::Element;

pub(crate) struct Framework {
    egui_ctx: Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,

    gui: Gui,
}

struct Gui {
    info_open: bool,
    elements_open: bool,
}

impl Framework {
    pub(crate) fn new<T>(event_loop: &EventLoopWindowTarget<T>, width: u32, height: u32, scale_factor: f32, pixels: &pixels::Pixels,) -> Self {
        let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;

        let egui_ctx = Context::default();

        let mut fonts = egui::FontDefinitions::default();

        fonts.font_data.insert(
            "pixel font".to_owned(),
            egui::FontData::from_static(include_bytes!(
                "../assets/font.ttf"
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
        let renderer = Renderer::new(pixels.device(), pixels.render_texture_format(), None, 1);
        let textures = TexturesDelta::default();
        let gui = Gui::new();        

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures,
            gui,
        }
    }

    pub(crate) fn handle_event(&mut self, event: &winit::event::WindowEvent) -> EventResponse {
        match event {
            winit::event::WindowEvent::KeyboardInput { input, .. } => {
                if let winit::event::ElementState::Released = input.state {
                    if let Some(keycode) = input.virtual_keycode {
                        match keycode {
                            winit::event::VirtualKeyCode::F1 => {
                                self.gui.info_open = !self.gui.info_open;
                            },
                            winit::event::VirtualKeyCode::F2 => {
                                self.gui.elements_open = !self.gui.elements_open;
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

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    pub(crate) fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
    }

    pub(crate) fn prepare(&mut self, input_manager: &mut InputManager, window: &Window) {
        let raw_input = self.egui_state.take_egui_input(window);
        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            self.gui.ui(input_manager, egui_ctx);
        });

        self.textures.append(output.textures_delta);
        self.egui_state.handle_platform_output(window, &self.egui_ctx, output.platform_output);
        self.paint_jobs = self.egui_ctx.tessellate(output.shapes);
    }

    pub(crate) fn render(&mut self, encoder: &mut wgpu::CommandEncoder, render_target: &wgpu::TextureView, context: &PixelsContext) {
        for (id, image_delta) in &self.textures.set {
            self.renderer.update_texture(&context.device, &context.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &context.device,
            &context.queue,
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

impl Gui {
    fn new() -> Self {
        Self { 
            info_open: true, 
            elements_open: true,
        }
    }

    fn ui(&mut self, input_manager: &mut InputManager , ctx: &Context) {
        egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
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
                    format!("Frame count: {}", input_manager.previous_frame.fps)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Chunks updated: {}", input_manager.previous_frame.chunks_updated)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("Pixels updated: {}", input_manager.previous_frame.pixels_updated)
                );

                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,format!("Brush size: {}", input_manager.mouse.brush_size)
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
            for element in Element::iterator() {
                if !empty {
                    ui.separator();
                }
                else {
                    empty = false;
                }

                let color = element.color();

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

                            if &input_manager.element == element {
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
                                            if &input_manager.element == element {
                                                egui::Color32::GOLD
                                            }
                                            else {
                                                egui::Color32::WHITE
                                            } 
                                        },
                                        element.to_string()
                                    );
                                });
                            });
                            
                        });
                    })
                });

                if response.clicked() {
                    input_manager.element = *element;
                }
            }

            ui.add_space(ctx.pixels_per_point() * 8.0);
            
            ui.horizontal_top(|ui| {
                ui.add_space(ctx.pixels_per_point() * 4.0);

                ui.add(
                    egui::widgets::Slider::new(&mut input_manager.mouse.brush_size, 2..=32)
                        .show_value(true)
                        .trailing_fill(true)
                );
            });

            ui.add_space(ctx.pixels_per_point() * 8.0);

            ui.horizontal_top(|ui| {
                ui.add_space(ctx.pixels_per_point() * 8.0);
                ui.checkbox(&mut input_manager.draw_object, " Draw objects");
            });

            if input_manager.draw_object {
                ui.add_space(ctx.pixels_per_point() * 8.0);
    
                ui.horizontal_top(|ui| {
                    ui.add_space(ctx.pixels_per_point() * 8.0);
                    ui.checkbox(&mut input_manager.draw_static_object, " Is object static?");
                });
            }

            ui.add_space(ctx.pixels_per_point() * 8.0);

            ui.horizontal_top(|ui| {
                ui.add_space(ctx.pixels_per_point() * 8.0);
                ui.checkbox(&mut input_manager.render_objects, " Render objects");
            });

            ui.add_space(ctx.pixels_per_point() * 4.0);
        });
    }
}
