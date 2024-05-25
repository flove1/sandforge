use std::time::Duration;

use bevy_egui::{
    egui::{ self, Color32, Frame, Id, ImageSource, Layout, Sense, TextureOptions, Ui },
    EguiContext,
    EguiContexts,
};

use bevy::{
    diagnostic::{ DiagnosticsStore, FrameTimeDiagnosticsPlugin },
    prelude::*,
    transform::commands,
    window::PrimaryWindow,
};
use bevy_math::{ ivec2, vec2 };
use bevy_rapier2d::{
    dynamics::{ ExternalImpulse, RigidBody, Sleeping, Velocity },
    geometry::ColliderMassProperties,
};
use egui_notify::{ ToastLevel, Toasts };

use crate::{
    actors::{ health::Health, player::{ Player, PlayerMaterials, PlayerSelectedMaterial } },
    assets::{ process_assets, FontAssets, FontBytes },
    camera::TrackingCamera,
    constants::CHUNK_SIZE,
    has_window,
    painter::{ BrushRes, BrushShape, BrushType, PainterObjectBuffer },
    registries::Registries,
    simulation::{
        chunk_manager::ChunkManager,
        materials::{ Material, PhysicsType },
        object::{ get_object_by_click, Object, ObjectBundle },
    },
    state::AppState,
};

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ToastEvent>()
            .init_resource::<Inventory>()
            .init_resource::<ToastsRes>()
            .add_systems(
                OnExit(AppState::LoadingAssets),
                (setup_egui, setup_user_interface).after(process_assets)
            )
            .add_systems(
                Update,
                (
                    ui_info_system,
                    ui_selected_cell_system,
                    ui_painter_system,
                    // ui_inventory_system,
                    show_toasts,
                    get_object_by_click,
                )
                    .run_if(has_window)
                    .run_if(egui_has_primary_context)
                    .run_if(in_state(AppState::Game))
            )
            .add_systems(
                Update,
                (synchonize_health_value, synchonize_materials).run_if(in_state(AppState::Game))
            );
    }
}

#[derive(Clone, Event)]
pub struct ToastEvent {
    pub duration: Duration,
    pub level: ToastLevel,
    pub content: String,
}

#[derive(Default, Resource)]
pub struct ToastsRes(Toasts);

#[derive(Component)]
pub struct UIHealthBar;

#[derive(Component)]
pub struct UIMaterials;

#[derive(Component)]
pub struct UIMaterialName;

fn synchonize_materials(
    registries: Res<Registries>,
    selected_material: Res<PlayerSelectedMaterial>,
    mut stored_materials: ResMut<PlayerMaterials>,
    mut style_q: Query<(&mut Style, &mut BackgroundColor), With<UIMaterials>>
) {
    let (mut style, mut color) = style_q.single_mut();

    if selected_material.is_changed() || stored_materials.is_changed() {
        let id = selected_material.0.clone();
        let value = stored_materials.entry(selected_material.0.clone()).or_insert(0.0);

        style.height = Val::Percent(value.clamp(0.0, 100.0));
        let material_color = registries.materials.get(&id).unwrap().color;
        color.0 = Color::rgba_u8(
            material_color[0],
            material_color[1],
            material_color[2],
            material_color[3]
        );
    }
}

fn synchonize_health_value(
    player_q: Query<&Health, With<Player>>,
    mut health_bar: Query<&mut Style, With<UIHealthBar>>
) {
    let health = player_q.single();
    let mut style = health_bar.single_mut();

    style.width = Val::Percent((health.current.max(0.0) / health.total) * 100.0);
}

