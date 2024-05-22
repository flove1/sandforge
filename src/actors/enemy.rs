use crate::{
    animation::{ Animation, AnimationState },
    assets::SpriteSheets,
    constants::{ CHUNK_SIZE, PLAYER_LAYER },
    raycast::raycast,
    simulation::chunk_manager::ChunkManager,
};
use benimator::FrameRate;
use bevy::{ ecs::query, prelude::*, window::PrimaryWindow };
use bevy_math::{ vec2, vec3 };
use bevy_rapier2d::{ dynamics::{GravityScale, ReadMassProperties, RigidBody, Velocity}, geometry::{ Collider, Sensor }, };

use super::{ actor::{ Actor, ActorBundle, MovementType }, health::{DamageEvent, Health, HealthBarOverlay}, pathfinding::Path, player::Player };

#[derive(Component)]
pub struct Enemy;

pub fn enemy_despawn(
    mut commands: Commands,
    enemy_q: Query<Entity, With<Enemy>>,
) {
    for entity in enemy_q.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

// since rapier automatically manages transforms it is required to manually store it
#[derive(Component, Deref, DerefMut)]
pub struct Flipped(pub bool);

pub fn enemy_update(
    player_q: Query<&Transform, With<Player>>,
    mut enemy_q: Query<(&mut Velocity, &Transform, Option<&mut Path>), With<Enemy>>,
    chunk_manager: Res<ChunkManager>,
    mut gizmos: Gizmos
) {
    let player_position = (player_q.single().translation.xy() * (CHUNK_SIZE as f32)).round().as_ivec2();

    for (mut velocity, transform, path) in enemy_q.iter_mut() {
        let enemy_position = (transform.translation.xy() * (CHUNK_SIZE as f32)).round().as_ivec2();

        if let Some(mut path) = path {
            let mut closest_position = path.nodes[0];
            while path.nodes.len() > 1 && (enemy_position - path.nodes[0]).length_squared() < 8 {
                closest_position = path.nodes[1];
                path.nodes.remove(0);
            }

            velocity.linvel +=
                (closest_position - enemy_position).as_vec2().normalize_or_zero() / 16.0 +
                (fastrand::f32() - 0.5) / 8.0;
        } else {
            velocity.linvel += vec2((fastrand::f32() - 0.5) / 4.0, (fastrand::f32() - 0.5) / 8.0);
        }
    }
}

pub fn update_enemy_rotation(
    mut enemy_q: Query<(&Actor, &mut Transform, &Velocity, &mut Flipped), With<Enemy>>,
) {
    for (actor, mut transform, velocity, mut flipped) in enemy_q.iter_mut() {
        if velocity.linvel.x < -0.01 {
            flipped.0 = true;
        } else if velocity.linvel.x > 0.01 {
            flipped.0 = false;
        }

        if flipped.0 {
            transform.rotation = Quat::from_rotation_y(-(180f32).to_radians());
        }
    }
}