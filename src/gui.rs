use std::time::Duration;


use bevy_egui::{
    egui::{ self, Color32, Frame, Id, Sense, TextureOptions },
    EguiContext,
    EguiContexts,
};

use bevy::{
    a11y::{ accesskit::{ NodeBuilder, Role }, AccessibilityNode },
    app::AppExit,
    audio::{ PlaybackMode, Volume },
    diagnostic::{ DiagnosticsStore, FrameTimeDiagnosticsPlugin },
    input::mouse::{ MouseScrollUnit, MouseWheel },
    prelude::*,
    window::{ PresentMode, PrimaryWindow, WindowMode },
};
use bevy_math::{ ivec2, vec2 };
use bevy_persistent::Persistent;
use bevy_rapier2d::geometry::ColliderMassProperties;
use bevy_tween::{
    interpolation::EaseFunction,
    span_tween::{ SpanTweenBundle, SpanTweenerBundle },
    tween::{ ComponentTween, TargetComponent },
};
use itertools::Itertools;

use crate::{
    actors::{ health::Health, player::{ InventoryParameters, Player, PlayerMaterials, PlayerSelectedMaterial } }, assets::{
        process_assets,
        AudioAssetCollection,
        FontAssetCollection,
        FontBytes,
        SpriteAssetCollection,
    }, camera::TrackingCamera, constants::CHUNK_SIZE, despawn_component, fade_out_audio, generation::LevelCounter, has_window, interpolator::{InterpolateBackgroundColor, InterpolatePadding, InterpolateTextColor, InterpolateTopOffset}, painter::{ BrushRes, BrushShape, BrushType, PainterObjectBuffer }, registries::Registries, settings::{ Config, Scoreboard }, simulation::{
        chunk_manager::ChunkManager,
        materials::Material,
        object::{ get_object_by_click, Object, ObjectBundle },
    }, state::GameState
};

pub struct GuiPlugin;
#[derive(Resource)]
pub struct Score {
    pub value: i32,
    pub timer: Timer,
}

impl Default for Score {
    fn default() -> Self {
        Self {
            value: 0,
            timer: Timer::new(Duration::from_secs(2), TimerMode::Repeating),
        }
    }
}

fn write_score(score: Res<Score>, level: Res<LevelCounter>, mut scoreboard: ResMut<Persistent<Scoreboard>>) {
    scoreboard.scores.push((level.0 as i32, score.value));
    scoreboard.persist().expect("failed to update scoreboard");
}

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MenuState>()
            .init_resource::<Inventory>()
            .add_systems(OnExit(GameState::LoadingAssets), setup_egui.after(process_assets))
            .add_systems(OnEnter(GameState::Game), setup_in_game_interface)
            .add_systems(OnExit(GameState::Game), despawn_component::<UiBars>)
            .add_systems(OnEnter(GameState::GameOver), (
                despawn_component::<UiHealthBar>,
                despawn_component::<UiMaterials>,
                game_over_splash,
                write_score
            ))
            .add_systems(OnExit(GameState::GameOver), despawn_component::<UiGameOver>)
            .add_systems(OnEnter(GameState::Setup), move |mut commands: Commands|
                commands.insert_resource(Score::default())
            )
            .add_systems(Update, tick_score.run_if(in_state(GameState::Game)))
            .add_systems(Update, game_over_button.run_if(in_state(GameState::GameOver)))
            .add_systems(
                Update,
                (
                    ui_info_system,
                    // ui_selected_cell_system,
                    ui_painter_system,
                    // ui_inventory_system,
                    get_object_by_click,
                )
                    .run_if(has_window)
                    .run_if(egui_has_primary_context)
                    .run_if(in_state(GameState::Game))
            )
            .add_systems(
                Update,
                (synchonize_health_value, synchonize_materials).run_if(in_state(GameState::Game))
            )
            .add_systems(
                Update,
                (
                    button_style_system,
                    menu_action,
                    mouse_scroll,
                    button_next_option_scroll,
                    button_next_option,
                ).run_if(in_state(GameState::Menu))
            )
            .add_systems(OnEnter(GameState::Menu), setup_menu)
            .add_systems(OnExit(GameState::Menu), fade_out_audio::<UiTrack>)
            .add_systems(OnEnter(MenuState::Main), setup_main_menu)
            .add_systems(OnExit(MenuState::Main), despawn_component::<UiMainMenu>)
            .add_systems(OnEnter(MenuState::Settings), setup_settings)
            .add_systems(OnExit(MenuState::Settings), despawn_component::<UiSettings>);
    }
}

pub fn tick_score(mut score: ResMut<Score>, time: Res<Time>) {
    score.timer.tick(time.delta());
    if score.timer.finished() {
        score.value = i32::max(score.value - 1, 0);
        score.timer.reset();
    }
}

