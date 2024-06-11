use bevy::{
    audio::Volume,
    prelude::*,
    window::{ PresentMode, PrimaryWindow, WindowMode },
};
use bevy_persistent::{ Persistent, StorageFormat };
use serde::{ Deserialize, Serialize };

#[derive(Debug, Resource, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub resolution: [u32; 2],

    #[serde(default)]
    pub mode: WindowMode,

    #[serde(default)]
    pub vsync: PresentMode,

    #[serde(default)]
    pub volume: i32,

    #[serde(default)]
    pub spatial: bool,
}

fn default_volume() -> i32 {
    50
}

#[derive(Debug, Resource, Serialize, Deserialize, Clone)]
pub struct Scoreboard {
    pub scores: Vec<(i32, i32)>,
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let config_dir = dirs::config_dir().unwrap().join("sandforge");

        app.insert_resource(
            Persistent::<Config>
                ::builder()
                .name("Main config")
                .format(StorageFormat::Toml)
                .path(config_dir.join("config.toml"))
                .default(Config {
                    vsync: PresentMode::AutoVsync,
                    mode: WindowMode::Windowed,
                    resolution: [1280, 720],
                    volume: default_volume(),
                    spatial: false,
                })
                .build()
                .expect("failed to initialize config")
        )
            .insert_resource(
                Persistent::<Scoreboard>
                    ::builder()
                    .name("Scoreboard")
                    .format(StorageFormat::Toml)
                    .path(config_dir.join("scoreboard.toml"))
                    .default(Scoreboard {
                        scores: vec![],
                    })
                    .build()
                    .expect("failed to initialize scores")
            );
    }
}

pub fn process_config(
    mut audio_sink_q: Query<&mut AudioSink>,
    mut global_volume: ResMut<GlobalVolume>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    config: Res<Persistent<Config>>,
) {
    let mut window = window_q.single_mut();

    window.resolution.set(config.resolution[0] as f32, config.resolution[1] as f32);
    window.resolution.set_scale_factor_override(Some(config.resolution[0] as f32 / 1280.0));
    window.mode = config.mode.clone();
    window.present_mode = config.vsync.clone();

    let volume = ((config.volume as f32) / 100.0).clamp(0.0, 100.0);
    global_volume.volume = Volume::new(volume);

    for audio_sink in audio_sink_q.iter_mut() {
        audio_sink.set_volume(volume);
    }
}