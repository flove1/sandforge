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

use actors::ActorsPlugin;
use assets::{
    BiomeMapAssets,
    FontAsset,
    FontAssetLoader,
    FontAssets,
    SpriteSheets,
    TileAssets,
};
use bevy::{
    audio::AudioPlugin,
    diagnostic::FrameTimeDiagnosticsPlugin,
    prelude::*,
    render::{ settings::WgpuSettings, RenderPlugin },
    window::PrimaryWindow,
    winit::{ UpdateMode, WinitSettings },
};
use bevy_asset_loader::loading_state::{
    config::ConfigureLoadingState,
    LoadingState,
    LoadingStateAppExt,
};
use bevy_egui::{ egui, EguiContexts, EguiPlugin };

use bevy_rapier2d::{plugin::{NoUserData, RapierConfiguration, RapierPhysicsPlugin}, render::RapierDebugRenderPlugin};
use camera::CameraPlugin;
use constants:: CHUNK_SIZE ;
use gui::
    GuiPlugin
;

use painter:: PainterPlugin ;
use simulation::{
    materials::MaterialInstance, object::get_object_by_click, particle:: Particle, SimulationPlugin
};
use state::AppState;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(ImagePlugin::default_nearest())
                .disable::<AudioPlugin>()
                .set(RenderPlugin {
                    render_creation: bevy::render::settings::RenderCreation::Automatic(
                        WgpuSettings {
                            power_preference: bevy::render::settings::PowerPreference::LowPower,
                            ..Default::default()
                        }
                    ),
                    synchronous_pipeline_compilation: false,
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Sandforge".into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                })
        )
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins(
            RapierPhysicsPlugin::<NoUserData>
                ::pixels_per_meter(CHUNK_SIZE as f32 / 4.0)
                .in_fixed_schedule()
        )
        .add_systems(Startup, setup_camera)
        // .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(SimulationPlugin)
        .add_plugins(EguiPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins((ActorsPlugin, animation::AnimationPlugin))
        .add_systems(
            Update,
            get_object_by_click.run_if(has_window).run_if(in_state(AppState::InGame))
        )
        .add_plugins(CameraPlugin)
        // .add_plugins(WorldInspectorPlugin::new())
        .insert_resource(RapierConfiguration {
            gravity: Vec2::new(0.0, -0.98),
            ..Default::default()
        })
        .add_plugins(PainterPlugin)
        .add_plugins(GuiPlugin)
        .init_state::<AppState>()
        .init_asset::<FontAsset>()
        .init_asset_loader::<FontAssetLoader>()
        .add_loading_state(
            LoadingState::new(AppState::LoadingScreen)
                .load_collection::<FontAssets>()
                .load_collection::<BiomeMapAssets>()
                .load_collection::<SpriteSheets>()
                .load_collection::<TileAssets>()
                .continue_to_state(AppState::InGame)
        )
        .register_type::<Particle>()
        .register_type::<MaterialInstance>()
        .run();
}

fn setup_camera(mut commands: Commands, mut time: ResMut<Time<Fixed>>) {
    time.set_timestep_hz(58.0);

    let mut camera = Camera2dBundle::default();
    camera.camera.hdr = true;
    camera.projection.scale = 0.25 / (CHUNK_SIZE as f32);

    commands.spawn(camera);
}

pub fn has_window(query: Query<&Window, With<PrimaryWindow>>) -> bool {
    !query.is_empty()
}