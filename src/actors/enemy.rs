use crate::{
    animation::{AnimationIndices, AnimationTimer}, assets::SpriteSheets, constants::{CHUNK_SIZE, PLAYER_LAYER}, raycast::raycast, simulation::chunk_manager::ChunkManager
};
use bevy::{ecs::query, prelude::*, window::PrimaryWindow};
use bevy_math::{vec2, vec3};

use super::{
    actor::{Actor, MovementType}, pathfinding::Path, player::Player
};

#[derive(Component)]
pub struct Enemy;

pub fn spawn_enemy(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    sprites: Res<SpriteSheets>,
) {
    let texture_atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        Vec2::splat(17.0),
        6,
        1,
        None,
        None,
    ));
    let animation_indices = AnimationIndices { first: 0, last: 5 };
    let animation_timer = AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating));

    commands.spawn((
        Enemy,
        Actor {
            position: vec2(-3., -9.),
            velocity: vec2(0., 0.),
            hitbox: Rect::from_corners(Vec2::ZERO, Vec2::new(6., 6.)),
            on_ground: false,
            movement_type: MovementType::Floating,
        },
        SpriteSheetBundle {
            texture: sprites.bat.clone(),
            atlas: TextureAtlas {
                layout: texture_atlas_layout.clone(),
                index: animation_indices.first,
            },
            transform: Transform {
                translation: vec3(1.0, 0.0, PLAYER_LAYER + 1.0),
                scale: (Vec3::splat(1.0 / CHUNK_SIZE as f32)),
                ..Default::default()
            },
            sprite: Sprite {
                anchor: bevy::sprite::Anchor::Center,
                ..Default::default()
            },
            ..Default::default()
        },
        animation_indices.clone(),
        animation_timer.clone()
    ));

    commands.spawn((
        Enemy,
        Actor {
            position: vec2(0., -2.),
            velocity: vec2(0., 0.),
            hitbox: Rect::from_corners(Vec2::ZERO, Vec2::new(6., 6.)),
            on_ground: false,
            movement_type: MovementType::Floating,
        },
        SpriteSheetBundle {
            texture: sprites.bat.clone(),
            atlas: TextureAtlas {
                layout: texture_atlas_layout.clone(),
                index: animation_indices.first,
            },
            transform: Transform {
                translation: vec3(1.0, 0.0, PLAYER_LAYER + 1.0),
                scale: (Vec3::splat(1.0 / CHUNK_SIZE as f32)),
                ..Default::default()
            },
            sprite: Sprite {
                anchor: bevy::sprite::Anchor::Center,
                ..Default::default()
            },
            ..Default::default()
        },
        animation_indices.clone(),
        animation_timer.clone()
    ));

    commands.spawn((
        Enemy,
        Actor {
            position: vec2(3., -9.),
            velocity: vec2(0., 0.),
            hitbox: Rect::from_corners(Vec2::ZERO, Vec2::new(6., 6.)),
            on_ground: false,
            movement_type: MovementType::Floating,
        },
        SpriteSheetBundle {
            texture: sprites.bat.clone(),
            atlas: TextureAtlas {
                layout: texture_atlas_layout.clone(),
                index: animation_indices.first,
            },transform: Transform {
                translation: vec3(1.0, 0.0, PLAYER_LAYER + 1.0),
                scale: (Vec3::splat(1.0 / CHUNK_SIZE as f32)),
                ..Default::default()
            },
            
            sprite: Sprite {
                anchor: bevy::sprite::Anchor::Center,
                ..Default::default()
            },
            ..Default::default()
        },
        animation_indices.clone(),
        animation_timer.clone(),
    ));
}

pub fn update_enemy(
    q1: Query<&Transform, With<Player>>,
    mut q2: Query<(&mut Actor, &Transform, Option<&mut Path>), With<Enemy>>,
    chunk_manager: Res<ChunkManager>,
    mut gizmos: Gizmos,
) {
    let player_position = (q1.single().translation.xy() * CHUNK_SIZE as f32)
        .round()
        .as_ivec2();

    for (mut actor, transform, path) in q2.iter_mut() {
        let enemy_position = (transform.translation.xy() * CHUNK_SIZE as f32)
            .round()
            .as_ivec2();

        // if raycast(player_position, enemy_position, &chunk_manager).is_none() {
        //     gizmos.line_2d(
        //         enemy_position.as_vec2() / CHUNK_SIZE as f32,
        //         player_position.as_vec2() / CHUNK_SIZE as f32,
        //         Color::RED,
        //     );

        //     actor.velocity += (player_position - enemy_position).as_vec2().normalize_or_zero() / 2.0;
        // }
        // else 
        if let Some(mut path) = path {
            let mut closest_position = path.0[0];
            while path.0.len() > 1 && (enemy_position - path.0[0]).length_squared() * 4 > (enemy_position - path.0[1]).length_squared() {
                closest_position = path.0[1];
                path.0.remove(0);
            }

            actor.velocity += ((closest_position - enemy_position).as_vec2()).normalize_or_zero() / 8.0 + (fastrand::f32() - 0.5) / 8.0;

        }
        else {
            // actor.velocity += vec2((fastrand::f32() - 0.5) / 4.0, (fastrand::f32() - 0.5) / 8.0);
        }
    }
}
