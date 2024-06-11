// #![windows_subsystem = "windows"]

// use mimalloc::MiMalloc;

// #[global_allocator]
// static GLOBAL: MiMalloc = MiMalloc;

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
mod raycast;
mod postprocessing;
mod cursor;
mod settings;
mod interpolator;

use std::time::Duration;

use actors::ActorsPlugin;
use animation::AnimationPlugin;
use assets::{
    process_assets, AudioAssetCollection, FontAssetCollection, FontAssetLoader, FontBytes, LayoutAssetCollection, SpriteAssetCollection
};
use bevy::{
    audio::{AudioPlugin, SpatialScale},
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    render::{ settings::{ PowerPreference, WgpuSettings }, RenderPlugin },
    window::{ Cursor, PresentMode, PrimaryWindow, WindowMode, WindowResolution },
    winit::{ UpdateMode, WinitSettings },
};
use bevy_asset_loader::loading_state::{
    config::ConfigureLoadingState,
    LoadingState,
    LoadingStateAppExt,
};
use bevy_egui::EguiPlugin;

use bevy_rapier2d::plugin::{ NoUserData, RapierConfiguration, RapierPhysicsPlugin };
use bevy_tween::{ interpolation::EaseFunction, span_tween::SpanTweenerBundle, tween::ComponentTween };
use camera::CameraPlugin;
use constants::CHUNK_SIZE;
use cursor::{ move_cursor, setup_cursor };
use gui::GuiPlugin;

use helpers::{ tick_despawn_timer, DespawnTimer };
use interpolator::{ InterpolateVolume, InterpolatorPlugin };
use painter::PainterPlugin;

use postprocessing::PostProcessPlugin;
use registries::Registries;
use seldom_state::StateMachinePlugin;
use settings::{ process_config, SettingsPlugin };
use simulation::SimulationPlugin;
use state::{ state_auto_transition, GameState };

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(AudioPlugin {
                    default_spatial_scale: SpatialScale::new_2d(1.0),
                    ..Default::default()
                })
                .set(ImagePlugin::default_nearest())
                .set(RenderPlugin {
                    render_creation: bevy::render::settings::RenderCreation::Automatic(
                        WgpuSettings {
                            power_preference: PowerPreference::LowPower,
                            ..Default::default()
                        }
                    ),
                    synchronous_pipeline_compilation: false,
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        mode: WindowMode::Windowed,
                        resolution: WindowResolution::default().with_scale_factor_override(1.0),
                        present_mode: PresentMode::AutoVsync,
                        cursor: Cursor {
                            visible: false,
                            ..Default::default()
                        },
                        title: "Sandforge".into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                }),
            RapierPhysicsPlugin::<NoUserData>
                ::pixels_per_meter((CHUNK_SIZE as f32) / 4.0)
                .with_default_system_setup(false)
                .in_fixed_schedule(),
            EguiPlugin,
            StateMachinePlugin,
            FrameTimeDiagnosticsPlugin,
        ))
        // .add_plugins(WorldInspectorPlugin::new())
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins((
            InterpolatorPlugin,
            SimulationPlugin,
            ActorsPlugin,
            AnimationPlugin,
            CameraPlugin,
            PainterPlugin,
            GuiPlugin,
            PostProcessPlugin,
            SettingsPlugin,
        ))
        .insert_resource(RapierConfiguration::new(0.1))
        .insert_resource(ClearColor(Color::BLACK))
        .init_state::<GameState>()
        .init_asset::<FontBytes>()
        .init_asset_loader::<FontAssetLoader>()
        .add_loading_state(
            LoadingState::new(GameState::LoadingAssets)
                .load_collection::<FontAssetCollection>()
                .load_collection::<LayoutAssetCollection>()
                .load_collection::<SpriteAssetCollection>()
                .load_collection::<AudioAssetCollection>()
                .continue_to_state(GameState::Menu)
        )
        .add_systems(OnExit(GameState::LoadingAssets), (
            process_assets,
            setup_cursor,
            process_config,
            move |mut commands: Commands| {
                commands.init_resource::<Registries>();
            },
        ))
        .add_systems(Update, (state_auto_transition, tick_despawn_timer, move_cursor))
        .run();
}

pub fn has_window(query: Query<&Window, With<PrimaryWindow>>) -> bool {
    !query.is_empty()
}

pub fn fade_out_audio<T: Component>(
    mut commands: Commands,
    mut audio_sink_q: Query<(Entity, &mut AudioSink), With<T>>
) {
    for (entity, sink) in audio_sink_q.iter_mut() {
        commands
            .entity(entity)
            .insert(DespawnTimer(Timer::from_seconds(1.0, TimerMode::Once)))
            .insert(EaseFunction::Linear)
            .insert(SpanTweenerBundle::new(Duration::from_secs(1)).tween_here())
            .insert(
                ComponentTween::new(InterpolateVolume {
                    start: sink.volume(),
                    end: 0.0,
                })
            );
    }
}

fn despawn_component<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in &to_despawn {
        commands.entity(entity).despawn_recursive();
    }
}

fn remove_respurce<T: Resource>(mut commands: Commands) {
    commands.remove_resource::<T>();
}
