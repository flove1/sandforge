mod constants;
mod chunk;
mod pixel;
mod materials;
mod dirty_rect;
mod world;
mod helpers;
mod gui;
mod painter;
mod actor;
mod player;
mod animation;
mod camera;
mod particle;
mod registries;


use actor::ActorsPlugin;
use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, input::mouse::MouseMotion, prelude::*, render::{settings::WgpuSettings, RenderPlugin}, window::{PresentMode, PrimaryWindow}, winit::{UpdateMode, WinitSettings}};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_math::ivec2;

use camera::CameraPlugin;
use pixel::Pixel;
use constants::CHUNK_SIZE;
use dirty_rect::{update_dirty_rects, DirtyRects};
use gui::{egui_has_primary_context, ui_info_system, ui_painter_system, ui_selected_cell_system, UiWidgets};
use helpers::line_from_pixels;
use painter::{BrushRes, BrushType};
use player::PlayerPlugin;
use world::{ChunkManager, ChunkManagerPlugin};

fn main() {
    // pretty_env_logger::formatted_builder()
    //     .filter_level(log::LevelFilter::Error)
    //     .format_target(false)
    //     .format_timestamp(None)
    //     .init();

    // process_elements_config();
    // let mut watcher = notify::recommended_watcher(|res| {
    //     match res {
    //         Ok(_) => {
    //             println!("elements config updated detected");
    //             process_elements_config();
    //         },
    //         Err(e) => println!("watch error: {:?}", e),
    //     }
    // }).unwrap();

    // if let Err(e) = watcher.watch(Path::new("elements.yaml"), RecursiveMode::NonRecursive) {
    //     panic!("error while loading elements file: {e}");
    // }

    App::new()
        .add_plugins(DefaultPlugins
            .set(
            ImagePlugin::default_nearest(),
            )
            .set(RenderPlugin {
                render_creation: bevy::render::settings::RenderCreation::Automatic(WgpuSettings{
                    power_preference: bevy::render::settings::PowerPreference::LowPower,
                    ..Default::default()
                }),
                synchronous_pipeline_compilation: false,
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "⚙️ Sandforge ⚙️".into(),
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            }),
        )
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins(ChunkManagerPlugin)
        .add_plugins(EguiPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins((
            ActorsPlugin,
            PlayerPlugin,
            animation::AnimationPlugin,
        ))
        .add_plugins(CameraPlugin)
        // // .add_plugins(WorldInspectorPlugin::new())
        .add_systems(Startup, setup)
        // // .add_systems(
        // //     PreUpdate,
        // //     (absorb_egui_inputs,)
        // //             .after(bevy_egui::systems::process_input_system)
        // //             .before(bevy_egui::EguiSet::BeginFrame),
        // // )
        .add_systems(Update, mouse_system.run_if(has_window))
        .add_systems(Update, 
            (
                // update_ui_scale_factor,
                ui_info_system,
                ui_selected_cell_system,
                ui_painter_system,
            ).run_if(egui_has_primary_context)
        )
        // // .add_systems(Update, animate_sprite)
        .init_resource::<UiWidgets>()
        .init_resource::<BrushRes>()
        .init_resource::<MouseState>()
        .run();
}

fn setup(
    mut commands: Commands, 
    mut time: ResMut<Time<Fixed>>,
    mut contexts: EguiContexts,
) {
    time.set_timestep_hz(58.);

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

    contexts.ctx_mut().set_fonts(fonts);
    contexts.ctx_mut().style_mut(|style| {
        style.interaction.selectable_labels = false;
    });

    let mut camera = Camera2dBundle::default();
    camera.camera.hdr = true;
    camera.projection.scale = 0.4 / CHUNK_SIZE as f32;

    commands.spawn(camera);
}

#[derive(Default, Resource, PartialEq, Eq)]
enum MouseState{
    #[default]
    Normal,
    Dragging,
    Painting
}

pub fn has_window(
    query: Query<&Window, With<PrimaryWindow>>,
) -> bool {
    !query.is_empty()
}

#[allow(clippy::too_many_arguments)]
fn mouse_system(
    brush: Res<BrushRes>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects: ResMut<DirtyRects>,
    mut motion_evr: EventReader<MouseMotion>,
    mut cursor_evr: EventReader<CursorMoved>,
    mut camera: Query<(&Camera, &mut Transform, &GlobalTransform), With<Camera>>,
    mut contexts: EguiContexts,
    mut mouse_state: ResMut<MouseState>,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    let (camera, mut camera_transform, camera_global_transform) = camera.single_mut();
    let window = window.single();
    let ctx = contexts.ctx_mut();

    let mut draw_operation = |x: i32, y: i32| {
        if brush.material.is_none() {
            return;
        }

        match brush.brush_type {
            BrushType::Particle(rate) => {
                if fastrand::u8(0..255) <= rate {
                    chunk_manager.replace_cell_at(ivec2(x, y), Pixel::new(brush.material.as_ref().unwrap(), 0));

                    let chunk_position = ivec2(x, y).div_euclid(IVec2::ONE * CHUNK_SIZE);
                    let cell_position = ivec2(x, y).rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2();

                    update_dirty_rects(&mut dirty_rects.current, chunk_position, cell_position);
                    update_dirty_rects(&mut dirty_rects.render, chunk_position, cell_position);
                }
            },
            BrushType::ObjectEraser => {},
            _ => {
                chunk_manager.replace_cell_at(ivec2(x, y), Pixel::new(brush.material.as_ref().unwrap(), 0));

                let chunk_position = ivec2(x, y).div_euclid(IVec2::ONE * CHUNK_SIZE);
                let cell_position = ivec2(x, y).rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2();

                update_dirty_rects(&mut dirty_rects.current, chunk_position, cell_position);
                update_dirty_rects(&mut dirty_rects.render, chunk_position, cell_position);
            }
        }
    };

    if buttons.just_pressed(MouseButton::Left) && !ctx.is_pointer_over_area() {
        if keys.pressed(KeyCode::ShiftLeft) {
            mouse_state.set_if_neq(MouseState::Dragging);
        }
        else {
            mouse_state.set_if_neq(MouseState::Painting);
            if let Some(position) = window.cursor_position() {
                let world_position = camera
                    .viewport_to_world(
                        camera_global_transform, 
                        position
                    )
                    .map(|ray| ray.origin.truncate())
                    // .map(|ray| vec2(ray.x, -ray.y))
                    .unwrap();
    
                brush.shape.draw(
                    (world_position.x * CHUNK_SIZE as f32).round() as i32,
                    (world_position.y * CHUNK_SIZE as f32).round() as i32, 
                    brush.size, &mut draw_operation
                );
            }
        }
    }

    if buttons.pressed(MouseButton::Left) {        
        match mouse_state.as_ref() {
            MouseState::Normal => {},
            MouseState::Dragging => {
                if keys.pressed(KeyCode::ShiftLeft) {
                    for event in motion_evr.read() {
                        camera_transform.translation.x -= event.delta.x / CHUNK_SIZE as f32 / 4.0;
                        camera_transform.translation.y += event.delta.y / CHUNK_SIZE as f32 / 4.0;
                    }
                }
            },
            MouseState::Painting => {
                if let Some(cursor_position) = window.cursor_position() {
                    let mut last_position = camera
                        .viewport_to_world(
                            camera_global_transform, 
                            cursor_position
                        )
                        .map(|ray| ray.origin.truncate())
                        .unwrap();

                    let mut function = |x: i32, y: i32| {
                        brush.shape.draw(x, y, brush.size, &mut draw_operation);
                        true
                    };

                    let movement_events = cursor_evr.read().collect::<Vec<&CursorMoved>>();
                    for event in movement_events.iter().rev() {
                        let new_position =  camera
                            .viewport_to_world(
                                camera_global_transform, 
                                event.position
                            )
                            .map(|ray| ray.origin.truncate())
                            .unwrap();

                        line_from_pixels(
                            (last_position * CHUNK_SIZE as f32).round().as_ivec2(),
                            (new_position * CHUNK_SIZE as f32).round().as_ivec2(),
                            &mut function
                        );

                        last_position = new_position;
                    }
                }
            },
        };
    }

    cursor_evr.clear();
    motion_evr.clear();

    if buttons.just_released(MouseButton::Left) {
        mouse_state.set_if_neq(MouseState::Normal);
    }
}