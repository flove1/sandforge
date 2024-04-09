use bevy::input::mouse::MouseWheel;

use bevy::prelude::*;
use bevy::tasks::Task;
use bevy_math::{vec2, vec3};

use crate::animation::{AnimationIndices, AnimationTimer};
use crate::assets::PlayerSpriteAssets;
use crate::constants::{CHUNK_SIZE, PLAYER_LAYER};
use crate::simulation::chunk_manager::manager_setup;
use crate::state::AppState;

use super::actor::{update_actors, Actor};

#[derive(Default, Component)]
pub struct Player {
    state: PlayerState,
}

#[derive(Default)]
pub enum PlayerState {
    #[default]
    Idle,
    Walking,
    Jumping(f64),
}

#[derive(Component, Default)]
pub struct Tool;

#[derive(Component)]
pub struct ToolFront;

pub fn player_setup(
    mut commands: Commands,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    player_sprites: Res<PlayerSpriteAssets>,
) {
    let player_actor = Actor {
        position: vec2(0., 0.),
        velocity: vec2(0., 0.),
        hitbox: Rect::from_corners(Vec2::ZERO, Vec2::new(6., 15.)),
        on_ground: false,
    };

    let texture_atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        Vec2::new(64.0, 64.0),
        8,
        1,
        None,
        None,
    ));
    let animation_indices = AnimationIndices { first: 0, last: 7 };

    commands.spawn((
        player_actor,
        Player::default(),
        SpriteSheetBundle {
            texture: player_sprites.idle.clone(),
            atlas: TextureAtlas {
                layout: texture_atlas_layout,
                index: animation_indices.first,
            },
            transform: Transform {
                translation: vec3(0.0, 0.0, PLAYER_LAYER),
                scale: (Vec3::splat(1.0 / CHUNK_SIZE as f32)),
                ..Default::default()
            },
            sprite: Sprite { anchor: bevy::sprite::Anchor::Center, ..Default::default() },
            ..Default::default()
        },
        animation_indices,
        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
    ));
}

pub const RUN_SPEED: f32 = 2.;
pub const JUMP_MAG: f32 = 2.;
pub const PRESSED_JUMP_MAG: f32 = 0.2;
pub const TIME_JUMP_PRESSED: f64 = 0.25;

/// Updates player
pub fn update_player(
    input: (Res<Inputs>, EventReader<MouseWheel>),
    mut player: Query<(&mut Actor, &mut Player, &mut AnimationIndices)>,
    time: Res<Time>,
) {
    let (mut actor, mut player, mut _anim_idxs) = player.single_mut();
    let (inputs, mut _scroll_evr) = input;

    // Movement
    let x = inputs.right - inputs.left;
    actor.velocity.x = x * RUN_SPEED;

    let on_ground = actor.on_ground;

    if on_ground {
        if x.abs() > 0. {
            player.state = PlayerState::Walking
        } else {
            player.state = PlayerState::Idle
        }
    }

    if inputs.jump_just_pressed && on_ground {
        actor.velocity.y = JUMP_MAG;
        player.state = PlayerState::Jumping(time.elapsed_seconds_wrapped_f64());
    }

    //Jump higher when holding space
    if let PlayerState::Jumping(jump_start) = player.state {
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

    //Zoom
    // for ev in scroll_evr.read() {
    //     if ev.unit == MouseScrollUnit::Line {
    //         zoom.0 *= 0.9_f32.powi(ev.y as i32);
    //         zoom.0 = zoom.0.clamp(ZOOM_LOWER_BOUND, ZOOM_UPPER_BOUND);
    //     }
    // }

    //Change shooting atoms
}

pub fn update_player_sprite(mut query: Query<(&mut Transform, &Actor), With<Player>>) {
    let (mut transform, actor) = query.single_mut();
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

#[derive(Resource, Default)]
pub struct SavingTask(pub Option<Task<()>>);

pub fn get_input(keys: Res<ButtonInput<KeyCode>>, mut inputs: ResMut<Inputs>) {
    //Jump
    if keys.just_pressed(KeyCode::Space) {
        inputs.jump_just_pressed = true;
        inputs.jump_pressed = true;
    } else if keys.pressed(KeyCode::Space) {
        inputs.jump_pressed = true;
    }

    //Movement
    if keys.pressed(KeyCode::KeyA) {
        inputs.left = 1.;
    }
    if keys.pressed(KeyCode::KeyD) {
        inputs.right = 1.;
    }
}

pub fn clear_input(mut inputs: ResMut<Inputs>) {
    *inputs = Inputs::default();
}

#[derive(Resource, Default)]
pub struct Inputs {
    left: f32,
    right: f32,

    jump_pressed: bool,
    jump_just_pressed: bool,
}

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                update_player.before(update_actors),
                update_player_sprite.after(update_actors),
                clear_input.after(update_player),
            )
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(PreUpdate, get_input.run_if(in_state(AppState::InGame)))
        .init_resource::<SavingTask>()
        .init_resource::<Inputs>()
        .add_systems(OnExit(AppState::LoadingScreen), player_setup.after(manager_setup));
    }
}
