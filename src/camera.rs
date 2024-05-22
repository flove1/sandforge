use bevy::{
    prelude::*,
    render::{
        camera:: RenderTarget ,
        extract_resource::{ExtractResource,  ExtractResourcePlugin} ,
        render_resource::{
            Extent3d,
            TextureDescriptor,
            TextureDimension,
            TextureFormat,
            TextureUsages,
        },
        texture::
            BevyDefault
        ,
        view::RenderLayers,
    },
    window::{ PrimaryWindow, WindowResized },
};
use bevy_math::vec2;
use log::{debug, info};

use crate::{
    actors::player::Player,
    constants::CHUNK_SIZE,
    postprocessing::{
        light_apply::LightApply,
        light_calculate::LightMask,
        light_propagate::LightPropagationSettings,
    },
    state::AppState,
};

#[derive(Component)]
pub struct TrackingCamera {
    pub position: Vec2,
    pub target: Vec2,
    pub tracking_size: Vec2,
    pub clamp_size: Vec2,
    pub dead_zone: Vec2,
    pub speed: f64,
    pub recenter_timeout: f32,
    pub last_track: f32,
}

impl Default for TrackingCamera {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            target: Vec2::ZERO,
            tracking_size: vec2(1.0 / 16.0, 1.0 / 9.0),
            clamp_size: vec2(48.0, 28.0),
            dead_zone: Vec2::splat(0.1),
            speed: 0.98,
            recenter_timeout: 3.0,
            last_track: 0.0,
        }
    }
}

impl TrackingCamera {
    pub fn update(&mut self, player_pos: Vec2, dt: f64) {
        self.track_player(player_pos);

        let new_last_track = self.last_track + (dt as f32);

        if self.last_track < self.recenter_timeout && new_last_track > self.recenter_timeout {
            // target the player
            self.target = player_pos;
        }

        self.last_track = new_last_track;

        let lerp = 1.0 - ((1.0 - self.speed).powf(dt) as f32);
        self.position = self.position.lerp(self.target, lerp);
    }

    pub fn clamp_rect(half_size: Vec2, point: Vec2) -> Option<Vec2> {
        let mut ox = None;
        let mut oy = None;

        if point.x > half_size.x {
            ox = Some(point.x - half_size.x);
        } else if point.x < -half_size.x {
            ox = Some(point.x + half_size.x);
        }

        if point.y > half_size.y {
            oy = Some(point.y - half_size.y);
        } else if point.y < -half_size.y {
            oy = Some(point.y + half_size.y);
        }

        if let (None, None) = (ox, oy) {
            None
        } else {
            Some(vec2(ox.unwrap_or(0.0), oy.unwrap_or(0.0)))
        }
    }

    pub fn track_player(&mut self, player_pos: Vec2) {
        let rel_pos = player_pos - self.position;

        if let Some(offset) = Self::clamp_rect(self.tracking_size, rel_pos) {
            if !self.dead_zone.cmpgt(offset.abs()).all() {
                self.target = self.position + offset;
                self.last_track = 0.0;
            }
        }

        if let Some(offset) = Self::clamp_rect(self.clamp_size, rel_pos) {
            self.position += offset;
        }
    }
}

pub fn update_camera(
    player_q: Query<&Transform, With<Player>>,
    mut camera_q: Query<(&mut Transform, &mut TrackingCamera), (With<Camera>, Without<Player>)>,
    time: Res<Time>
) {
    let transform = player_q.single();
    let (mut camera_transform, mut camera_tracking) = camera_q.single_mut();
    let dt = time.delta_seconds_f64();
    camera_tracking.update(transform.translation.xy(), dt);
    camera_transform.translation = camera_tracking.position.extend(4.0);
}

pub const BACKGROUND_LAYER: u8 = 1;
pub const TERRAIN_LAYER: u8 = 2;
pub const ACTOR_LAYER: u8 = 3;
pub const LIGHTING_LAYER: u8 = 4;
pub const PROCESSED_TERRAIN_LAYER: u8 = 5;

#[derive(Default, Resource, ExtractResource, Clone)]
pub struct LightingTexture {
    pub scale: f32,
    pub texture: Handle<Image>,
}

fn on_resize_system(
    mut resize_reader: EventReader<WindowResized>,
    lighting: Res<LightingTexture>,
    mut images: ResMut<Assets<Image>>
) {
    for e in resize_reader.read() {
        let size = Extent3d {
            width: (e.width.ceil() * lighting.scale) as u32,
            height: (e.height.ceil() * lighting.scale) as u32,
            ..Default::default()
        };

        let image = images.get_mut(lighting.texture.clone()).unwrap();

        image.texture_descriptor = TextureDescriptor {
            size,
            ..image.texture_descriptor.clone()
        };
        
        info!("window was resized: {}:{}", e.width, e.height);

        image.resize(size);
    }
}

#[derive(Component)]
pub struct LightingCamera;

// TODO: optimize by moving processing of terrain into separate pipeline and downscalling
fn setup_camera(
    mut commands: Commands,
    mut time: ResMut<Time<Fixed>>,
    lighting: Res<LightingTexture>
) {
    time.set_timestep_hz(58.0);

    commands
        .spawn((
            Camera2dBundle {
                projection: OrthographicProjection {
                    scale: 0.5 / (CHUNK_SIZE as f32),
                    ..Default::default()
                },
                ..Default::default()
            },
            InheritedVisibility::VISIBLE,
            Visibility::Visible,
            TrackingCamera::default(),
            LightApply,
            RenderLayers::from_layers(&[BACKGROUND_LAYER, TERRAIN_LAYER, ACTOR_LAYER]),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Lighting"),
                Camera2dBundle {
                    camera: Camera {
                        order: -1,
                        clear_color: ClearColorConfig::Custom(Color::rgba_from_array([0.0; 4])),
                        target: RenderTarget::Image(lighting.texture.clone()),
                        ..Default::default()
                    },
                    projection: OrthographicProjection {
                        scale: (0.5 / (CHUNK_SIZE as f32) / lighting.scale) * 1.25,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                LightMask,
                LightPropagationSettings { offset: 2.0, passes: 8 },
                RenderLayers::from_layers(&[BACKGROUND_LAYER, TERRAIN_LAYER]),
            ));

            parent.spawn((
                Name::new("Other"),
                Camera2dBundle {
                    camera: Camera {
                        order: 1,
                        clear_color: ClearColorConfig::None,
                        ..Default::default()
                    },
                    projection: OrthographicProjection {
                        scale: (0.5 / (CHUNK_SIZE as f32)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                RenderLayers::layer(0),
            ));
        });
}

pub fn setup_lighting(
    mut commands: Commands,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>
) {
    let window = window_q.single();

    let scale = 1.25;
    let size = Extent3d {
        width: (window.width() * scale).round() as u32,
        height: (window.height() * scale).round() as u32,
        ..Default::default()
    };

    let mut lighting = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::bevy_default(),
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING |
            TextureUsages::COPY_DST |
            TextureUsages::COPY_SRC |
            TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };

    lighting.resize(size);

    commands.insert_resource(LightingTexture {
        scale,
        texture: images.add(lighting),
    })
}

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractResourcePlugin::<LightingTexture>::default())

            .add_systems(Startup, (setup_lighting, setup_camera).chain())
            .add_systems(
                Update,
                (update_camera, on_resize_system).run_if(in_state(AppState::Game))
            );
    }
}