fn setup_user_interface(mut commands: Commands, asset_server: Res<AssetServer>) {
    let image = asset_server.load("health_border.png");

    let slicer = TextureSlicer {
        border: BorderRect::square(10.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 1.0,
    };

    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Auto,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(20.0),
                margin: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            parent.spawn(NodeBundle::default()).with_children(|parent| {
                parent
                    .spawn((
                        ImageBundle {
                            style: Style {
                                width: Val::Px(160.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::Start,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(12.0)),
                                ..default()
                            },
                            image: image.clone().into(),
                            ..default()
                        },
                        ImageScaleMode::Sliced(slicer.clone()),
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            UIHealthBar,
                            NodeBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    position_type: PositionType::Relative,
                                    ..default()
                                },
                                background_color: Color::WHITE.into(),
                                ..default()
                            },
                        ));
                    });
            });

            parent
                .spawn((
                    NodeBundle {
                        style: Style {
                            height: Val::Auto,
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ))
                .with_children(|parent| {
                    parent
                        .spawn((
                            ImageBundle {
                                style: Style {
                                    width: Val::Px(32.0),
                                    height: Val::Px(160.0),
                                    justify_self: JustifySelf::Center,
                                    align_items: AlignItems::End,
                                    padding: UiRect::all(Val::Px(12.0)),
                                    ..default()
                                },
                                image: image.clone().into(),
                                ..default()
                            },
                            ImageScaleMode::Sliced(slicer.clone()),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                UIMaterials,
                                NodeBundle {
                                    style: Style {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(50.0),
                                        max_height: Val::Percent(100.0),
                                        ..default()
                                    },
                                    background_color: Color::rgb_u8(0xf2, 0xf1, 0xa3).into(),
                                    ..default()
                                },
                            ));
                        });

                    parent.spawn((
                        UIMaterialName,
                        TextBundle {
                            text: Text::from_section("sand", TextStyle {
                                font_size: 18.0,
                                color: Color::WHITE,
                                ..Default::default()
                            }),
                            style: Style {
                                justify_self: JustifySelf::Center,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    ));
                });
        });
}

fn setup_egui(
    mut contexts: EguiContexts,
    fonts: Res<FontAssets>,
    fonts_assets: Res<Assets<FontBytes>>
) {
    contexts.ctx_mut().style_mut(|style| {
        style.visuals.override_text_color = Some(egui::Color32::WHITE);
        style.visuals.window_fill = egui::Color32::from_rgba_unmultiplied(27, 27, 27, 200);
        style.interaction.selectable_labels = false;
    });

    let font = fonts_assets.get(fonts.ui.clone()).unwrap();
    let mut fonts_definitions = egui::FontDefinitions::default();

    fonts_definitions.font_data.insert(
        "pixel font".to_owned(),
        egui::FontData::from_owned(font.get_bytes().clone())
    );

    fonts_definitions.families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "pixel font".to_owned());

    fonts_definitions.families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("pixel font".to_owned());

    contexts.ctx_mut().set_fonts(fonts_definitions);
}

pub fn show_toasts(
    mut toasts: ResMut<ToastsRes>,
    mut events: EventReader<ToastEvent>,
    mut egui_ctx_q: Query<&mut EguiContext, With<PrimaryWindow>>
) {
    let Ok(mut egui_ctx) = egui_ctx_q.get_single_mut() else {
        return;
    };

    let ctx = egui_ctx.get_mut();

    let toasts = &mut toasts.0;

    for event in events.read() {
        let ToastEvent { duration, level, content } = event.clone();

        toasts.basic(content).set_level(level).set_duration(Some(duration));
    }

    toasts.show(ctx);
}

pub fn egui_has_primary_context(query: Query<&EguiContext, With<PrimaryWindow>>) -> bool {
    !query.is_empty()
}

fn ui_info_system(
    diagnostics: Res<DiagnosticsStore>,
    mut egui_ctx_q: Query<&mut EguiContext, With<PrimaryWindow>>
) {
    let Ok(mut egui_ctx) = egui_ctx_q.get_single_mut() else {
        return;
    };

    let ctx = egui_ctx.get_mut();

    egui::Window
        ::new("Info")
        .auto_sized()
        .title_bar(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::Vec2 {
            x: -ctx.pixels_per_point() * 8.0,
            y: -ctx.pixels_per_point() * 8.0,
        })
        .show(ctx, |ui| {
            ui.set_max_width(ctx.pixels_per_point() * 80.0);

            ui.colored_label(
                egui::Color32::WHITE,
                format!(
                    "Frame count: {}",
                    diagnostics
                        .get(&FrameTimeDiagnosticsPlugin::FPS)
                        .and_then(|fps| fps.smoothed())
                        .map(|fps| (fps as i32).to_string())
                        .unwrap_or(String::from("NaN"))
                )
            );

            ui.separator();

            ui.colored_label(egui::Color32::WHITE, format!("Chunks updated: {}", 0));

            ui.separator();

            ui.colored_label(egui::Color32::WHITE, format!("Pixels updated: {}", 0));
        });
}

