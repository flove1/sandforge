use std::{
    cmp::{ Ordering, Reverse },
    collections::{ BTreeMap, BTreeSet, BinaryHeap },
    process::Command,
};

use bevy::{
    prelude::*,
    tasks::{ block_on, futures_lite::future, AsyncComputeTaskPool, Task },
    transform::commands,
    utils::{ HashMap, HashSet },
};
use bevy_math::{ ivec2, vec2 };
use itertools::Itertools;

use crate::{
    constants::{ CHUNK_SIZE, WORLD_HEIGHT, WORLD_WIDTH },
    simulation::{
        chunk_groups::{ build_chunk_group, ChunkGroup},
        chunk_manager::ChunkManager,
        materials::PhysicsType,
        mesh::douglas_peucker,
        pixel::Pixel,
    },
};

use super::{ actor::Actor, enemy::Enemy, player::Player};

#[derive(Component)]
pub struct Path{
    pub nodes: Vec<IVec2>,
    pub created_at: f64
}

#[derive(Component)]
pub struct PathGenerationTask(Task<Option<Path>>);

#[inline]
fn is_empty(position: IVec2, size: IVec2, chunk_group: &ChunkGroup<Pixel>) -> bool {
    let size = (size.as_vec2() / 2.0).round().as_ivec2() - 1
    ;
    // let minimum = position - IVec2::splat(8);
    // let maximum = position + IVec2::splat(8);

    // chunk_group
    //     .get(position)
    //     .map_or(false, |pixel|
    //         matches!(
    //             pixel.material.physics_type,
    //             PhysicsType::Air | PhysicsType::Gas | PhysicsType::Liquid(..)
    //         )
    //     )

    (-size.x..=size.x)
        .cartesian_product(-size.y..=size.y)
        .all(|(x, y)|
            chunk_group
                .get(position + ivec2(x, y))
                .map_or(false, |pixel|
                    matches!(
                        pixel.material.physics_type,
                        PhysicsType::Air | PhysicsType::Gas | PhysicsType::Liquid(..)
                    )
                )
        )
}

const DIRECTIONS: [IVec2; 8] = [
    ivec2(-1, 0),
    ivec2(1, 0),
    ivec2(0, -1),
    ivec2(0, 1),
    ivec2(-1, -1),
    ivec2(-1, 1),
    ivec2(1, -1),
    ivec2(1, 1),
];

#[derive(Copy, Clone, Eq, PartialEq)]
struct Node {
    cost: i32,
    position: IVec2,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[inline]
pub fn heuristic(a: IVec2, b: IVec2) -> i32 {
    (b.x - a.x).abs() + (b.y - a.y).abs()
}

fn astar_search(
    start: IVec2,
    goal: IVec2,
    size: IVec2,
    chunk_group: &ChunkGroup<Pixel>,
    chunk_position: IVec2
) -> Option<Vec<IVec2>> {
    let mut open_set = BinaryHeap::new();
    open_set.push(Node { cost: 0, position: start });

    let mut came_from = HashMap::new();
    let mut cost_so_far = HashMap::new();

    came_from.insert(start, None);
    cost_so_far.insert(start, 0);

    while let Some(Node { position, .. }) = open_set.pop() {
        if position == goal {
            let mut path = vec![goal.as_vec2()];
            let mut current = position;

            while current != start {
                path.push(current.as_vec2());
                current = came_from[&current].unwrap();
            }

            // path.push(start.as_vec2());

            return Some(
                douglas_peucker(&path, 4.0)
                    .iter()
                    .rev()
                    .map(|position| {
                        position.round().as_ivec2() +
                            chunk_position * CHUNK_SIZE +
                            ivec2(fastrand::i32(-2..=2), fastrand::i32(-2..=2))
                            // - size / 2
                    })
                    .collect_vec()
            );
        }

        for &offset in &DIRECTIONS {
            let next = position + offset;
            if is_empty(next, size, chunk_group) {
                let new_cost = cost_so_far[&position] + 1;
                if !cost_so_far.contains_key(&next) || new_cost < cost_so_far[&next] {
                    cost_so_far.insert(next, new_cost);
                    let priority = new_cost + heuristic(goal, next);
                    open_set.push(Node { cost: priority, position: next });
                    came_from.insert(next, Some(position));
                }
            }
        }
    }

    None
}

pub fn pathfind(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut actors: Query<(Entity, &Actor, &Transform, Option<&mut PathGenerationTask>, Option<&Path>), With<Enemy>>,
    player_q: Query<&Transform, (With<Player>, Without<Enemy>)>,
    time: Res<Time>
) {
    let thread_pool = AsyncComputeTaskPool::get();
    let player_position = (player_q.single().translation.xy() * (CHUNK_SIZE as f32))
        .round()
        .as_ivec2();

    for (entity, actor, transform, mut task, path) in actors.iter_mut() {
        let position = (transform.translation.xy() * (CHUNK_SIZE as f32))
            .round()
            .as_ivec2();
        
        let chunk_position = ((player_position + position) / 2).div_euclid(
            IVec2::splat(CHUNK_SIZE)
        );

        if (player_position - position).abs().max_element() > CHUNK_SIZE * 2 {
            commands.entity(entity).remove::<Path>().remove::<PathGenerationTask>();
            continue;
        }

        if let Some(task) = task.as_mut() {
            if let Some(result) = block_on(future::poll_once(&mut task.0)) {
                if let Some(path) = result {
                    commands.entity(entity).insert(path);
                    commands.entity(entity).remove::<PathGenerationTask>();
                } else {
                    commands.entity(entity).remove::<Path>();
                    commands.entity(entity).remove::<PathGenerationTask>();
                }
            }
        }

        
        if path.map_or(true, |path| time.elapsed_seconds_f64() - path.created_at > 0.1) {
            let size = actor.hitbox.size().as_ivec2();
            let created_at = time.elapsed_seconds_f64();
            let Some(chunk_group) = build_chunk_group(&mut chunk_manager, chunk_position) else {
                continue;
            };

            commands
                .entity(entity)
                .insert(
                    PathGenerationTask(
                        thread_pool.spawn(async move {
                            astar_search(
                                position - chunk_position * CHUNK_SIZE,
                                player_position - chunk_position * CHUNK_SIZE,
                                size,
                                &chunk_group,
                                chunk_position
                            ).map(|nodes| Path { nodes, created_at })
                        })
                    )
                );
        }

    }
}

pub fn gizmos_path(mut actors: Query<Option<&Path>, With<Enemy>>, mut gizmos: Gizmos) {
    for path in actors.iter_mut().flatten() {
        for (p1, p2) in path.nodes[0..path.nodes.len() - 1].iter().zip(path.nodes[1..path.nodes.len()].iter()) {
            gizmos.line_2d(
                p1.as_vec2() / (CHUNK_SIZE as f32) + 0.5 / (CHUNK_SIZE as f32),
                p2.as_vec2() / (CHUNK_SIZE as f32) + 0.5 / (CHUNK_SIZE as f32),
                Color::PINK
            );
        }
    }
}
