use bevy_egui::{egui, EguiContext, EguiContexts};
use bevy_math::{ivec2, vec2};

use bevy::{diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin}, prelude::*, window::PrimaryWindow};

use crate::{constants::CHUNK_SIZE, materials::{Material, PhysicsType}, painter::{BrushRes, BrushShape, BrushType}, pixel::SimulationType, registries::Registries, world::ChunkManager};

#[derive(Resource)]
pub struct UiWidgets {
    pub menu_bar_open: bool,
    pub info_open: bool,
    pub elements_open: bool,
    pub cell_info_open: bool,
}

impl FromWorld for UiWidgets {
    fn from_world(_: &mut World) -> Self {
        Self {
            menu_bar_open: true,
            info_open: true,
            elements_open: true,
            cell_info_open: true,
        }        
    }
}

pub fn egui_has_primary_context(
    query: Query<&EguiContext, With<PrimaryWindow>>,
) -> bool {
    !query.is_empty()
}

pub fn ui_info_system(
    mut contexts: EguiContexts,
    mut widgets: ResMut<UiWidgets>,
    diagnostics: Res<DiagnosticsStore>,
) {
    let ctx = contexts.ctx_mut();

    egui::Window::new("Info")
        .open(&mut widgets.info_open)
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
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::Vec2 { x: - ctx.pixels_per_point() * 8.0, y: - ctx.pixels_per_point() * 8.0 })
        .show(ctx, |ui| {
            ui.set_max_width(ctx.pixels_per_point() * 80.0);

            ui.colored_label(
                egui::Color32::WHITE,
                format!("Frame count: {}", diagnostics
                    .get(&FrameTimeDiagnosticsPlugin::FPS)
                    .and_then(|fps| fps.smoothed())
                    .map(|fps| fps.to_string())
                    .unwrap_or(String::from("NaN"))
                )
            );

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!("Chunks updated: {}", 0)
            );

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!("Pixels updated: {}", 0)
            );
        });
}

pub fn ui_selected_cell_system(
    mut contexts: EguiContexts,
    mut widgets: ResMut<UiWidgets>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_camera: Query<(&Camera, &GlobalTransform), With<Camera>>,
    registries: Res<Registries>,
    chunk_manager: Res<ChunkManager>,
) {
    let ctx = contexts.ctx_mut();

    egui::Window::new("Selected pixel")
        .open(&mut widgets.cell_info_open)
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

            let (camera, camera_global_transform) = q_camera.single_mut();
            let window = q_window.single();

            let Some(world_position) = window.cursor_position()
                .and_then(|cursor| camera.viewport_to_world(camera_global_transform, cursor))
                .map(|ray| ray.origin.truncate())
                .map(|point| vec2(point.x, point.y))
                .map(|point| {
                    ivec2(
                        (point.x * CHUNK_SIZE as f32).round() as i32,
                        (point.y * CHUNK_SIZE as f32).round() as i32,
                    )
                })
            else {
                ui.colored_label(
                    egui::Color32::WHITE,
                    "Position: NaN".to_string()
                );
                return;
            };
            
            ui.colored_label(
                egui::Color32::WHITE,
                format!("Position: {}, {}", world_position.x, world_position.y)
            );

            
            let Some(pixel) = chunk_manager.get(world_position) else {
                return;
            };

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!("Material name: {}", registries.materials.get(&pixel.material.id.to_string()).unwrap().id)
            );

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!("ra: {}", pixel.ra)
            );

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!("rb: {}", pixel.rb)
            );

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!("updated at: {}", pixel.updated_at)
            );

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                {
                    match pixel.simulation {
                        SimulationType::Ca => "simulation: ca".to_string(),
                        SimulationType::RigidBody(object_id, cell_id) => format!("simulation: rb({}, {})", object_id, cell_id),
                        SimulationType::Displaced(dx, dy) => format!("simulation: displaced({}, {})", dx, dy),
                    }
                }
            );

            ui.separator();

            ui.colored_label(
                egui::Color32::WHITE,
                format!("Matter type: {}", pixel.material.matter_type.to_string())
            );

            match pixel.material.matter_type {
                PhysicsType::Liquid(parameters) => {
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
                PhysicsType::Empty => {},
                PhysicsType::Static => {},
                PhysicsType::Powder => {},
                PhysicsType::Gas => {},
            }

            if let Some(fire_parameters) = &pixel.material.fire_parameters {
                ui.separator();

                ui.colored_label(
                    egui::Color32::WHITE,
                    format!("burning: {}", pixel.on_fire)
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

pub fn ui_painter_system(
    mut contexts: EguiContexts,
    mut widgets: ResMut<UiWidgets>,
    mut brush: ResMut<BrushRes>,
    registries: Res<Registries>,
) {
    let ctx = contexts.ctx_mut();
    
    egui::Window::new("Elements")
        .open(&mut widgets.elements_open)
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

            let mut elements = registries.materials.iter().cloned().collect::<Vec<Material>>();

            elements.sort_by(|a, b| {
                a.id.to_lowercase().cmp(&b.id.to_lowercase())
            });

            let mut empty = true;
            for material in elements.into_iter() {
                if !empty {
                    ui.separator();
                }
                else {
                    empty = false;
                }

                let color = material.color;

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
                                            }
                                            else {
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