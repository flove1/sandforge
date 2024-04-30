use bevy::input::mouse::MouseWheel;

use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::tasks::Task;
use bevy::utils::dbg;
use bevy::window::PrimaryWindow;
use bevy_math::{vec2, vec3};
use bevy_rapier2d::na::ComplexField;

use crate::animation::{AnimationIndices, AnimationTimer};
use crate::assets::SpriteSheets;
use crate::constants::{CHUNK_SIZE, PLAYER_LAYER};
use crate::gui::egui_has_primary_context;
use crate::raycast::raycast;
use crate::simulation::chunk_manager::{manager_setup, ChunkManager};
use crate::state::AppState;

use super::actor::{update_actors, Actor, MovementType};
use super::enemy::spawn_enemy;

#[derive(Default, Component)]
pub struct Player {
    state: PlayerState,
    jump_start: Option<f64>
}

#[derive(Default)]
pub enum PlayerState {
    #[default]
    Idle,
    Walking,
    Jumping(f64),
}

pub fn player_setup(
    mut commands: Commands,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    sprites: Res<SpriteSheets>,
) {
    let player_actor = Actor {
        position: vec2(-3., -9.),
        velocity: vec2(0., 0.),
        hitbox: Rect::from_corners(Vec2::ZERO, Vec2::new(13., 15.)),
        on_ground: false,
        movement_type: MovementType::Walking,
    };

    let texture_atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        Vec2::new(48.0, 48.0),
        9,
        17,
        None,
        None,
    ));
    let animation_indices = AnimationIndices { first: 0, last: 5 };

    commands.spawn((
        player_actor,
        Player::default(),
        SpriteSheetBundle {
            texture: sprites.player.clone(),
            atlas: TextureAtlas {
                layout: texture_atlas_layout,
                index: animation_indices.first,
            },
            transform: Transform {
                translation: vec3(0.0, 0.0, PLAYER_LAYER),
                scale: (Vec3::splat(1.0 / CHUNK_SIZE as f32)),
                ..Default::default()
            },
            sprite: Sprite {
                anchor: bevy::sprite::Anchor::Center,
                ..Default::default()
            },
            ..Default::default()
        },
        animation_indices,
        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
    ));
}

pub const RUN_SPEED: f32 = 2.;
pub const JUMP_MAG: f32 = 1.0;
pub const PRESSED_JUMP_MAG: f32 = 0.175;
pub const TIME_JUMP_PRESSED: f64 = 0.25;

/// Updates player
pub fn update_player(
    input: (ResMut<Inputs>, EventReader<MouseWheel>),
    mut player: Query<(&mut Actor, &mut Player, &mut AnimationIndices)>,
    time: Res<Time>,
) {
    let (mut actor, mut player, mut _anim_idxs) = player.single_mut();
    let (mut inputs, _) = input;

    // Movement
    let x = inputs.right - inputs.left;
    actor.velocity.x = f32::clamp(
        actor.velocity.x + x * RUN_SPEED / 2.0,
        -RUN_SPEED,
        RUN_SPEED,
    );

    if actor.on_ground && actor.velocity.y.is_sign_negative()  {
        player.jump_start = None;
    }

    if actor.on_ground {
        if x.abs().is_sign_negative() {
            player.state = PlayerState::Walking
        } else {
            player.state = PlayerState::Idle
        }
    }

    if let Some(buffered_at) = inputs.jump_buffered {
        if time.elapsed_seconds_wrapped_f64() - buffered_at > 0.1 {
            inputs.jump_buffered = None
        }
        else if actor.on_ground && player.jump_start.is_none() {
            inputs.jump_buffered = None;
            actor.velocity.y = JUMP_MAG;
            player.jump_start = Some(time.elapsed_seconds_wrapped_f64());
        }
    }

    //Jump higher when holding space
    if let Some(jump_start) = player.jump_start {
        if inputs.jump_pressed
            && time.elapsed_seconds_wrapped_f64() - jump_start < TIME_JUMP_PRESSED
        {
            actor.velocity.y += PRESSED_JUMP_MAG
        }
    }

    // (anim_idxs.first, anim_idxs.last) = match player.state {
    //     PlayerState::Idle => (0, 1),
    //     PlayerState::Walking => (8, 11),
    //     PlayerState::Jumping { .. } => (16, 23),
    // };
}