fn button_style_system(
    mut commands: Commands,
    mut interaction_query: Query<
        (Entity, &Style, &Interaction, &Children),
        (Changed<Interaction>, With<Button>)
    >,
    mut text_query: Query<(Entity, &Text)>,
    audio_assets: Res<AudioAssetCollection>,
) {
    for (entity, style, interaction, children) in &mut interaction_query {
        let (text_entity, text) = text_query.get_mut(children[0]).unwrap();
        let offset = match style.padding.left {
            Val::Px(offset) => offset,
            _ => 0.0,
        };

        match *interaction {
            Interaction::Pressed => {
                commands.spawn(AudioBundle {
                    source: audio_assets.button_click.clone(),
                    settings: PlaybackSettings::DESPAWN,
                });
            }
            Interaction::Hovered => {
                commands
                    .entity(text_entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                    .insert(
                        ComponentTween::new(InterpolateTextColor {
                            start: text.sections[0].style.color,
                            end: Color::Rgba {
                                red: (0xf2 as f32) / 255.0,
                                green: (0xf1 as f32) / 255.0,
                                blue: (0xa3 as f32) / 255.0,
                                alpha: 1.0,
                            },
                        })
                    );

                // f2f1a3

                commands
                    .entity(entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                    .insert(AudioBundle {
                        source: audio_assets.button_select.clone(),
                        settings: PlaybackSettings::REMOVE,
                    })
                    .insert(
                        ComponentTween::new(InterpolatePadding {
                            start: [offset, 0.0, 0.0, 0.0],
                            end: [10.0, 0.0, 0.0, 0.0],
                        })
                    );
            }
            Interaction::None => {
                commands
                    .entity(text_entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                    .insert(
                        ComponentTween::new(InterpolateTextColor {
                            start: text.sections[0].style.color,
                            end: Color::Rgba {
                                red: 0.75,
                                green: 0.75,
                                blue: 0.75,
                                alpha: 1.0,
                            },
                        })
                    );

                commands
                    .entity(entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                    .insert(
                        ComponentTween::new(InterpolatePadding {
                            start: [offset, 0.0, 0.0, 0.0],
                            end: [0.0, 0.0, 0.0, 0.0],
                        })
                    );
            }
        }
    }
}

#[derive(Component, Default)]
struct ScrollingList {
    position: f32,
}

fn mouse_scroll(
    mut commands: Commands,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut query_list: Query<(Entity, &mut ScrollingList, &mut Style, &Parent, &Node, &Interaction)>,
    query_node: Query<&Node>
) {
    for mouse_wheel_event in mouse_wheel_events.read() {
        for (entity, mut scrolling_list, style, parent, list_node, interaction) in &mut query_list {
            if *interaction != Interaction::Hovered {
                continue;
            }

            let items_height = list_node.size().y;
            let container_height = query_node.get(parent.get()).unwrap().size().y;

            let max_scroll = (items_height - container_height).max(0.0);

            let dy = match mouse_wheel_event.unit {
                MouseScrollUnit::Line => mouse_wheel_event.y * 50.0,
                MouseScrollUnit::Pixel => mouse_wheel_event.y,
            };

            scrolling_list.position += dy;
            scrolling_list.position = scrolling_list.position.clamp(-max_scroll, 0.0);

            commands
                .entity(entity)
                .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                .insert(
                    ComponentTween::new_target(
                        TargetComponent::tweener_entity(),
                        InterpolateTopOffset {
                            start: style.top,
                            end: Val::Px(scrolling_list.position),
                        }
                    )
                );
        }
    }
}

#[derive(Component)]
pub struct UiBars;

#[derive(Component)]
pub struct UiHealthBar;

#[derive(Component)]
pub struct UiMaterials;

fn synchonize_materials(
    registries: Res<Registries>,
    selected_material: Res<PlayerSelectedMaterial>,
    inventory_q: Query<&InventoryParameters, With<Player>>,
    mut stored_materials: ResMut<PlayerMaterials>,
    mut style_q: Query<(&mut Style, &mut BackgroundColor), With<UiMaterials>>
) {
    let (mut style, mut color) = style_q.single_mut();
    let Ok(inventory) = inventory_q.get_single() else {
        return;
    };

    if selected_material.is_changed() || stored_materials.is_changed() {
        let id = selected_material.0.clone();
        let value = *stored_materials.entry(selected_material.0.clone()).or_insert(0.0) / inventory.max_storage * 100.0;

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
    mut health_bar: Query<&mut Style, With<UiHealthBar>>
) {
    let health = player_q.single();
    let mut style = health_bar.single_mut();

    style.width = Val::Percent((health.current.max(0.0) / health.total) * 100.0);
}

fn setup_in_game_interface(mut commands: Commands, sprites: Res<SpriteAssetCollection>) {
    let slicer = TextureSlicer {
        border: BorderRect::square(10.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 1.0,
    };

    commands
        .spawn((
            UiBars,
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Auto,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(20.0),
                    margin: UiRect::all(Val::Px(20.0)),
                    ..default()
                },
                ..default()
            },
        ))
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
                            image: sprites.in_game_border.clone().into(),
                            ..default()
                        },
                        ImageScaleMode::Sliced(slicer.clone()),
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            UiHealthBar,
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
                                image: sprites.in_game_border.clone().into(),
                                ..default()
                            },
                            ImageScaleMode::Sliced(slicer.clone()),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                UiMaterials,
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
                });
        });
}

// State used for the current menu screen
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
enum MenuState {
    Main,
    Settings,
    #[default]
    Disabled,
}

#[derive(Component)]
enum MenuButtonAction {
    Play,
    Settings,
    ApplySettings,
    BackToMainMenu,
    Quit,
}

