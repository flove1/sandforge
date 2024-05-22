use std::{ sync::Arc, time::{ SystemTime, UNIX_EPOCH } };

use bevy::{ gizmos, prelude::*, utils::HashMap };
use bevy_math::{ ivec2, vec3 };
use bevy_rapier2d::dynamics::Velocity;
use fast_poisson::Poisson2D;
use leafwing_input_manager::action_state::ActionState;
use noise::{ Fbm, MultiFractal, NoiseFn, Perlin, RidgedMulti };
use rayon::spawn;

use crate::{
    actors::{ actor::Actor, player::{ Player, PlayerActions } },
    constants::CHUNK_SIZE,
    registries::{ self, Registries },
    simulation::{
        chunk_groups::{ build_chunk_group, build_chunk_group_with_texture_access },
        chunk_manager::{ update_loaded_chunks, ChunkManager },
        pixel::Pixel,
        reset_world,
    },
    state::AppState,
};

use self::{
    chunk::{ populate_chunk, process_chunk_generation_events, process_chunk_generation_tasks, AwaitingNearbyChunks },
    structure::Structure,
};

pub mod chunk;
pub mod structure;
pub mod populator;

pub struct GenerationPlugin;

#[derive(Resource, Deref, DerefMut)]
pub struct Seed(pub u32);

impl FromWorld for Seed {
    fn from_world(world: &mut World) -> Self {
        Self(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_millis())
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct Noise(pub Arc<Box<dyn (Fn(Vec2) -> f32) + Send + Sync>>);

impl Noise {
    pub fn from_seed(seed: u32) -> Self {
        let ridged_1 = RidgedMulti::<Perlin>::new(seed).set_octaves(1).set_frequency(2.0);
        let ridged_2 = RidgedMulti::<Perlin>
            ::new(seed * 2)
            .set_octaves(1)
            .set_frequency(2.0);

        let fbm = Fbm::<Perlin>::new(seed).set_octaves(6).set_frequency(3.0);

        Self(
            Arc::new(
                Box::new(move |mut point| {
                    point.x += (fbm.get([point.x as f64, point.y as f64]) as f32) / 2.0;

                    let value =
                        ridged_1.get([point.x as f64, point.y as f64]) +
                        ridged_2.get([point.x as f64, point.y as f64]) / 4.0;

                    value as f32
                })
            )
        )
    }
}

#[derive(Resource)]
pub struct StructureQueue(Vec<(Vec2, Image)>);

#[derive(Resource, Deref, DerefMut)]
pub struct Poisson(Poisson2D);

#[derive(Resource, Deref, DerefMut)]
pub struct PoissonEnemyPosition(pub HashMap<IVec2, Vec<Vec2>>);

impl Poisson {
    pub fn from_seed(seed: u32) -> Self {
        Self(
            Poisson2D::new()
                .with_seed(seed as u64)
                .with_dimensions([20.0, 20.0], 1.0)
        )
    }
}

impl PoissonEnemyPosition {
    pub fn from_distibution(poisson: &Poisson) -> Self {
        let mut map = HashMap::new();

        for point in poisson.iter() {
            let point = Vec2::new(point[0] as f32, point[1] as f32) - 10.0;

            map.entry(point.floor().as_ivec2()).or_insert(Vec::new()).push(point);
        }

        Self(map)
    }
}

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
            let position = ivec2(x, y);

            if position.length_squared() > radius.pow(2) {
                continue;
            }

            chunk_group.set(position, Pixel::default()).expect("ok");
        }
    }

    // for x in -radius / 2..=radius / 2 {
    //     for y in -radius / 2..=radius / 2 {
    //         let position = ivec2(x, y);

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
            let position = ivec2(x, y);

            chunk_group.set(position, Pixel::new(element.into())).expect("ok");
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

pub fn increase_level_counter(
    mut commands: Commands,
    mut counter: ResMut<LevelCounter>,
    seed: Res<Seed>
) {
    counter.0 += 1;

    let seed = seed.0 + counter.0;

    let noise = Noise::from_seed(seed);
    let poisson = Poisson::from_seed(seed);
    let enemies = PoissonEnemyPosition::from_distibution(&poisson);
    
    commands.insert_resource(AwaitingNearbyChunks::default());
    commands.insert_resource(noise);
    commands.insert_resource(poisson);
    commands.insert_resource(enemies);
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
                (process_chunk_generation_tasks, populate_chunk).run_if(in_state(AppState::WorldInitilialization))
            )
            .add_systems(OnExit(AppState::WorldInitilialization), add_exit)
            .add_systems(Update, is_player_in_exit.run_if(in_state(AppState::Game)));
    }
}