fn ui_selected_cell_system(
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_camera: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>,
    registries: Res<Registries>,
    chunk_manager: Res<ChunkManager>,
    mut egui_ctx_q: Query<&mut EguiContext, With<PrimaryWindow>>
) {
    let Ok(mut egui_ctx) = egui_ctx_q.get_single_mut() else {
        return;
    };

    let ctx = egui_ctx.get_mut();

    egui::Window
        ::new("Selected pixel")
        .max_width(ctx.pixels_per_point() * 120.0)
        .title_bar(false)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2 {
            x: ctx.pixels_per_point() * 8.0,
            y: -ctx.pixels_per_point() * 8.0,
        })
        .show(ctx, |ui| {
            // ui.set_max_width(ctx.pixels_per_point() * 80.0);

            let (camera, camera_global_transform) = q_camera.single_mut();
            let window = q_window.single();

            let Some(world_position) = window
                .cursor_position()
                .and_then(|cursor| camera.viewport_to_world(camera_global_transform, cursor))
                .map(|ray| ray.origin.truncate())
                .map(|point| vec2(point.x, point.y))
                .map(|point| {
                    ivec2(
                        (point.x * (CHUNK_SIZE as f32)).round() as i32,
                        (point.y * (CHUNK_SIZE as f32)).round() as i32
                    )
                }) else {
                ui.colored_label(egui::Color32::WHITE, "Position: NaN".to_string());
                return;
            };

            ui.colored_label(
                egui::Color32::WHITE,
                format!("Position: {}, {}", world_position.x, world_position.y)
            );

            ui.colored_label(
                egui::Color32::WHITE,
                format!(
                    "Chunk position: {}, {}",
                    world_position.x.div_euclid(CHUNK_SIZE),
                    world_position.y.div_euclid(CHUNK_SIZE)
                )
            );

            let Some(pixel) = chunk_manager.get(world_position).ok() else {
                return;
            };

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!(
                    "Material name: {}",
                    registries.materials.get(&pixel.id.to_string()).unwrap().id
                )
            );

            ui.separator();

            ui.colored_label(egui::Color32::WHITE, format!("ra: {}", pixel.ra));

            ui.separator();

            ui.colored_label(egui::Color32::WHITE, format!("rb: {}", pixel.rb));

            ui.separator();

            ui.colored_label(egui::Color32::WHITE, format!("updated at: {}", pixel.updated_at));

            // ui.separator();

            // ui.colored_label(
            //     egui::Color32::WHITE,
            //     {
            //         match pixel.simulation {
            //             SimulationType::Ca => "simulation: ca".to_string(),
            //             SimulationType::RigidBody(object_id, cell_id) => format!("simulation: rb({}, {})", object_id, cell_id),
            //             SimulationType::Displaced(dx, dy) => format!("simulation: displaced({}, {})", dx, dy),
            //         }
            //     }
            // );

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!("Physics type: {}", pixel.physics_type.to_string())
            );

            // if let Some(fire_parameters) = &pixel.material.fire_parameters {
            //     ui.separator();

            //     ui.colored_label(
            //         egui::Color32::WHITE,
            //         format!("temperature: {}", pixel.temperature)
            //     );

            //     ui.colored_label(egui::Color32::WHITE, format!("burning: {}", pixel.on_fire));

            //     ui.colored_label(
            //         egui::Color32::WHITE,
            //         format!("fire_hp: {}", fire_parameters.fire_hp)
            //     );

            //     ui.colored_label(
            //         egui::Color32::WHITE,
            //         format!("fire temperature: {}", fire_parameters.fire_temperature)
            //     );

            //     ui.colored_label(
            //         egui::Color32::WHITE,
            //         format!("ignition temperature: {}", fire_parameters.ignition_temperature)
            //     );
            // }
        });
}

