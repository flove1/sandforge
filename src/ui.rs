use egui::{ClippedPrimitive, Context, TexturesDelta};
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use egui_winit::EventResponse;
use pixels::{wgpu, PixelsContext};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

use crate::constants::INFO_MENU_OPEN;
use crate::input::StateManager;

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
    window_open: bool,
}

impl Framework {
    pub(crate) fn new<T>(event_loop: &EventLoopWindowTarget<T>, width: u32, height: u32, scale_factor: f32, pixels: &pixels::Pixels,) -> Self {
        let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;

        let egui_ctx = Context::default();
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

    pub(crate) fn prepare(&mut self, state_manager: &StateManager, window: &Window) {
        let raw_input = self.egui_state.take_egui_input(window);
        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            self.gui.ui(state_manager, egui_ctx);
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
        Self { window_open: INFO_MENU_OPEN }
    }

    fn ui(&mut self, state_manager: &StateManager , ctx: &Context) {
        egui::TopBottomPanel::top("menubar_container").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                if ui.button("Info").clicked() {
                    self.window_open = !self.window_open;
                }
            });
        });

        egui::Window::new("Info")
            .open(&mut self.window_open)
            .show(ctx, |ui| {
                ui.label(format!("Frame count: {}", state_manager.previous_frame.fps));
                ui.label(format!("Chunks updated: {}", state_manager.previous_frame.chunks_updated));
                ui.label(format!("Pixels updated: {}", state_manager.previous_frame.pixels_updated));

                ui.separator();

                ui.label(format!("Brush size: {}", state_manager.brush_size));
                ui.label(format!("Element : {}", state_manager.element.to_string()));
            });
    }
}