fn menu_action(
    interaction_query: Query<
        (&Interaction, &MenuButtonAction),
        (Changed<Interaction>, With<Button>)
    >,
    mut app_exit_events: EventWriter<AppExit>,
    mut menu_state: ResMut<NextState<MenuState>>,
    mut game_state: ResMut<NextState<GameState>>,
    mut config: ResMut<Persistent<Config>>,
    display_index_q: Query<&UiOptions>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    mut audio_sink_q: Query<&mut AudioSink>,
    mut global_volume: ResMut<GlobalVolume>,
) {
    for (interaction, menu_button_action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match menu_button_action {
                MenuButtonAction::Quit => {
                    app_exit_events.send(AppExit);
                }
                MenuButtonAction::Play => {
                    game_state.set(GameState::Setup);
                    menu_state.set(MenuState::Disabled);
                }
                MenuButtonAction::Settings => menu_state.set(MenuState::Settings),
                MenuButtonAction::BackToMainMenu => menu_state.set(MenuState::Main),
                MenuButtonAction::ApplySettings => {
                    let mut window = window_q.single_mut();

                    for display_index in display_index_q.iter() {
                        match display_index {
                            UiOptions::Mode(index) => {
                                config.mode = ALLOWED_WINDOW_MODES[*index].0;
                            }
                            UiOptions::VSync(index) => {
                                config.vsync = ALLOWED_VSYNC_MODES[*index].0;
                            }
                            UiOptions::Resolution(index) => {
                                config.resolution = ALLOWED_RESOLUTIONS[*index];
                            }
                            UiOptions::Volume(value) => {
                                config.volume = *value;
                            }
                            UiOptions::Spatial(value) => {
                                config.spatial = *value;
                            }
                        }
                    }

                    config.persist().expect("failed to update config");

                    window.resolution.set(config.resolution[0] as f32, config.resolution[1] as f32);
                    window.resolution.set_scale_factor_override(
                        Some((config.resolution[0] as f32) / 1280.0)
                    );
                    window.mode = config.mode.clone();
                    window.present_mode = config.vsync.clone();

                    let volume = ((config.volume as f32) / 100.0).clamp(0.0, 100.0);
                    global_volume.volume = Volume::new(volume);
                
                    for audio_sink in audio_sink_q.iter_mut() {
                        audio_sink.set_volume(volume);
                    }
                }
            }
        }
    }
}

#[derive(Component)]
pub struct UiSettings;

#[derive(Component)]
pub struct UiMainMenu;

#[derive(Component)]
pub struct UiTrack;

fn setup_menu(
    mut commands: Commands,
    mut menu_state: ResMut<NextState<MenuState>>,
    audios: Res<AudioAssetCollection>
) {
    commands.spawn((
        UiTrack,
        AudioBundle {
            source: audios.menu.clone(),
            settings: PlaybackSettings {
                mode: PlaybackMode::Loop,
                ..Default::default()
            },
        },
    ));

    menu_state.set(MenuState::Main);
}

