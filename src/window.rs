use wgpu::SurfaceError;
use winit::{dpi::LogicalSize, window::WindowBuilder, event_loop::EventLoop};

use crate::constants::{SCREEN_WIDTH, SCREEN_HEIGHT};

pub struct WindowContext {
    pub window: winit::window::Window,
    pub surface: wgpu::Surface,
    pub config: wgpu::SurfaceConfiguration,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl WindowContext {
    pub async fn new<T>(event_loop: &EventLoop<T>) -> Self {
        let window = {
            let size = LogicalSize::new(SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
    
            if cfg!(windows) {
                WindowBuilder::new()
                    .with_title("Rust-physics")
                    .with_inner_size(size)
                    .with_min_inner_size(size)
                    .with_max_inner_size(size)
                    .build(&event_loop)
                    .unwrap()
            }
            else {
                use winit::platform::x11::{WindowBuilderExtX11, XWindowType};
    
                WindowBuilder::new()
                    .with_title("Rust-physics")
                    .with_inner_size(size)
                    .with_x11_window_type(vec![XWindowType::Dialog])
                    .with_min_inner_size(size)
                    .with_max_inner_size(size)
                    .build(&event_loop)
                    .unwrap()   
            }
        };

        let size = window.inner_size();
    
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
    
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
    
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                None,
            )
            .await
            .unwrap();
    
        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
    
        Self {
            window,
            surface,
            config,
            device,
            queue,
        }
    }

    pub fn render_with<F>(&self, render_function: F) -> Result<(), SurfaceError>
    where
        F: FnOnce(
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            &wgpu::Extent3d,
            &wgpu::Device,
            &wgpu::Queue,
        ),
    {
        let output = self.surface.get_current_texture()?;   
        let output_size = output.texture.size();

        let output_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Renderer render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }
        
        (render_function)(&mut encoder, &output_view, &output_size, &self.device, &self.queue);

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}