fn ui_painter_system(
    brush: Option<ResMut<BrushRes>>,
    object_buffer: Option<ResMut<PainterObjectBuffer>>,
    registries: Res<Registries>,
    mut egui_ctx_q: Query<&mut EguiContext, With<PrimaryWindow>>
) {
    let Ok(mut egui_ctx) = egui_ctx_q.get_single_mut() else {
        return;
    };

    let ctx = egui_ctx.get_mut();

    egui::Window
        ::new("Elements")
        .default_open(brush.is_some())
        .auto_sized()
        .title_bar(false)
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2 {
            x: -ctx.pixels_per_point() * 8.0,
            y: ctx.pixels_per_point() * 8.0,
        })
        .show(ctx, |ui| {
            let mut brush = brush.unwrap();
            ui.set_max_width(ctx.pixels_per_point() * 80.0);

            let mut elements = registries.materials.values().cloned().collect::<Vec<Material>>();

            elements.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));

            let mut empty = true;
            for material in elements.into_iter() {
                if !empty {
                    ui.separator();
                } else {
                    empty = false;
                }

                let color = material.color;

                let (rect, response) = ui.allocate_exact_size(
                    egui::Vec2 {
                        x: ui.available_width(),
                        y: ctx.pixels_per_point() * 16.0,
                    },
                    egui::Sense {
                        click: true,
                        drag: false,
                        focusable: true,
                    }
                );

                ui.allocate_ui_at_rect(rect, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.horizontal_top(|ui| {
                            ui.add_space(ctx.pixels_per_point() * 4.0);

                            let (rect, _) = ui.allocate_exact_size(
                                egui::Vec2 {
                                    x: ctx.pixels_per_point() * 12.0,
                                    y: ctx.pixels_per_point() * 12.0,
                                },
                                egui::Sense {
                                    click: false,
                                    drag: false,
                                    focusable: false,
                                }
                            );

                            ui.painter().rect_filled(
                                rect,
                                egui::Rounding::default().at_most(0.5),
                                egui::Color32::from_rgba_unmultiplied(
                                    color[0],
                                    color[1],
                                    color[2],
                                    color[3]
                                )
                            );

                            if brush.material.as_ref() == Some(&material) {
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
                                            if brush.material.as_ref() == Some(&material) {
                                                egui::Color32::GOLD
                                            } else {
                                                egui::Color32::WHITE
                                            }
                                        },
                                        material.id.to_string()
                                    );
                                });
                            });
                        });
                    })
                });

                if response.clicked() {
                    brush.material = Some(material.clone());
                }
            }

            ui.add_space(ctx.pixels_per_point() * 8.0);

            egui::ComboBox
                ::from_label("Shape")
                .selected_text(match brush.shape {
                    BrushShape::Circle => "Circle",
                    BrushShape::Square => "Square",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut brush.shape, BrushShape::Square, "Square");
                    ui.selectable_value(&mut brush.shape, BrushShape::Circle, "Circle");
                });

            ui.add_space(ctx.pixels_per_point() * 8.0);

            ui.label("Brush size");

            ui.add(
                egui::widgets::Slider
                    ::new(&mut brush.size, 2..=32)
                    .show_value(true)
                    .trailing_fill(true)
            );

            ui.add_space(ctx.pixels_per_point() * 8.0);

            egui::ComboBox
                ::from_label("Type")
                .selected_text(match brush.brush_type {
                    BrushType::Cell => "Cell",
                    BrushType::Object => "Object",
                    BrushType::Particle(_) => "Particle",
                })
                .show_ui(ui, |ui| {
                    if let Some(mut object_buffer) = object_buffer {
                        object_buffer.map.clear();
                    }

                    ui.selectable_value(&mut brush.brush_type, BrushType::Cell, "Cell");
                    ui.selectable_value(&mut brush.brush_type, BrushType::Particle(1), "Particle");
                    ui.selectable_value(&mut brush.brush_type, BrushType::Object, "Object");
                });

            if let BrushType::Particle(size) = &mut brush.brush_type {
                ui.add_space(ctx.pixels_per_point() * 8.0);

                ui.label("Particle spawn rate");

                ui.add(
                    egui::widgets::Slider
                        ::new(size, 1..=25)
                        .show_value(true)
                        .trailing_fill(true)
                );
            }
        });
}