fn setup_main_menu(
    mut commands: Commands,
    sprites: Res<SpriteAssetCollection>,
    scoreboard: Res<Persistent<Scoreboard>>
) {
    let border_slicer = TextureSlicer {
        border: BorderRect::square(13.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 1.0,
    };

    commands
        .spawn((
            UiMainMenu,
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(64.0)),
                    column_gap: Val::Px(32.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                background_color: Color::BLACK.into(),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(300.0),
                        max_width: Val::Px(300.0),
                        min_width: Val::Px(150.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Stretch,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(20.0),
                        flex_shrink: 1.0,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        TextBundle::from_section("SandForge", TextStyle {
                            font_size: 40.0,
                            color: Color::WHITE,
                            ..Default::default()
                        }).with_text_justify(JustifyText::Left),
                    ));

                    for (action, text) in [
                        (MenuButtonAction::Play, "Start"),
                        (MenuButtonAction::Settings, "Settings"),
                        (MenuButtonAction::Quit, "Exit"),
                    ] {
                        parent
                            .spawn((
                                action,
                                ButtonBundle {
                                    style: Style {
                                        justify_content: JustifyContent::Start,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    background_color: Color::NONE.into(),
                                    ..default()
                                },
                                EaseFunction::ExponentialOut,
                                SpanTweenBundle::new(..Duration::from_millis(250)),
                            ))
                            .with_children(|parent| {
                                parent.spawn((
                                    TextBundle::from_section(text, TextStyle {
                                        font_size: 32.0,
                                        color: Color::WHITE,
                                        ..Default::default()
                                    }),
                                    EaseFunction::ExponentialOut,
                                    SpanTweenBundle::new(..Duration::from_millis(250)),
                                ));
                            });
                    }
                });

            parent
                .spawn((
                    ImageBundle {
                        style: Style {
                            width: Val::Auto,
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceAround,
                            align_items: AlignItems::Center,
                            flex_grow: 1.0,
                            ..default()
                        },
                        image: sprites.border.clone().into(),
                        ..default()
                    },
                    ImageScaleMode::Sliced(border_slicer.clone()),
                ))
                .with_children(|parent| {
                    parent
                        .spawn(NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .with_children(|parent| {
                            parent.spawn(ImageBundle {
                                style: Style {
                                    top: Val::Px(41.0),
                                    width: Val::Px(96.0),
                                    height: Val::Px(14.0),
                                    ..Default::default()
                                },
                                image: sprites.help_divider.clone().into(),
                                transform: Transform::from_rotation(
                                    Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)
                                ),
                                ..Default::default()
                            });
                            parent.spawn(ImageBundle {
                                style: Style {
                                    top: Val::Px(-41.0),
                                    width: Val::Px(96.0),
                                    height: Val::Px(14.0),
                                    ..Default::default()
                                },
                                image: sprites.help_divider.clone().into(),
                                transform: Transform::from_rotation(
                                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)
                                ),
                                ..Default::default()
                            });
                        });

                    parent
                        .spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(50.0),
                                height: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(14.0)),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .with_children(|parent| {
                            parent
                                .spawn(NodeBundle {
                                    style: Style {
                                        flex_direction: FlexDirection::Column,
                                        align_self: AlignSelf::Stretch,
                                        overflow: Overflow::clip_y(),
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                })
                                .with_children(|parent| {
                                    parent
                                        .spawn((
                                            NodeBundle {
                                                style: Style {
                                                    flex_direction: FlexDirection::Column,
                                                    align_items: AlignItems::Center,
                                                    top: Val::Px(0.0),
                                                    row_gap: Val::Px(8.0),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            Interaction::default(),
                                            ScrollingList::default(),
                                            EaseFunction::ExponentialOut,
                                            SpanTweenerBundle::new(Duration::from_secs(1)),
                                            SpanTweenBundle::new(..Duration::from_secs(1)),
                                            AccessibilityNode(NodeBuilder::new(Role::List)),
                                        ))
                                        .with_children(|parent| {
                                            parent.spawn(TextBundle {
                                                style: Style {
                                                    height: Val::Auto,
                                                    align_self: AlignSelf::Center,
                                                    justify_self: JustifySelf::Center,
                                                    ..Default::default()
                                                },
                                                text: Text::from_section(
                                                    "  Game Controls ",
                                                    TextStyle {
                                                        font_size: 24.0,
                                                        color: Color::WHITE,
                                                        ..Default::default()
                                                    }
                                                ).with_justify(JustifyText::Center),
                                                ..Default::default()
                                            });
                                            parent.spawn(TextBundle {
                                                style: Style {
                                                    width: Val::Percent(100.0),
                                                    height: Val::Auto,
                                                    ..Default::default()
                                                },
                                                text: Text::from_section(
                                                    "Movement\n\n- Run: Use the A and D keys to move left and right, respectively.\n- Crouch: Press the S key to crouch.\n\nActions\n\n- Jump: Press the Spacebar to make your character jump.\n- Attack: Press the F key to perform an attack.\n- Dash: Use the Q key to dash forward quickly.\n- Hook: Click the right mouse button to use the hook.\n- Shoot: Use the R key to shoot.\n- Collect: Press the G key to collect materials.\n\nMaterial Selection\n\n- Next Material: Scroll the mouse wheel up to cycle to the next material.\n- Previous Material: Scroll the mouse wheel down to cycle to the previous material.",
                                                    TextStyle {
                                                        font_size: 18.0,
                                                        color: Color::WHITE,
                                                        ..Default::default()
                                                    }
                                                ),
                                                ..Default::default()
                                            });
                                        });
                                });
                        });

                    parent
                        .spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(50.0),
                                height: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::Stretch,
                                align_items: AlignItems::Center,
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .with_children(|parent| {
                            parent.spawn(TextBundle {
                                style: Style {
                                    // width: Val::Auto,
                                    // height: Val::Auto,
                                    top: Val::Px(16.0),
                                    ..Default::default()
                                },
                                text: Text::from_section("  SCOREBOARD ", TextStyle {
                                    font_size: 24.0,
                                    color: Color::WHITE,
                                    ..Default::default()
                                }).with_justify(JustifyText::Center),
                                ..Default::default()
                            });

                            parent
                                .spawn(NodeBundle {
                                    style: Style {
                                        justify_content: JustifyContent::SpaceBetween,
                                        width: Val::Percent(100.0),
                                        padding: UiRect::horizontal(Val::Px(8.0)),
                                        ..default()
                                    },
                                    ..default()
                                })
                                .with_children(|parent| {
                                    parent.spawn(ImageBundle {
                                        style: Style {
                                            width: Val::Px(96.0),
                                            height: Val::Px(14.0),
                                            ..Default::default()
                                        },
                                        image: sprites.help_divider_horizontal.clone().into(),
                                        transform: Transform::from_rotation(
                                            Quat::from_rotation_z(std::f32::consts::PI)
                                        ),
                                        ..Default::default()
                                    });
                                    parent.spawn(ImageBundle {
                                        style: Style {
                                            // left: Val::Px(1.0),
                                            width: Val::Px(96.0),
                                            height: Val::Px(14.0),
                                            ..Default::default()
                                        },
                                        image: sprites.help_divider_horizontal.clone().into(),
                                        ..Default::default()
                                    });
                                });

                            parent
                                .spawn(NodeBundle {
                                    style: Style {
                                        flex_direction: FlexDirection::Column,
                                        align_self: AlignSelf::Stretch,
                                        overflow: Overflow::clip_y(),
                                        width: Val::Percent(100.0),
                                        height: Val::ZERO,
                                        margin: UiRect::vertical(Val::Px(14.0)),
                                        padding: UiRect::horizontal(Val::Px(14.0)),
                                        flex_grow: 1.0,
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                })
                                .with_children(|parent| {
                                    parent
                                        .spawn((
                                            NodeBundle {
                                                style: Style {
                                                    flex_direction: FlexDirection::Column,
                                                    align_items: AlignItems::Center,
                                                    top: Val::Px(0.0),
                                                    row_gap: Val::Px(8.0),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            Interaction::default(),
                                            ScrollingList::default(),
                                            EaseFunction::ExponentialOut,
                                            SpanTweenerBundle::new(Duration::from_secs(1)),
                                            SpanTweenBundle::new(..Duration::from_secs(1)),
                                            AccessibilityNode(NodeBuilder::new(Role::List)),
                                        ))
                                        .with_children(|parent| {
                                            scoreboard.scores
                                                .iter()
                                                .sorted_by(|(_, score_1), (_, score_2)| score_2.cmp(score_1))
                                                .enumerate()
                                                .for_each(|(index, (level, score))| {
                                                    parent.spawn(TextBundle {
                                                        style: Style {
                                                            width: Val::Percent(100.0),
                                                            height: Val::Auto,
                                                            ..Default::default()
                                                        },
                                                        text: Text::from_section(
                                                            format!(
                                                                "{}. Level {}: {}",
                                                                index + 1,
                                                                level,
                                                                score
                                                            ),
                                                            TextStyle {
                                                                font_size: 18.0,
                                                                color: Color::WHITE,
                                                                ..Default::default()
                                                            }
                                                        ),
                                                        ..Default::default()
                                                    });
                                                });
                                        });
                                });
                        });
                });
        });
}

#[derive(Component)]
pub struct UiWindowModeValue(WindowMode);

#[derive(Debug, Component)]
pub enum UiOptions {
    Mode(usize),
    VSync(usize),
    Resolution(usize),
    Volume(i32),
    Spatial(bool),
}

const ALLOWED_WINDOW_MODES: [(WindowMode, &str); 2] = [
    (WindowMode::Windowed, "Windowed"),
    // (WindowMode::BorderlessFullscreen, "Borderless fullscreen"),
    (WindowMode::SizedFullscreen, "Fullscreen"),
];

const ALLOWED_VSYNC_MODES: [(PresentMode, &str); 2] = [
    (PresentMode::AutoNoVsync, "Off"),
    (PresentMode::AutoVsync, "On"),
];

const ALLOWED_RESOLUTIONS: [[u32; 2]; 5] = [
    [1280, 720],
    [1366, 768],
    [1600, 900],
    [1920, 1080],
    [2560, 1600],
    // [2560, 1440],
    // [3840, 2160],
];

fn setup_settings(
    mut commands: Commands,
    config: ResMut<Persistent<Config>>,
    sprites: Res<SpriteAssetCollection>
) {
    let border_slicer = TextureSlicer {
        border: BorderRect::square(13.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 1.0,
    };

    commands
        .spawn((
            UiSettings,
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(64.0)),
                    column_gap: Val::Px(32.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Stretch,
                    ..default()
                },
                background_color: Color::BLACK.into(),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(300.0),
                        max_width: Val::Px(300.0),
                        min_width: Val::Px(150.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Stretch,
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(20.0),
                        // flex_shrink: 1.0,
                        flex_shrink: 0.0,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn(
                        TextBundle::from_section("Settings", TextStyle {
                            font_size: 40.0,
                            color: Color::WHITE,
                            ..Default::default()
                        }).with_text_justify(JustifyText::Left)
                    );

                    for (action, text) in [
                        (MenuButtonAction::ApplySettings, "Apply"),
                        (MenuButtonAction::BackToMainMenu, "Return"),
                    ] {
                        parent
                            .spawn((
                                action,
                                ButtonBundle {
                                    style: Style {
                                        justify_content: JustifyContent::Start,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    background_color: Color::NONE.into(),
                                    ..default()
                                },
                                EaseFunction::ExponentialOut,
                                SpanTweenBundle::new(..Duration::from_millis(250)),
                            ))
                            .with_children(|parent| {
                                parent.spawn((
                                    TextBundle::from_section(text, TextStyle {
                                        font_size: 32.0,
                                        color: Color::WHITE,
                                        ..Default::default()
                                    }),
                                    EaseFunction::ExponentialOut,
                                    SpanTweenBundle::new(..Duration::from_millis(250)),
                                ));
                            });
                    }
                });

            parent
                .spawn((
                    ImageBundle {
                        style: Style {
                            width: Val::Auto,
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::SpaceAround,
                            align_items: AlignItems::Center,
                            flex_grow: 1.0,
                            ..default()
                        },
                        image: sprites.border.clone().into(),
                        ..default()
                    },
                    ImageScaleMode::Sliced(border_slicer.clone()),
                ))
                .with_children(|parent| {
                    parent
                        .spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(14.0)),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .with_children(|parent| {
                            parent
                                .spawn(NodeBundle {
                                    style: Style {
                                        flex_direction: FlexDirection::Column,
                                        align_self: AlignSelf::Stretch,
                                        overflow: Overflow::clip_y(),
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                })
                                .with_children(|parent| {
                                    parent
                                        .spawn((
                                            NodeBundle {
                                                style: Style {
                                                    flex_direction: FlexDirection::Column,
                                                    align_items: AlignItems::Start,
                                                    top: Val::Px(0.0),
                                                    row_gap: Val::Px(8.0),
                                                    ..default()
                                                },
                                                ..default()
                                            },
                                            Interaction::default(),
                                            ScrollingList::default(),
                                            EaseFunction::ExponentialOut,
                                            SpanTweenerBundle::new(Duration::from_secs(1)),
                                            SpanTweenBundle::new(..Duration::from_secs(1)),
                                            AccessibilityNode(NodeBuilder::new(Role::List)),
                                        ))
                                        .with_children(|parent| {
                                            parent.spawn(TextBundle {
                                                style: Style {
                                                    width: Val::Percent(100.0),
                                                    height: Val::Auto,
                                                    ..Default::default()
                                                },
                                                text: Text::from_section(
                                                    "Display settings: ",
                                                    TextStyle {
                                                        font_size: 18.0,
                                                        color: Color::WHITE,
                                                        ..Default::default()
                                                    }
                                                ),
                                                ..Default::default()
                                            });

                                            parent
                                                .spawn(NodeBundle {
                                                    style: Style {
                                                        width: Val::Percent(100.0),
                                                        margin: UiRect::horizontal(Val::Px(32.0)),
                                                        row_gap: Val::Px(4.0),
                                                        flex_direction: FlexDirection::Column,
                                                        height: Val::Auto,
                                                        ..Default::default()
                                                    },
                                                    ..Default::default()
                                                })
                                                .with_children(|parent| {
                                                    let (mode_index, (mode, mode_text)) =
                                                        ALLOWED_WINDOW_MODES.into_iter()
                                                            .enumerate()
                                                            .find(
                                                                |(_, (mode, _))|
                                                                    *mode == config.mode
                                                            )
                                                            .unwrap();

                                                    let (resolution_index, resolution) =
                                                        ALLOWED_RESOLUTIONS.into_iter()
                                                            .enumerate()
                                                            .find(|(_, resolution)| {
                                                                *resolution == config.resolution
                                                            })
                                                            .unwrap_or((0, ALLOWED_RESOLUTIONS[0]));

                                                    let (vsync_index, (vsync, vsync_text)) =
                                                        ALLOWED_VSYNC_MODES.into_iter()
                                                            .enumerate()
                                                            .find(
                                                                |(_, (mode, _))|
                                                                    *mode == config.vsync
                                                            )
                                                            .unwrap();

                                                    parent
                                                        .spawn((
                                                            UiOptions::Mode(mode_index),
                                                            ButtonBundle {
                                                                style: Style {
                                                                    justify_content: JustifyContent::Start,
                                                                    align_items: AlignItems::Center,
                                                                    ..default()
                                                                },
                                                                background_color: Color::NONE.into(),
                                                                ..default()
                                                            },
                                                            EaseFunction::ExponentialOut,
                                                            SpanTweenBundle::new(
                                                                ..Duration::from_millis(250)
                                                            ),
                                                        ))
                                                        .with_children(|parent| {
                                                            parent.spawn((
                                                                TextBundle::from_sections([
                                                                    TextSection {
                                                                        value: "Window mode: ".into(),
                                                                        style: TextStyle {
                                                                            font_size: 18.0,
                                                                            color: Color::WHITE,
                                                                            ..Default::default()
                                                                        },
                                                                    },

                                                                    TextSection {
                                                                        value: mode_text.into(),
                                                                        style: TextStyle {
                                                                            font_size: 18.0,
                                                                            color: Color::WHITE,
                                                                            ..Default::default()
                                                                        },
                                                                    },
                                                                ]),
                                                                EaseFunction::ExponentialOut,
                                                                SpanTweenBundle::new(
                                                                    ..Duration::from_millis(250)
                                                                ),
                                                            ));
                                                        });

                                                    parent
                                                        .spawn((
                                                            UiOptions::Resolution(resolution_index),
                                                            ButtonBundle {
                                                                style: Style {
                                                                    justify_content: JustifyContent::Start,
                                                                    align_items: AlignItems::Center,
                                                                    ..default()
                                                                },
                                                                background_color: Color::NONE.into(),
                                                                ..default()
                                                            },
                                                            EaseFunction::ExponentialOut,
                                                            SpanTweenBundle::new(
                                                                ..Duration::from_millis(250)
                                                            ),
                                                        ))
                                                        .with_children(|parent| {
                                                            parent.spawn((
                                                                TextBundle::from_sections([
                                                                    TextSection {
                                                                        value: "Resolution: ".into(),
                                                                        style: TextStyle {
                                                                            font_size: 18.0,
                                                                            color: Color::WHITE,
                                                                            ..Default::default()
                                                                        },
                                                                    },

                                                                    TextSection {
                                                                        value: format!(
                                                                            "{}x{}",
                                                                            resolution[0],
                                                                            resolution[1]
                                                                        ),
                                                                        style: TextStyle {
                                                                            font_size: 18.0,
                                                                            color: Color::WHITE,
                                                                            ..Default::default()
                                                                        },
                                                                    },
                                                                ]),
                                                                EaseFunction::ExponentialOut,
                                                                SpanTweenBundle::new(
                                                                    ..Duration::from_millis(250)
                                                                ),
                                                            ));
                                                        });

                                                    parent
                                                        .spawn((
                                                            UiOptions::VSync(vsync_index),
                                                            ButtonBundle {
                                                                style: Style {
                                                                    justify_content: JustifyContent::Start,
                                                                    align_items: AlignItems::Center,
                                                                    ..default()
                                                                },
                                                                background_color: Color::NONE.into(),
                                                                ..default()
                                                            },
                                                            EaseFunction::ExponentialOut,
                                                            SpanTweenBundle::new(
                                                                ..Duration::from_millis(250)
                                                            ),
                                                        ))
                                                        .with_children(|parent| {
                                                            parent.spawn((
                                                                TextBundle::from_sections([
                                                                    TextSection {
                                                                        value: "VSync: ".into(),
                                                                        style: TextStyle {
                                                                            font_size: 18.0,
                                                                            color: Color::WHITE,
                                                                            ..Default::default()
                                                                        },
                                                                    },

                                                                    TextSection {
                                                                        value: vsync_text.into(),
                                                                        style: TextStyle {
                                                                            font_size: 18.0,
                                                                            color: Color::WHITE,
                                                                            ..Default::default()
                                                                        },
                                                                    },
                                                                ]),
                                                                EaseFunction::ExponentialOut,
                                                                SpanTweenBundle::new(
                                                                    ..Duration::from_millis(250)
                                                                ),
                                                            ));
                                                        });
                                                });

                                            parent.spawn(TextBundle {
                                                style: Style {
                                                    width: Val::Percent(100.0),
                                                    height: Val::Auto,
                                                    ..Default::default()
                                                },
                                                text: Text::from_section(
                                                    "Audio settings: ",
                                                    TextStyle {
                                                        font_size: 18.0,
                                                        color: Color::WHITE,
                                                        ..Default::default()
                                                    }
                                                ),
                                                ..Default::default()
                                            });

                                            parent
                                                .spawn(NodeBundle {
                                                    style: Style {
                                                        width: Val::Percent(100.0),
                                                        margin: UiRect::horizontal(Val::Px(32.0)),
                                                        row_gap: Val::Px(4.0),
                                                        flex_direction: FlexDirection::Column,
                                                        height: Val::Auto,
                                                        ..Default::default()
                                                    },
                                                    ..Default::default()
                                                })
                                                .with_children(|parent| {
                                                    parent
                                                        .spawn((
                                                            UiOptions::Volume(config.volume),
                                                            ButtonBundle {
                                                                style: Style {
                                                                    justify_content: JustifyContent::Start,
                                                                    align_items: AlignItems::Center,
                                                                    ..default()
                                                                },
                                                                background_color: Color::NONE.into(),
                                                                ..default()
                                                            },
                                                            EaseFunction::ExponentialOut,
                                                            SpanTweenBundle::new(
                                                                ..Duration::from_millis(250)
                                                            ),
                                                        ))
                                                        .with_children(|parent| {
                                                            parent.spawn((
                                                                TextBundle::from_sections([
                                                                    TextSection {
                                                                        value: "Volume: ".into(),
                                                                        style: TextStyle {
                                                                            font_size: 18.0,
                                                                            color: Color::WHITE,
                                                                            ..Default::default()
                                                                        },
                                                                    },

                                                                    TextSection {
                                                                        value: format!(
                                                                            "{}%",
                                                                            config.volume
                                                                        ),
                                                                        style: TextStyle {
                                                                            font_size: 18.0,
                                                                            color: Color::WHITE,
                                                                            ..Default::default()
                                                                        },
                                                                    },
                                                                ]),
                                                                EaseFunction::ExponentialOut,
                                                                SpanTweenBundle::new(
                                                                    ..Duration::from_millis(250)
                                                                ),
                                                            ));
                                                        });
                                                });
                                        });
                                });
                        });
                });
        });
}

#[derive(Component)]
pub struct UiGameOver;

#[derive(Component)]
pub struct UiGameOverReturnButton;

fn game_over_button(
    mut commands: Commands,
    button_q: Query<
        (Entity, &Style, &Interaction, &Children),
        (Changed<Interaction>, With<UiGameOverReturnButton>)
    >,
    mut game_state: ResMut<NextState<GameState>>,
    mut text_query: Query<(Entity, &Text)>
) {
    for (entity, style, interaction, children) in button_q.iter() {
        let (text_entity, text) = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                game_state.set(GameState::Menu);
            }
            Interaction::Hovered => {
                commands
                    .entity(text_entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)).tween_here())
                    .insert(
                        ComponentTween::new(InterpolateTextColor {
                            start: text.sections[0].style.color,
                            end: Color::Rgba {
                                red: (0xf2 as f32) / 255.0,
                                green: (0xf1 as f32) / 255.0,
                                blue: (0xa3 as f32) / 255.0,
                                alpha: 1.0,
                            },
                        })
                    );
            }
            Interaction::None => {
                commands
                    .entity(text_entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)).tween_here())
                    .insert(
                        ComponentTween::new(InterpolateTextColor {
                            start: text.sections[0].style.color,
                            end: Color::Rgba {
                                red: 0.75,
                                green: 0.75,
                                blue: 0.75,
                                alpha: 1.0,
                            },
                        })
                    );
            }
        }
    }
}

