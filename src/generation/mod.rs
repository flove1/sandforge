use bevy::prelude::*;
use bevy_rapier2d::dynamics::Velocity;
use leafwing_input_manager::action_state::ActionState;

use crate::{
    actors::{ actor::Actor, player::{ Player, PlayerActions } }, assets::ChunkLayoutAssets, constants::CHUNK_SIZE, registries::Registries, simulation::{
        chunk_groups::build_chunk_group_with_texture_access,
        chunk_manager::{ update_loaded_chunks, ChunkManager },
        pixel::Pixel,
        reset_world,
    }, state::AppState, SplashTimer
};

use self::{
    chunk::{
        populate_chunk,
        process_chunk_generation_events,
        process_chunk_generation_tasks,
        AwaitingNearbyChunks,
        GenerationTask,
    }, level::Level, noise::{ Noise, Seed }, poisson::{ Poisson, PoissonEnemyPosition }
};

pub mod chunk;
pub mod level;
pub mod noise;
pub mod poisson;

pub struct GenerationPlugin;

#[derive(Component)]
pub struct Exit;

pub fn add_exit(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut images: ResMut<Assets<Image>>,
    registries: Res<Registries>
) {
    let mut chunk_group = build_chunk_group_with_texture_access(
        &mut chunk_manager,
        IVec2::ZERO,
        &mut images
    ).unwrap();

    let radius = 32;

    for x in -radius..=radius {
        for y in -radius..=radius {
            let position = IVec2::new(x, y);

            if position.length_squared() > radius.pow(2) {
                continue;
            }

            chunk_group.set(position, Pixel::default()).expect("ok");
        }
    }

    // for x in -radius / 2..=radius / 2 {
    //     for y in -radius / 2..=radius / 2 {
    //         let position = IVec2::new(x, y);

    //         if position.length_squared() > (radius / 2).pow(2) {
    //             continue;
    //         }

    //         chunk_group.background_set(position, [0; 4]).expect("ok");
    //     }
    // }

    commands.spawn((
        Exit,
        TransformBundle::from_transform(Transform::from_translation(Vec3::new(2.0, 2.0, 0.0))),
    ));

    let element = registries.materials.get("wood").unwrap();
    for x in -16..=16 {
        for y in -8..=0 {
            let position = IVec2::new(x, y);

            chunk_group.set(position, element.into()).expect("ok");
        }
    }
}

pub fn remove_exit(mut commands: Commands, exit_q: Query<Entity, With<Exit>>) {
    if !exit_q.is_empty() {
        commands.entity(exit_q.single()).despawn_recursive();
    }
}

pub fn move_actors_to_exit(
    mut actor_q: Query<(&Transform, &mut Velocity), With<Actor>>,
    exit_q: Query<&Transform, With<Exit>>
) {
    let Ok(exit_transform) = exit_q.get_single() else {
        return;
    };

    for (transform, mut velocity) in actor_q.iter_mut() {
        if transform.translation.xy().distance(exit_transform.translation.xy()) < 2.0 {
            let delta = exit_transform.translation.xy() - transform.translation.xy();
            if delta.length() > 8.0 / (CHUNK_SIZE as f32) {
                velocity.linvel +=
                    ((delta.signum() * delta.length_recip()) / (CHUNK_SIZE as f32)) * 4.0;
            }
        }
    }
}

#[derive(Default, Resource, Deref, DerefMut)]
pub struct LevelCounter(u32);

pub fn is_player_in_exit(
    player_q: Query<(Entity, &Transform, &ActionState<PlayerActions>), With<Player>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut gizmos: Gizmos,
    mut game_state: ResMut<NextState<AppState>>
) {
    let (player_entity, player_transform, action_state) = player_q.single();

    // let text = Text2dBundle {
    //     text: Text::from_section("Press E to exit", TextStyle {
    //         font: (),
    //         font_size: (),
    //         color: (),
    //     }),
    //     transform: todo!(),
    //     ..Default::default()
    // };

    let rect = Rect::new(-1.0, -1.0, 2.0, 2.0);
    if rect.contains(player_transform.translation.xy()) {
        if action_state.just_pressed(&PlayerActions::Interaction) {
            game_state.set(AppState::WorldInitilialization);
        }
    }
}

#[derive(Resource)]
pub struct LevelData(pub Level, pub Handle<Image>);

pub fn increase_level_counter(
    mut commands: Commands,
    mut counter: ResMut<LevelCounter>,
    registries: Res<Registries>,
    layouts: ResMut<ChunkLayoutAssets>,
    seed: Res<Seed>
) {
    counter.0 += 1;

    let level = registries.levels
        .get((counter.0 - 1).rem_euclid(registries.levels.len() as u32) as usize)
        .unwrap();

    let level_texture = layouts.folder.get(&level.texture_path).unwrap();

    let seed = seed.0 + counter.0;

    let noise = Noise::from_seed(seed, level.noise_type);
    let poisson = Poisson::from_seed(seed, level.enemy_frequency);
    let enemies = PoissonEnemyPosition::from_distibution(&poisson);

    commands.insert_resource(AwaitingNearbyChunks::default());
    commands.insert_resource(LevelData(level.clone(), level_texture.clone()));
    commands.insert_resource(noise);
    commands.insert_resource(poisson);
    commands.insert_resource(enemies);
}

fn check_generation_tasks(
    mut game_state: ResMut<NextState<AppState>>,
    timer: Res<SplashTimer>,
    tasks_q: Query<&GenerationTask>
) {
    if timer.finished() && tasks_q.is_empty() {
        game_state.set(AppState::Game);
    }
}

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Seed>()
            .init_resource::<LevelCounter>()
            .add_systems(
                OnEnter(AppState::WorldInitilialization),
                (
                    increase_level_counter,
                    reset_world,
                    remove_exit,
                    update_loaded_chunks,
                    process_chunk_generation_events,
                ).chain()
            )
            .add_systems(
                Update,
                (process_chunk_generation_tasks, populate_chunk, check_generation_tasks).run_if(
                    in_state(AppState::WorldInitilialization)
                )
            )
            .add_systems(OnExit(AppState::WorldInitilialization), add_exit)
            .add_systems(Update, is_player_in_exit.run_if(in_state(AppState::Game)));
    }
}
