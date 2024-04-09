mod actors;
mod animation;
mod assets;
mod camera;
mod constants;
mod generation;
mod gui;
mod helpers;
mod painter;
mod registries;
mod simulation;
mod state;

use actors::{actor::ActorsPlugin, player::PlayerPlugin};
use ahash::HashMap;
use assets::{
    BiomeMapAssets, FontAsset, FontAssetLoader, FontAssets, PlayerSpriteAssets, TileAssets,
};
use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin,
    input::mouse::MouseMotion,
    prelude::*,
    render::{settings::WgpuSettings, RenderPlugin},
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
    window::PrimaryWindow,
    winit::{UpdateMode, WinitSettings},
};
use bevy_asset_loader::loading_state::{
    config::ConfigureLoadingState, LoadingState, LoadingStateAppExt,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_math::ivec2;

use bevy_rapier2d::{
    dynamics::RigidBody,
    plugin::{NoUserData, RapierConfiguration, RapierPhysicsPlugin},
};
use camera::CameraPlugin;
use constants::{CHUNK_SIZE, PARTICLE_LAYER};
use gui::{
    egui_has_primary_context, ui_info_system, ui_painter_system, ui_selected_cell_system, UiWidgets,
};
use helpers::line_from_pixels;
use painter::{BrushRes, BrushType};
use simulation::{
    chunk_manager::ChunkManager,
    dirty_rect::{update_dirty_rects, DirtyRects},
    materials::MaterialInstance,
    object::Object,
    particle::{Particle, ParticleInstances},
    SimulationPlugin,
};
use state::AppState;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(RenderPlugin {
                    render_creation: bevy::render::settings::RenderCreation::Automatic(
                        WgpuSettings {
                            power_preference: bevy::render::settings::PowerPreference::LowPower,
                            ..Default::default()
                        },
                    ),
                    synchronous_pipeline_compilation: false,
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Sandforge".into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins(
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(CHUNK_SIZE as f32)
                .in_fixed_schedule(),
        )
        .add_systems(Startup, setup_camera)
        // .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(SimulationPlugin)
        .add_plugins(EguiPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins((ActorsPlugin, PlayerPlugin, animation::AnimationPlugin))
        .add_plugins(CameraPlugin)
        .add_plugins(WorldInspectorPlugin::new())
        .add_systems(
            PreUpdate,
            mouse_system
                .run_if(has_window)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                // update_ui_scale_factor,
                ui_info_system,
                ui_selected_cell_system,
                ui_painter_system,
            )
                .run_if(egui_has_primary_context)
                .run_if(in_state(AppState::InGame)),
        )
        .insert_resource(RapierConfiguration {
            gravity: Vec2::new(0.0, -0.98),
            ..Default::default()
        })
        .init_resource::<UiWidgets>()
        .init_resource::<BrushRes>()
        .init_resource::<MouseState>()
        .init_resource::<PainterObjectBuffer>()
        .init_state::<AppState>()
        .init_asset::<FontAsset>()
        .init_asset_loader::<FontAssetLoader>()
        .add_loading_state(
            LoadingState::new(AppState::LoadingScreen)
                .load_collection::<FontAssets>()
                .load_collection::<BiomeMapAssets>()
                .load_collection::<PlayerSpriteAssets>()
                .load_collection::<TileAssets>()
                .continue_to_state(AppState::InGame),
        )
        .register_type::<Particle>()
        .register_type::<MaterialInstance>()
        .run();
}

fn setup_camera(mut commands: Commands, mut time: ResMut<Time<Fixed>>) {
    time.set_timestep_hz(58.);

    let mut camera = Camera2dBundle::default();
    camera.camera.hdr = true;
    camera.projection.scale = 0.25 / CHUNK_SIZE as f32;

    commands.spawn(camera);
}

fn setup_egui(mut contexts: EguiContexts) {
    contexts.ctx_mut().style_mut(|style| {
        style.visuals.override_text_color = Some(egui::Color32::WHITE);
        style.visuals.window_fill = egui::Color32::from_rgba_unmultiplied(27, 27, 27, 200);
        style.interaction.selectable_labels = false;
    });
}

fn setup_egui_fonts(
    mut contexts: EguiContexts,
    fonts: Res<FontAssets>,
    fonts_assets: Res<Assets<FontAsset>>,
) {
    let font = fonts_assets.get(fonts.ui.clone()).unwrap();
    let mut fonts_definitions = egui::FontDefinitions::default();

    fonts_definitions.font_data.insert(
        "pixel font".to_owned(),
        egui::FontData::from_owned(font.get_bytes().clone()),
    );

    fonts_definitions
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "pixel font".to_owned());

    fonts_definitions
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("pixel font".to_owned());

    contexts.ctx_mut().set_fonts(fonts_definitions);
}

#[derive(Default, Resource, PartialEq, Eq)]
enum MouseState {
    #[default]
    Normal,
    Dragging,
    Painting,
}