pub fn update_player_sprite(mut query: Query<(&mut AnimationIndices , &Actor), With<Player>>) {
    for (mut indices, actor) in query.iter_mut() {
        if actor.velocity.x.abs() < 0.25 {
            *indices = AnimationIndices { first: 0, last: 5 }
        }
        else {
            *indices = AnimationIndices { first: 45, last: 52 }
        }
    }
}

pub fn update_actors_transforms(mut query: Query<(&mut Transform, &Actor)>) {
    for (mut transform, actor) in query.iter_mut() {
        let left_bottom_vec = vec2(actor.position.x, actor.position.y);
    
        let size = actor.hitbox.size().as_ivec2();
        let center_vec = left_bottom_vec + vec2(size.x as f32 / 2.0, size.y as f32 / 2.0);

        if actor.velocity.x < -0.001 {
            transform.rotation = Quat::from_rotation_y(180f32.to_radians());
        } else if actor.velocity.x > 0.001 {
            transform.rotation = Quat::from_rotation_y(0f32.to_radians());
        }
    
        transform.translation = (center_vec / CHUNK_SIZE as f32).extend(PLAYER_LAYER);
    }
}

pub fn raycast_from_player(
    mut query: Query<&mut Transform, With<Player>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera>>,
    chunk_manager: Res<ChunkManager>,
    mut gizmos: Gizmos,
    q_windows: Query<&Window, With<PrimaryWindow>>,
) {
    let transform = query.single_mut();
    let (camera, camera_transform) = camera.single();

    if let Some(cursor_position) = q_windows.single().cursor_position() {
        let player_position = (transform.translation.xy() * CHUNK_SIZE as f32)
            .round()
            .as_ivec2();

        let pixel_position = (camera
            .viewport_to_world(camera_transform, cursor_position)
            .map(|ray| ray.origin.truncate())
            .unwrap()
            * CHUNK_SIZE as f32)
            .round()
            .as_ivec2();

        if let Some((point, _)) = raycast(player_position, pixel_position, &chunk_manager) {
            gizmos.line_2d(
                transform.translation.xy(),
                point.as_vec2() / CHUNK_SIZE as f32,
                Color::RED,
            );
        } else {
            gizmos.line_2d(
                transform.translation.xy(),
                pixel_position.as_vec2() / CHUNK_SIZE as f32,
                Color::BLUE,
            );
        }
    }
}

#[derive(Resource, Default)]
pub struct SavingTask(pub Option<Task<()>>);

pub fn get_input(keys: Res<ButtonInput<KeyCode>>, mut inputs: ResMut<Inputs>, 
    time: Res<Time>) {
    if keys.just_pressed(KeyCode::Space) {
        inputs.jump_buffered = Some(time.elapsed_seconds_wrapped_f64());
        inputs.jump_pressed = true;
    } else if keys.pressed(KeyCode::Space) {
        inputs.jump_pressed = true;
    }

    if keys.pressed(KeyCode::KeyA) {
        inputs.left = 1.;
    }
    if keys.pressed(KeyCode::KeyD) {
        inputs.right = 1.;
    }
}

pub fn clear_input(mut inputs: ResMut<Inputs>) {
    inputs.jump_pressed = false;
    inputs.left = 0.;
    inputs.right = 0.;
}

#[derive(Resource, Default)]
pub struct Inputs {
    left: f32,
    right: f32,

    jump_pressed: bool,
    jump_buffered: Option<f64>
}