fn game_over_splash(mut commands: Commands, asset_server: Res<AssetServer>, score: Res<Score>) {
    commands
        .spawn((
            UiGameOver,
            EaseFunction::ExponentialOut,
            SpanTweenerBundle::new(Duration::from_millis(1000)).tween_here(),
            ComponentTween::new(InterpolateBackgroundColor {
                start: Color::NONE,
                end: Color::Rgba { red: 0.0, green: 0.0, blue: 0.0, alpha: 0.9 },
            }),
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    column_gap: Val::Px(32.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ..default()
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    NodeBundle {
                        style: Style {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_items: JustifyItems::Center,
                            padding: UiRect::all(Val::Px(64.0)),
                            row_gap: Val::Px(10.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ))
                .with_children(|parent| {
                    parent.spawn((
                        TextBundle {
                            style: Style {
                                ..Default::default()
                            },
                            text: Text::from_section("  GAME OVER ", TextStyle {
                                font_size: 48.0,
                                color: Color::WHITE,
                                ..Default::default()
                            }).with_justify(JustifyText::Center),
                            ..Default::default()
                        },
                        EaseFunction::ExponentialOut,
                        SpanTweenerBundle::new(Duration::from_millis(1500)).tween_here(),
                        ComponentTween::new(InterpolateTextColor {
                            start: Color::NONE,
                            end: Color::Rgba { red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0 },
                        }),
                    ));

                    parent.spawn((
                        TextBundle {
                            style: Style {
                                ..Default::default()
                            },
                            text: Text::from_section(
                                format!("  final score: {} ", score.value),
                                TextStyle {
                                    font_size: 36.0,
                                    color: Color::WHITE,
                                    ..Default::default()
                                }
                            ).with_justify(JustifyText::Center),
                            ..Default::default()
                        },
                        EaseFunction::ExponentialOut,
                        SpanTweenerBundle::new(Duration::from_millis(1500)).tween_here(),
                        ComponentTween::new(InterpolateTextColor {
                            start: Color::NONE,
                            end: Color::Rgba { red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0 },
                        }),
                    ));

                    parent
                        .spawn((
                            UiGameOverReturnButton,
                            ButtonBundle {
                                style: Style {
                                    justify_content: JustifyContent::Start,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                background_color: Color::NONE.into(),
                                ..default()
                            },
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                TextBundle::from_section("  continue ", TextStyle {
                                    font_size: 36.0,
                                    color: Color::NONE,
                                    ..Default::default()
                                }),
                                EaseFunction::ExponentialOut,
                                SpanTweenerBundle::new(Duration::from_millis(1500)).tween_here(),
                                ComponentTween::new(InterpolateTextColor {
                                    start: Color::NONE,
                                    end: Color::Rgba {
                                        red: 0.75,
                                        green: 0.75,
                                        blue: 0.75,
                                        alpha: 1.0,
                                    },
                                }),
                            ));
                        });
                });
        });
}

fn button_next_option(
    mut interaction_query: Query<
        (&mut UiOptions, &Interaction, &Children),
        (Changed<Interaction>, With<Button>)
    >,
    mut text_query: Query<&mut Text>
) {
    for (mut option, interaction, children) in &mut interaction_query {
        let mut text = text_query.get_mut(children[0]).unwrap();
        match *interaction {
            Interaction::Pressed => {
                match option.as_mut() {
                    UiOptions::Mode(index) => {
                        *index = (*index + 1) % ALLOWED_WINDOW_MODES.len();
                        let (mode, string) = ALLOWED_WINDOW_MODES[*index];
                        text.sections[1].value = string.to_owned();
                    }
                    UiOptions::VSync(index) => {
                        *index = (*index + 1) % ALLOWED_VSYNC_MODES.len();
                        let (mode, string) = ALLOWED_VSYNC_MODES[*index];
                        text.sections[1].value = string.to_owned();
                    }
                    UiOptions::Resolution(index) => {
                        *index = (*index + 1) % ALLOWED_RESOLUTIONS.len();
                        let resolution = ALLOWED_RESOLUTIONS[*index];
                        text.sections[1].value = format!("{}x{}", resolution[0], resolution[1]);
                    }
                    UiOptions::Volume(value) => {
                        *value = (*value + 1).clamp(0, 100);
                        text.sections[1].value = format!("{} %", *value);
                    }
                    UiOptions::Spatial(value) => {
                        *value = !*value;
                        text.sections[1].value = format!("{}", match *value {
                            true => "on",
                            false => "off",
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn button_next_option_scroll(
    mut interaction_query: Query<(&mut UiOptions, &Interaction, &Children), With<Button>>,
    mut text_query: Query<&mut Text>,
    mut mouse_wheel_events: EventReader<MouseWheel>
) {
    for ev in mouse_wheel_events.read() {
        let direction = ev.y.signum() as i32;

        for (mut option, interaction, children) in &mut interaction_query {
            if *interaction != Interaction::Hovered {
                continue;
            }

            let mut text = text_query.get_mut(children[0]).unwrap();
            match option.as_mut() {
                UiOptions::Volume(value) => {
                    *value = (*value + direction).clamp(0, 100);
                    text.sections[1].value = format!("{} %", *value);
                }
                _ => {}
            }
        }
    }
}

fn setup_egui(
    mut contexts: EguiContexts,
    fonts: Res<FontAssetCollection>,
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
                    "FPS: {}",
                    diagnostics
                        .get(&FrameTimeDiagnosticsPlugin::FPS)
                        .and_then(|fps| fps.smoothed())
                        .map(|fps| (fps as i32).to_string())
                        .unwrap_or(String::from("NaN"))
                )
            );
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
                    registries.materials.get(&pixel.material.id.to_string()).unwrap().id
                )
            );

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
            ui.set_max_width(ctx.pixels_per_point() * 120.0);

            let mut elements = registries.materials.values().cloned().collect::<Vec<Material>>();

            elements.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));

            let mut empty = true;

            egui::ScrollArea
                ::vertical()
                .auto_shrink(true)
                .max_height(ctx.pixels_per_point() * 200.0)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
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
                                            egui::Stroke::new(
                                                ctx.pixels_per_point(),
                                                egui::Color32::GOLD
                                            )
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
                });

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
