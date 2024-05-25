use bevy::prelude::*;
use bevy_math::{ IVec2, Vec2 };
use bevy_rapier2d::geometry::{Collider, CollisionGroups, Group};

use super::{chunk::Chunk, chunk_manager::ChunkManager};

pub const TERRAIN_MASK: u32 = 1 << 0;
pub const PLAYER_MASK: u32 = 1 << 1;
pub const ACTOR_MASK: u32 = 1 << 3;
pub const ENEMY_MASK: u32 = 1 << 4;
pub const HITBOX_MASK: u32 = 1 << 5;
pub const OBJECT_MASK: u32 = 1 << 6;

pub fn douglas_peucker(vertices: &[Vec2], epsilon: f32) -> Vec<Vec2> {
    let mut d_squared_max = 0.0;
    let mut farthest_point_index = 0;
    let end = vertices.len() - 1;
    if end < 3 {
        return vertices.to_vec();
    }
    let line = (vertices[0], vertices[end - 1]);
    for (i, _) in vertices
        .iter()
        .enumerate()
        .take(end - 1)
        .skip(1) {
        let d_squared = perpendicular_squared_distance(vertices[i], line);
        if d_squared > d_squared_max {
            farthest_point_index = i;
            d_squared_max = d_squared;
        }
    }

    if d_squared_max > epsilon {
        let rec_results1 = douglas_peucker(&vertices[0..farthest_point_index], epsilon);
        let rec_results2 = douglas_peucker(&vertices[farthest_point_index..end + 1], epsilon);

        [rec_results1, rec_results2[1..rec_results2.len()].to_vec()].concat()
    } else {
        vec![vertices[0], vertices[end]]
    }
}

fn perpendicular_squared_distance(point: Vec2, line: (Vec2, Vec2)) -> f32 {
    let x_diff = line.1.x - line.0.x;
    let y_diff = line.1.y - line.0.y;
    let numerator = (
        y_diff * point.x -
        x_diff * point.y +
        line.1.x * line.0.y -
        line.1.y * line.0.x
    ).abs();
    let numerator_squared = numerator * numerator;
    let denominator_squared = y_diff * y_diff + x_diff * x_diff;
    numerator_squared / denominator_squared
}

#[derive(Event, Deref, DerefMut)]
pub struct ChunkColliderEveny(pub IVec2);

pub fn process_chunk_collider_events(
    mut commands: Commands,
    chunk_manager: Res<ChunkManager>,
    mut chunk_ev: EventReader<ChunkColliderEveny>,
    mut chunk_set: ParamSet<
        (Query<&Children, With<Chunk>>, Query<Entity, (With<Parent>, With<Collider>)>)
    >
) {
    for ev in chunk_ev.read() {
        let chunk_position = ev.0;
        if let Some((entity, chunk)) = chunk_manager.chunks.get(&chunk_position) {
            let mut chunk_children = vec![];

            for child_entity in chunk_set.p0().get(*entity).unwrap() {
                chunk_children.push(*child_entity);
            }

            for child in chunk_children {
                if let Ok(child_entity) = chunk_set.p1().get(child) {
                    commands.entity(child_entity).despawn();
                }
            }

            if let Ok(colliders) = chunk.build_colliders() {
                commands.entity(*entity).with_children(|parent| {
                    for collider in colliders {
                        parent.spawn((
                            collider,
                            TransformBundle {
                                local: Transform::IDENTITY,
                                ..Default::default()
                            },
                            CollisionGroups::new(
                                Group::from_bits_truncate(TERRAIN_MASK),
                                Group::from_bits_truncate(OBJECT_MASK)
                            ),
                        ));
                    }
                });
            }
        }
    }
}
