#![allow(dead_code)]

use fps_counter::FPSCounter;
use gui::Gui;
use notify::{Watcher, RecursiveMode};
use painter::Painter;
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;
use window::WindowContext;
use winit::event_loop::EventLoop;
use winit_input_helper::WinitInputHelper;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

mod sim;
mod vector;
mod constants;
mod helpers;
mod painter;
mod gui;
mod window;

use crate::sim::elements::{MatterType, process_elements_config};

use parking_lot::deadlock;
use sim::world::World;
use winit::event::Event;
use crate::constants::*;

struct Camera {
    position: [f32; 2],
}

struct MainState {
    gui: Gui,
    painter: Painter,
    camera: Camera,
    scale_factor: f32,
    fps: Fps,
}

struct Fps {
    instant: Instant,
    value: usize,
    fps_counter: FPSCounter,
}

impl Fps {
    fn ms_from_previous_update(&self) -> u128 {
        let now = Instant::now();
        now.duration_since(self.instant).as_millis()
    }

    fn is_update_required(&self) -> bool {
        self.ms_from_previous_update() > (1000 / TARGET_FPS)
    }
    
    pub fn next_frame(&mut self) {
        self.value = self.fps_counter.tick();
        self.instant = Instant::now();
    }
}

impl MainState {
    fn new<T>(ctx: &WindowContext, event_loop: &EventLoop<T>) -> MainState {
        let gui = {
            let window_size = ctx.window.inner_size();
            let scale_factor = ctx.window.scale_factor() as f32;
            
            Gui::new(
                &event_loop,
                window_size.width,
                window_size.height,
                scale_factor,
                &ctx.device,
                &ctx.config.format
            )
};

        MainState {
            gui,
            painter: Painter::new(),

            camera: Camera {
                position: [0.0, 0.0],
            },

            fps: Fps {
                instant: Instant::now(),
                fps_counter: FPSCounter::new(),
                value: 0,
            },
            
            scale_factor: ctx.window.scale_factor() as f32,
        }
    }

    pub fn get_world_position_from_pixel(&self, x: f32, y: f32) -> (i32, i32) {
        (
            (x / SCALE / self.scale_factor + ((self.camera.position[0] - WORLD_WIDTH as f32 / 2.0) * CHUNK_SIZE as f32)).round() as i32, 
            ((SCREEN_HEIGHT - (y / self.scale_factor)) / SCALE + ((self.camera.position[1] - WORLD_HEIGHT as f32 / 2.0) * CHUNK_SIZE as f32)).round() as i32
        )
    }
}

impl MainState {
    fn handle_painter(&mut self, ctx: &WindowContext, world: &mut World, input: &WinitInputHelper) {
        if self.painter.is_cells_queued() {
            if self.painter.brush.element.matter_type == MatterType::Empty {
                world.place_batch(self.painter.drain_placing_queue());
            }
            else {
                match self.painter.brush.brush_type {
                    painter::BrushType::Cell => {
                        world.place_batch(self.painter.drain_placing_queue());
                    },
                    painter::BrushType::Object => {
                        if !matches!(self.painter.brush.element.matter_type, MatterType::Static) {
                            self.painter.drain_placing_queue();
                        }
                        else if !input.mouse_held(0) {
                            world.place_object(
                                self.painter.drain_placing_queue(),
                                false,
                                &ctx.device,
                                &ctx.queue
                            );
                        }
                    },
                    painter::BrushType::StaticObject => {
                        if !input.mouse_held(0) {
                            world.place_object(
                                self.painter.drain_placing_queue(),
                                true,
                                &ctx.device,
                                &ctx.queue
                            );
                        }
                    },
                    painter::BrushType::Particle(_) => {
                        world.place_particles(self.painter.drain_placing_queue());
                    },
                }          
            }

        }
    }
    
}