#[derive(Clone, Component)]
pub struct Cell {
    pub id: Id,
    pub texture: Option<egui::TextureHandle>,
    pub object: Object,
}

const INVENTORY_ROWS: usize = 2;
const INVENTORY_COLUMNS: usize = 4;
const INVENTORY_SLOTS: usize = INVENTORY_ROWS * INVENTORY_COLUMNS;

#[derive(Resource)]
pub struct Inventory {
    pub cells: [Option<Cell>; INVENTORY_SLOTS],
}

impl FromWorld for Inventory {
    fn from_world(world: &mut World) -> Self {
        let mut initial_cells = vec![];

        for _ in 0..INVENTORY_SLOTS - initial_cells.len() {
            initial_cells.push(None);
        }

        Self {
            cells: initial_cells.try_into().ok().unwrap(),
        }
    }
}

fn bilinear_filtering(image: &[[u8; 4]], position: Vec2, width: i32, height: i32) -> [u8; 4] {
    let position = position
        .round()
        .as_ivec2()
        .clamp(IVec2::ZERO, IVec2::new(width - 1, height - 1));

    image[(position.y * width + position.x) as usize]

    // let top_left = position.floor().as_ivec2().clamp(IVec2::ZERO, IVec2::new(width - 1, height - 1));
    // let bottom_right = (top_left + IVec2::ONE).clamp(IVec2::ZERO, IVec2::new(width - 1, height - 1));

    // let frac = position - position.floor();

    // let value_top_left = image[(top_left.y * width + top_left.x) as usize];
    // let value_top_right = image[(top_left.y * width + bottom_right.x) as usize];
    // let value_bottom_left = image[(bottom_right.y * width + top_left.x) as usize];
    // let value_bottom_right = image[(bottom_right.y * width + bottom_right.x) as usize];

    // let mut color = [0, 0, 0, 255];

    // color[0..3]
    //     .iter_mut()
    //     .enumerate()
    //     .for_each(|(index, value)| {
    //         *value += (
    //             (value_top_left[index] as f32) *
    //             (1.0 - frac.x) *
    //             (1.0 - frac.y)
    //         ).floor() as u8;
    //         *value += ((value_top_right[index] as f32) * frac.x * (1.0 - frac.y)).floor() as u8;
    //         *value += ((value_bottom_left[index] as f32) * (1.0 - frac.x) * frac.y).floor() as u8;
    //         *value += ((value_bottom_right[index] as f32) * frac.x * frac.y).floor() as u8;
    //     });

    // color
}

