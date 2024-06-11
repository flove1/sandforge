use bevy::{ prelude::*, tasks::{ block_on, futures_lite::future, AsyncComputeTaskPool, Task } };

use itertools::Itertools;

use crate::{
    constants::CHUNK_SIZE,
    simulation::{
        chunk_groups::build_chunk_group,
        chunk_manager::ChunkManager,
        materials::PhysicsType,
    },
};

use super::{ actor::Actor, enemy::Enemy, player::Player };

#[derive(Component)]
pub struct Path {
    pub nodes: Vec<IVec2>,
    pub created_at: f64,
}

#[derive(Component)]
pub struct PathGenerationTask(Task<Option<Path>>);

const DIRECTIONS: [(i32, i32); 8] = [
    (0, -1),
    (0, 1),
    (-1, 0),
    (1, 0),
    (-1, -1),
    (-1, 1),
    (1, -1),
    (1, 1),
];

pub fn pathfind_start(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut actors: Query<
        (Entity, &Actor, &Transform, Option<&Path>),
        (With<Enemy>, Without<PathGenerationTask>)
    >,
    player_q: Query<&Transform, (With<Player>, Without<Enemy>)>,
    time: Res<Time>
) {
    let thread_pool = AsyncComputeTaskPool::get();
    let player_position = (player_q.single().translation.xy() * (CHUNK_SIZE as f32))
        .round()
        .as_ivec2();

    for (entity, actor, transform, path) in actors.iter_mut() {
        let position = (transform.translation.xy() * (CHUNK_SIZE as f32)).round().as_ivec2();

        let chunk_position = ((player_position + position) / 2).div_euclid(
            IVec2::splat(CHUNK_SIZE)
        );

        if (player_position - position).abs().max_element() > CHUNK_SIZE * 2 {
            commands.entity(entity).remove::<Path>().remove::<PathGenerationTask>();
            continue;
        }

        if path.map_or(true, |path| time.elapsed_seconds_f64() - path.created_at > 0.1) {
            let size = actor.size;
            let created_at = time.elapsed_seconds_f64();
            let Some(chunk_group) = build_chunk_group(&mut chunk_manager, chunk_position) else {
                continue;
            };

            let task = PathGenerationTask(
                thread_pool.spawn(async move {
                    let node_size = 4;

                    let start = (position - chunk_position * CHUNK_SIZE) / node_size;
                    let end = (player_position - chunk_position * CHUNK_SIZE) / node_size;

                    let path = pathfinding::prelude::astar(
                        &(start.x, start.y),
                        |&(x, y)| {
                            let directions = DIRECTIONS.iter()
                                .map(|(dx, dy)| (x + dx, y + dy))
                                .collect_vec();

                            directions
                                .into_iter()
                                .filter(|(x, y)| {
                                    let node_position = IVec2::new(*x, *y);
                                    let world_position = node_position * node_size + node_size / 2;

                                    let size = (size / 2.0).round().as_ivec2();

                                    (-size.x..=size.x)
                                        .cartesian_product(-size.y..=size.y)
                                        .all(|(x, y)|
                                            chunk_group
                                                .get(world_position + IVec2::new(x, y))
                                                .map_or(false, |pixel|
                                                    matches!(
                                                        pixel.physics_type,
                                                        PhysicsType::Air |
                                                            PhysicsType::Gas(..) |
                                                            PhysicsType::Liquid(..)
                                                    )
                                                )
                                        )
                                })
                                .map(|node| (node, 1))
                        },
                        |&(x, y)| (end.x - x).abs() + (end.y - y).abs(),
                        |&(x, y)| (IVec2::new(x, y) - end).abs().cmple(IVec2::ONE).all()
                    );


                    path.map(|(path, _)| {
                        Path {
                            nodes: path
                            .into_iter()
                            .map(|(x, y)| {
                                IVec2::new(x, y) * node_size +
                                    node_size / 2 +
                                    chunk_position * CHUNK_SIZE
                            })
                            .collect(),
                            created_at
                        }
                    })
                })
            );

            commands.entity(entity).insert(task);
        }
    }
}

pub fn pathfind_apply(
    mut commands: Commands,
    mut actors: Query<(Entity, &mut PathGenerationTask), With<Enemy>>
) {
    for (entity, mut task) in actors.iter_mut() {
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
}

pub fn gizmos_path(mut actors: Query<Option<&Path>, With<Enemy>>, mut gizmos: Gizmos) {
    for path in actors.iter_mut().flatten() {
        for (p1, p2) in path.nodes[0..path.nodes.len() - 1]
            .iter()
            .zip(path.nodes[1..path.nodes.len()].iter()) {
            gizmos.line_2d(
                p1.as_vec2() / (CHUNK_SIZE as f32) + 0.5 / (CHUNK_SIZE as f32),
                p2.as_vec2() / (CHUNK_SIZE as f32) + 0.5 / (CHUNK_SIZE as f32),
                Color::PINK
            );
        }
    }
}