pub async fn run() {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window_ctx = WindowContext::new(&event_loop).await;
    let mut main_state = MainState::new(&window_ctx, &event_loop);

    process_elements_config();
    let mut watcher = notify::recommended_watcher(|res| {
        match res {
            Ok(_) => {
                println!("elements config updated detected");
                process_elements_config();
            },
            Err(e) => println!("watch error: {:?}", e),
        }
    }).unwrap();

    if let Err(e) = watcher.watch(Path::new("./elements.yaml"), RecursiveMode::NonRecursive) {
        panic!("error while loading elements file: {e}");
    }

    let mut world = World::new(&window_ctx.device, &window_ctx.config.format);

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();

        let mut event_consumed = false;

        if let Event::WindowEvent { event, .. } = &event {
            event_consumed = main_state.gui.handle_event(&input, event).consumed;
        }

        if !event_consumed && input.update(&event){
            if input.mouse_pressed(0) {
                main_state.painter.activate();
                if let Some((x, y)) = input.mouse() {
                    let (x, y) = main_state.get_world_position_from_pixel(x, y);
                    main_state.painter.draw_point(x, y);
                }
            }

            if input.mouse_held(0) {
                if let Some((x, y)) = input.mouse() {
                    let (dx, dy) = input.mouse_diff();
                            
                    let (x1, y1) = main_state.get_world_position_from_pixel(x - dx, y - dy);
                    let (x2, y2) = main_state.get_world_position_from_pixel(x, y);

                    main_state.painter.draw_line(x1, y1, x2, y2);
                }
            }

            if input.mouse_released(0) {
                main_state.painter.deactivate();
            }

            if input.mouse_held(1) {
                let (dx, dy) = input.mouse_diff();

                main_state.camera.position[0] -= dx / (WORLD_WIDTH * CHUNK_SIZE) as f32 / 5.0;
                main_state.camera.position[1] += dy / (WORLD_HEIGHT * CHUNK_SIZE) as f32 / 5.0;
            }
        
            if input.key_pressed_os(winit::event::VirtualKeyCode::Left) {
                main_state.camera.position[0] -= 0.1;
            }
            
            if input.key_pressed_os(winit::event::VirtualKeyCode::Right) {
                main_state.camera.position[0] += 0.1;
            }
            
            if input.key_pressed_os(winit::event::VirtualKeyCode::Up) {
                main_state.camera.position[1] += 0.1;
            }
            
            if input.key_pressed_os(winit::event::VirtualKeyCode::Down) {
                main_state.camera.position[1] -= 0.1;
            }

            if input.key_pressed(winit::event::VirtualKeyCode::Q) {
                control_flow.set_exit();
            }
        }

        match &event {
            Event::MainEventsCleared => {
                if main_state.fps.is_update_required() {
                    window_ctx.window.request_redraw();
    
                    if world.needs_update(main_state.fps.ms_from_previous_update()) {
                        main_state.handle_painter(&window_ctx, &mut world, &input);
        
                        let (chunks_updated, pixels_updated) = world.update(main_state.camera.position);
                        main_state.gui.widget_data.chunks_updated += chunks_updated;
                        main_state.gui.widget_data.pixels_updated += pixels_updated;
        
                        world.update_textures(
                            &window_ctx,
                            main_state.camera.position
                        );
                    }
                }
            }
            Event::RedrawRequested(_) => {
                let posititon = input.mouse().map(|(x, y)| {
                    main_state.get_world_position_from_pixel(x, y)
                });

                main_state.gui.widget_data.mouse_posititon = posititon;
                if let Some((x, y)) = posititon {
                    main_state.gui.widget_data.selected_cell = world.get_cell_by_pixel(x, y);
                }

                main_state.gui.widget_data.fps = main_state.fps.value;

                let rendering_result = window_ctx.render_with(|encoder, view, output_size, device, queue| {
                    world.render(device, encoder, view, output_size);

                    main_state.gui.prepare(&window_ctx.window, &mut main_state.painter.brush);
                    main_state.gui.render(encoder, view, device, queue);
                });

                main_state.gui.widget_data.chunks_updated = 0;
                main_state.gui.widget_data.pixels_updated = 0;

                main_state.fps.next_frame();

                if rendering_result.is_err() {
                    println!("error while rendering");
                    control_flow.set_exit();
                }

            },
            _ => {}
        }
    });
}

pub fn deadlock_checker() {
    loop {
        thread::sleep(Duration::from_secs(10));
        let deadlocks = deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        println!("{} deadlocks detected", deadlocks.len());
        for (i, threads) in deadlocks.iter().enumerate() {
            println!("Deadlock #{}", i);
            for t in threads {
                println!("Thread Id {:#?}", t.thread_id());
                println!("{:#?}", t.backtrace());
            }
        }
    }
}

// pub fn bench_init() -> WorldApi {
//     World::new()
// }

pub fn bench_fill(world: &mut World) {
    // let element = Element {
    //     color: [0, 0, 0, 0], 
    //     color_offset: 0,
    //     id: format_compact!("bench_powder"),
    //     ui_label: format_compact!("BENCH POWDER"),
    //     matter_type: MatterType::Powder,
    // };

    // for x in 0..(WORLD_WIDTH * CHUNK_SIZE) {
    //     for y in 0..CHUNK_SIZE {
    //         world.set_cell_by_pixel(x, y, &element);
    //     }
    // }

    // for x in 0..(WORLD_WIDTH * CHUNK_SIZE) {
    //     for y in (WORLD_HEIGHT / 2 * CHUNK_SIZE)..((WORLD_HEIGHT / 2 + 1) * CHUNK_SIZE) {
    //         world.set_cell_by_pixel(x, y, &element);
    //     }
    // }
}

pub fn bench_until_empty(world: &mut World) {
    loop {
        world.forced_update();
    };
}

pub fn bench_number_of_updated(world: &mut World, n: usize) {
    for _ in 0..n {
        world.forced_update();
    }
}