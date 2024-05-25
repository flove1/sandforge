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
use animation::{ Animation, AnimationPlugin, AnimationState };
use assets::{ process_assets, ChunkLayoutAssets, FontBytes, FontAssetLoader, FontAssets, SpriteSheets };
use benimator::FrameRate;
use bevy::{
    audio::AudioPlugin, diagnostic::FrameTimeDiagnosticsPlugin, prelude::*, render::{ settings::WgpuSettings, RenderPlugin }, window::PrimaryWindow, winit::{ UpdateMode, WinitSettings }
};
use bevy_asset_loader::loading_state::{
    config::ConfigureLoadingState,
    LoadingState,
    LoadingStateAppExt,
};
use bevy_egui::EguiPlugin;

use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::
    plugin::{ NoUserData, RapierConfiguration, RapierPhysicsPlugin }
;
use camera::CameraPlugin;
use constants::CHUNK_SIZE;
use generation::chunk::GenerationTask;
use gui::GuiPlugin;

use painter::PainterPlugin;
use postprocessing::PostProcessPlugin;
use seldom_state::StateMachinePlugin;
use simulation::SimulationPlugin;
use state::AppState;

fn main() {
    App::new()
        .add_plugins((
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
                }),
            RapierPhysicsPlugin::<NoUserData>
                ::pixels_per_meter((CHUNK_SIZE as f32) / 4.0).with_default_system_setup(false)
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
            SimulationPlugin,
            ActorsPlugin,
            AnimationPlugin,
            CameraPlugin,
            PainterPlugin,
            GuiPlugin,
            PostProcessPlugin
        ))
        .insert_resource(RapierConfiguration::new(0.1))
        .init_state::<AppState>()
        .init_asset::<FontBytes>()
        .init_asset_loader::<FontAssetLoader>()
        .add_loading_state(
            LoadingState::new(AppState::LoadingAssets)
                .load_collection::<FontAssets>()
                .load_collection::<ChunkLayoutAssets>()
                .load_collection::<SpriteSheets>()
                .continue_to_state(AppState::WorldInitilialization)
        )
        .add_systems(OnExit(AppState::LoadingAssets), process_assets)
        .add_systems(OnEnter(AppState::WorldInitilialization), (splash_setup, add_splash_content).chain())
        .add_systems(
            Update,
            countdown
                .run_if(in_state(AppState::WorldInitilialization))
        )
        .add_systems(OnExit(AppState::WorldInitilialization), despawn_screen::<SplashScreen>)
        .run();
}

#[derive(Component)]
struct SplashScreen;

#[derive(Resource, Deref, DerefMut)]
struct SplashTimer(Timer);

fn splash_setup(mut commands: Commands) {
    commands.spawn((
        SplashScreen,
        NodeBundle {
            style: Style {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            background_color: BackgroundColor(Color::BLACK),
            ..default()
        },
    ));
}

fn add_splash_content(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    splash_q: Query<Entity, With<SplashScreen>>
) {
    let icon = asset_server.load("loading.png");
    let texture_atlas_layout = texture_atlas_layouts.add(
        TextureAtlasLayout::from_grid(Vec2::new(48.0, 48.0), 6, 1, None, None)
    );

    commands.entity(splash_q.single()).with_children(|parent| {
        parent.spawn((
            AnimationState::default(),
            Animation(
                benimator::Animation::from_indices(0..=5, FrameRate::from_fps(12.0)).repeat()
            ),
            AtlasImageBundle {
                style: Style {
                    width: Val::Px(200.0),
                    ..default()
                },
                image: UiImage::new(icon),
                texture_atlas: TextureAtlas {
                    layout: texture_atlas_layout,
                    index: 0,
                },
                ..default()
            },
        ));
    });

    commands.insert_resource(SplashTimer(Timer::from_seconds(1.0, TimerMode::Once)));
}

fn countdown(mut timer: ResMut<SplashTimer>, time: Res<Time>) {
    timer.tick(time.delta());
}

pub fn has_window(query: Query<&Window, With<PrimaryWindow>>) -> bool {
    !query.is_empty()
}

fn despawn_screen<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in &to_despawn {
        commands.entity(entity).despawn_recursive();
    }
}