pub fn has_window(query: Query<&Window, With<PrimaryWindow>>) -> bool {
    !query.is_empty()
}

#[derive(Resource, Default)]
struct PainterObjectBuffer {
    map: HashMap<IVec2, MaterialInstance>,
}

#[allow(clippy::too_many_arguments)]
fn mouse_system(
    mut commands: Commands,
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
    mut object_buffer: ResMut<PainterObjectBuffer>,
    particles: Query<(Entity, &Mesh2dHandle), With<ParticleInstances>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let (particles, particle_mesh) = particles.get_single().unwrap();
    let (camera, mut camera_transform, camera_global_transform) = camera.single_mut();
    let window = window.single();
    let ctx = contexts.ctx_mut();

    if buttons.just_pressed(MouseButton::Left) && !ctx.is_pointer_over_area() {
        if keys.pressed(KeyCode::ShiftLeft) {
            mouse_state.set_if_neq(MouseState::Dragging);
        } else {
            mouse_state.set_if_neq(MouseState::Painting);
            if let Some(position) = window.cursor_position() {
                let world_position = camera
                    .viewport_to_world(camera_global_transform, position)
                    .map(|ray| ray.origin.truncate())
                    // .map(|ray| vec2(ray.x, -ray.y))
                    .unwrap();

                brush.shape.draw(
                    (world_position.x * CHUNK_SIZE as f32).round() as i32,
                    (world_position.y * CHUNK_SIZE as f32).round() as i32,
                    brush.size,
                    &mut |x: i32, y: i32| {
                        if brush.material.is_none() {
                            return;
                        }

                        match brush.brush_type {
                            BrushType::Particle(rate) => {
                                if fastrand::u8(0..255) <= rate {
                                    let particle = Particle::new(
                                        brush.material.as_ref().unwrap().into(),
                                        Vec2::new(x as f32, y as f32),
                                        Vec2::ZERO,
                                    );

                                    let mesh = MaterialMesh2dBundle {
                                        mesh: particle_mesh.clone(),
                                        material: materials.add(Color::rgba_u8(
                                            particle.material.color[0],
                                            particle.material.color[1],
                                            particle.material.color[2],
                                            particle.material.color[3],
                                        )),
                                        transform: Transform::from_translation(
                                            (particle.pos / CHUNK_SIZE as f32)
                                                .extend(PARTICLE_LAYER),
                                        ),
                                        ..Default::default()
                                    };

                                    let particle_handle = commands.spawn((particle, mesh)).id();

                                    commands.entity(particles).add_child(particle_handle);

                                    let chunk_position =
                                        ivec2(x, y).div_euclid(IVec2::ONE * CHUNK_SIZE);
                                    let cell_position =
                                        ivec2(x, y).rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2();

                                    update_dirty_rects(
                                        &mut dirty_rects.current,
                                        chunk_position,
                                        cell_position,
                                    );
                                    update_dirty_rects(
                                        &mut dirty_rects.render,
                                        chunk_position,
                                        cell_position,
                                    );
                                }
                            }
                            BrushType::Object => {
                                object_buffer
                                    .map
                                    .insert(ivec2(x, y), brush.material.as_ref().unwrap().into());
                            }
                            _ => {
                                if chunk_manager
                                    .set(ivec2(x, y), brush.material.as_ref().unwrap().into())
                                    .is_ok()
                                {
                                    let chunk_position =
                                        ivec2(x, y).div_euclid(IVec2::ONE * CHUNK_SIZE);
                                    let cell_position =
                                        ivec2(x, y).rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2();

                                    update_dirty_rects(
                                        &mut dirty_rects.current,
                                        chunk_position,
                                        cell_position,
                                    );
                                    update_dirty_rects(
                                        &mut dirty_rects.render,
                                        chunk_position,
                                        cell_position,
                                    );
                                }
                            }
                        }
                    },
                );
            }
        }
    }

    if buttons.pressed(MouseButton::Left) {
        match mouse_state.as_ref() {
            MouseState::Normal => {}
            MouseState::Dragging => {
                if keys.pressed(KeyCode::ShiftLeft) {
                    for event in motion_evr.read() {
                        camera_transform.translation.x -= event.delta.x / CHUNK_SIZE as f32 / 4.0;
                        camera_transform.translation.y += event.delta.y / CHUNK_SIZE as f32 / 4.0;
                    }
                }
            }
            MouseState::Painting => {
                if let Some(cursor_position) = window.cursor_position() {
                    let mut last_position = camera
                        .viewport_to_world(camera_global_transform, cursor_position)
                        .map(|ray| ray.origin.truncate())
                        .unwrap();

                    let movement_events = cursor_evr.read().collect::<Vec<&CursorMoved>>();
                    for event in movement_events.iter().rev() {
                        let new_position = camera
                            .viewport_to_world(camera_global_transform, event.position)
                            .map(|ray| ray.origin.truncate())
                            .unwrap();

                        line_from_pixels(
                            (last_position * CHUNK_SIZE as f32).round().as_ivec2(),
                            (new_position * CHUNK_SIZE as f32).round().as_ivec2(),
                            &mut |x: i32, y: i32| {
                                brush.shape.draw(x, y, brush.size, &mut |x: i32, y: i32| {
                                    if brush.material.is_none() {
                                        return;
                                    }

                                    match brush.brush_type {
                                        BrushType::Particle(rate) => {
                                            if fastrand::u8(0..255) <= rate {
                                                let particle = Particle::new(
                                                    brush.material.as_ref().unwrap().into(),
                                                    Vec2::new(x as f32, y as f32),
                                                    event
                                                        .delta
                                                        .map(|vel| Vec2::new(vel.x, -vel.y))
                                                        .unwrap_or_default(),
                                                );

                                                let mesh = MaterialMesh2dBundle {
                                                    mesh: particle_mesh.clone(),
                                                    material: materials.add(Color::rgba_u8(
                                                        particle.material.color[0],
                                                        particle.material.color[1],
                                                        particle.material.color[2],
                                                        particle.material.color[3],
                                                    )),
                                                    transform: Transform::from_translation(
                                                        (particle.pos / CHUNK_SIZE as f32)
                                                            .extend(PARTICLE_LAYER),
                                                    ),
                                                    ..Default::default()
                                                };

                                                let particle_handle =
                                                    commands.spawn((particle, mesh)).id();

                                                commands
                                                    .entity(particles)
                                                    .add_child(particle_handle);

                                                let chunk_position =
                                                    ivec2(x, y).div_euclid(IVec2::ONE * CHUNK_SIZE);
                                                let cell_position = ivec2(x, y)
                                                    .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                                    .as_uvec2();

                                                update_dirty_rects(
                                                    &mut dirty_rects.current,
                                                    chunk_position,
                                                    cell_position,
                                                );
                                                update_dirty_rects(
                                                    &mut dirty_rects.render,
                                                    chunk_position,
                                                    cell_position,
                                                );
                                            }
                                        }
                                        BrushType::Object => {
                                            object_buffer.map.insert(
                                                ivec2(x, y),
                                                brush.material.as_ref().unwrap().into(),
                                            );
                                        }
                                        _ => {
                                            if chunk_manager
                                                .set(
                                                    ivec2(x, y),
                                                    brush.material.as_ref().unwrap().into(),
                                                )
                                                .is_ok()
                                            {
                                                let chunk_position =
                                                    ivec2(x, y).div_euclid(IVec2::ONE * CHUNK_SIZE);
                                                let cell_position = ivec2(x, y)
                                                    .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                                    .as_uvec2();

                                                update_dirty_rects(
                                                    &mut dirty_rects.current,
                                                    chunk_position,
                                                    cell_position,
                                                );
                                                update_dirty_rects(
                                                    &mut dirty_rects.render,
                                                    chunk_position,
                                                    cell_position,
                                                );
                                            }
                                        }
                                    }
                                });
                                true
                            },
                        );

                        last_position = new_position;
                    }
                }
            }
        };
    }

    cursor_evr.clear();
    motion_evr.clear();

    if buttons.just_released(MouseButton::Left) {
        if brush.brush_type == BrushType::Object {
            let mut rect: Option<IRect> = None;
            let values = object_buffer
                .map
                .drain()
                .collect::<Vec<(IVec2, MaterialInstance)>>();

            values.iter().for_each(|(pos, _)| {
                let rect = rect.get_or_insert(IRect::new(pos.x, pos.y, pos.x + 1, pos.y + 1));

                rect.min.x = i32::min(rect.min.x, pos.x);
                rect.max.x = i32::max(rect.max.x, pos.x + 1);

                rect.min.y = i32::min(rect.min.y, pos.y);
                rect.max.y = i32::max(rect.max.y, pos.y + 1);
            });

            if let Some(rect) = rect {
                let mut pixels: Vec<Option<MaterialInstance>> =
                    vec![None; (rect.size().x * rect.size().y) as usize];

                values.iter().for_each(|(pos, material)| {
                    let offseted_pos = *pos - rect.min;

                    pixels[(offseted_pos.y * rect.size().x + offseted_pos.x) as usize] =
                        Some(material.clone());
                });

                if let Ok(object) =
                    Object::from_pixels(pixels, rect.size().x as u16, rect.size().y as u16)
                {
                    if let Ok(collider) = object.create_collider() {
                        commands.spawn((
                            object,
                            collider,
                            RigidBody::Dynamic,
                            TransformBundle {
                                local: Transform::from_translation(
                                    rect.center().extend(0).as_vec3() / CHUNK_SIZE as f32,
                                ),
                                ..Default::default()
                            },
                        ));
                    }
                }
            }
        }

        mouse_state.set_if_neq(MouseState::Normal);
    }
}