fn ui_inventory_system(
    mut commands: Commands,
    mut inventory: ResMut<Inventory>,
    mut events: EventWriter<ToastEvent>,
    window_q: Query<(Entity, &Window), With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>,
    mut egui_ctx_q: Query<&mut EguiContext, With<PrimaryWindow>>
) {
    let Ok(mut egui_ctx) = egui_ctx_q.get_single_mut() else {
        return;
    };

    let ctx = egui_ctx.get_mut();

    let (_window_entity, window) = window_q.single();
    let (camera, camera_global_transform) = camera_q.single();

    egui::Window
        ::new("inventory")
        .auto_sized()
        .title_bar(false)
        .anchor(egui::Align2::LEFT_BOTTOM, [
            ctx.pixels_per_point() * 8.0,
            -ctx.pixels_per_point() * 8.0,
        ])
        .show(ctx, |ui| {
            egui::Grid
                ::new("inventory_grid")
                .spacing([ctx.pixels_per_point() * 4.0, ctx.pixels_per_point() * 8.0])
                .show(ui, |ui| {
                    let mut to = None;
                    let mut from = None;

                    for (index, cell_option) in inventory.cells.iter_mut().enumerate() {
                        let cell_size = egui::Vec2::new(32.0, 32.0);

                        let (_, payload) = ui.dnd_drop_zone::<usize, ()>(
                            Frame::menu(ui.style()),
                            |ui| {
                                let drag_stopped =
                                    ctx.drag_stopped_id() ==
                                    cell_option.as_ref().map(|cell| cell.id);
                                let over_ui = ctx.is_pointer_over_area();

                                if cell_option.is_none() || (drag_stopped && over_ui) {
                                    let rect = ui.allocate_exact_size(
                                        cell_size,
                                        Sense::click_and_drag()
                                    ).0;
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::Rounding::default().at_most(0.5),
                                        Color32::TRANSPARENT
                                    );

                                    return;
                                }

                                if drag_stopped && !over_ui {
                                    let rect = ui.allocate_exact_size(
                                        cell_size,
                                        Sense::click_and_drag()
                                    ).0;
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::Rounding::default().at_most(0.5),
                                        Color32::TRANSPARENT
                                    );

                                    if let Some(position) = window.cursor_position() {
                                        let point = camera
                                            .viewport_to_world(camera_global_transform, position)
                                            .map(|ray| ray.origin.truncate())
                                            .unwrap();

                                        let collider_result = cell_option
                                            .as_ref()
                                            .unwrap()
                                            .object.create_collider();

                                        if let Ok(collider) = collider_result {
                                            let cell = cell_option.take().unwrap();
                                            commands.spawn(ObjectBundle {
                                                object: cell.object,
                                                collider,
                                                transform: TransformBundle {
                                                    local: Transform::from_translation(
                                                        point.extend(0.0)
                                                    ),
                                                    ..Default::default()
                                                },
                                                mass_properties: ColliderMassProperties::Density(
                                                    2.0
                                                ),
                                                ..Default::default()
                                            });
                                        }

                                        events.send(ToastEvent {
                                            content: "Dropped in world".to_string(),
                                            level: egui_notify::ToastLevel::Info,
                                            duration: Duration::from_secs(2),
                                        });
                                    } else {
                                        events.send(ToastEvent {
                                            content: "Position is out of bounds".to_string(),
                                            level: egui_notify::ToastLevel::Error,
                                            duration: Duration::from_secs(2),
                                        });
                                    }

                                    return;
                                }

                                let cell = cell_option.as_mut().unwrap();
                                ui.dnd_drag_source(cell.id, index, |ui| {
                                    let texture_size = cell.object.size.max_element() as usize;

                                    let texture = cell.texture.get_or_insert_with(|| {
                                        let x_offset =
                                            (texture_size - (cell.object.size.x as usize)) / 2;
                                        let y_offset =
                                            (texture_size - (cell.object.size.y as usize)) / 2;

                                        let mut colors = vec![0; texture_size.pow(2) * 4 as usize];

                                        cell.object.pixels
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, pixel)| pixel.is_some())
                                            .map(|(index, pixel)| (index, pixel.as_ref().unwrap()))
                                            .for_each(|(pixel_index, pixel)| {
                                                let y = pixel_index / (cell.object.size.x as usize);
                                                let x = pixel_index % (cell.object.size.x as usize);

                                                let index =
                                                    (((cell.object.size.y as usize) -
                                                        (y + 1) +
                                                        y_offset) *
                                                        texture_size +
                                                        x +
                                                        x_offset) *
                                                    4;

                                                colors[index..index + 4].copy_from_slice(
                                                    &pixel.get_color()
                                                )
                                            });

                                        ui.ctx().load_texture(
                                            cell.id.value().to_string(),
                                            egui::ColorImage::from_rgba_unmultiplied(
                                                [texture_size, texture_size],
                                                &colors
                                            ),
                                            TextureOptions::NEAREST
                                        )
                                    });

                                    let cell_rect = ui.allocate_exact_size(
                                        cell_size,
                                        Sense::click_and_drag()
                                    ).0;

                                    ui.painter().image(
                                        texture.id(),
                                        cell_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0)
                                        ),
                                        egui::Color32::WHITE
                                    );
                                });
                            }
                        );

                        if let Some(dropped_index) = payload {
                            to = Some(index);
                            from = Some(*dropped_index);
                        }

                        if index % INVENTORY_COLUMNS == INVENTORY_COLUMNS - 1 {
                            ui.end_row();
                        }
                    }

                    if to.is_some() && from.is_some() {
                        inventory.cells.swap(to.unwrap(), from.unwrap());
                    }
                });
        });
}
