use std::{ sync::Arc, time::{ Duration, SystemTime, UNIX_EPOCH } };

use bevy::{ prelude::*, time::common_conditions::on_timer, transform::TransformSystem };
use bevy_rapier2d::plugin::{ systems::sync_removals, NoUserData, PhysicsSet, RapierPhysicsPlugin };
use noise::{ Fbm, MultiFractal, Perlin, RidgedMulti };

#[cfg(feature = "debug-render")]
use bevy_rapier2d::render::{ RapierDebugRenderPlugin, DebugRenderMode };

use crate::{
    constants::CHUNK_SIZE,
    generation::{
        chunk::{
            populate_chunk,
            process_chunk_generation_events,
            process_chunk_generation_tasks,
            GenerationEvent,
        },
        GenerationPlugin,
    },
    registries::Registries,
    state::AppState,
};

use self::{
    chunk_manager::{
        chunk_set_parent,
        chunks_update,
        manager_setup,
        update_loaded_chunks,
        ChunkManager,
        Terrain,
    },
    colliders::{ process_chunk_collider_events, ChunkColliderEveny },
    dirty_rect::{ dirty_rects_gizmos, DirtyRects },
    object::{ fill_objects, object_collision_damage, process_explosion, process_fall_apart_on_collision, unfill_objects, Object },
    particle::{
        particle_modify_velocity,
        particle_set_parent,
        particle_setup,
        particles_update,
        Particle,
        ParticleParent,
    },
};

pub mod chunk;
pub mod chunk_groups;
pub mod chunk_manager;
pub mod dirty_rect;
pub mod materials;
pub mod colliders;
pub mod object;
pub mod particle;
pub mod pixel;

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkManager>()
            .init_resource::<Registries>()
            .add_plugins(GenerationPlugin)
            .add_event::<GenerationEvent>()
            .add_event::<ChunkColliderEveny>()
            .add_systems(Startup, (manager_setup, particle_setup))
            .add_systems(
                PreUpdate,
                (
                    update_loaded_chunks,
                    process_chunk_generation_events,
                    process_chunk_generation_tasks,
                    populate_chunk,
                )
                    .chain()
                    .run_if(in_state(AppState::Game))
            )
            .add_systems(
                Update,
                (
                    (particle_set_parent, particle_modify_velocity, particles_update)
                        .chain()
                        .run_if(|chunk_manager: Res<ChunkManager>| chunk_manager.clock() % 4 == 0),
                    (chunk_set_parent, chunks_update)
                        .chain()
                        .run_if(on_timer(Duration::from_millis(10))),
                )
                    .chain()
                    .run_if(in_state(AppState::Game))
            )
            .add_systems(
                PostUpdate,
                (render_dirty_rect_updates, process_chunk_collider_events).run_if(
                    in_state(AppState::Game)
                )
            )
            .add_systems(
                FixedUpdate,
                (
                    unfill_objects.before(PhysicsSet::SyncBackend),
                    (object_collision_damage, process_explosion, process_fall_apart_on_collision).after(PhysicsSet::Writeback),
                    fill_objects,
                )
                    .chain()
                    .run_if(in_state(AppState::Game))
            )
            .insert_resource(Msaa::Off)
            .insert_resource(
                ClearColor(Color::Rgba {
                    red: 0.6,
                    green: 0.88,
                    blue: 1.0,
                    alpha: 1.0,
                })
            )
            .init_resource::<DirtyRects>();

        app.configure_sets(
            FixedUpdate,
            (PhysicsSet::SyncBackend, PhysicsSet::StepSimulation, PhysicsSet::Writeback)
                .chain()
                .before(TransformSystem::TransformPropagate)
                .run_if(in_state(AppState::Game))
        );

        app.add_systems(PostUpdate, sync_removals);

        app.add_systems(FixedUpdate, (
            RapierPhysicsPlugin::<NoUserData>
                ::get_systems(PhysicsSet::SyncBackend)
                .in_set(PhysicsSet::SyncBackend),
            RapierPhysicsPlugin::<NoUserData>
                ::get_systems(PhysicsSet::StepSimulation)
                .in_set(PhysicsSet::StepSimulation),
            RapierPhysicsPlugin::<NoUserData>
                ::get_systems(PhysicsSet::Writeback)
                .in_set(PhysicsSet::Writeback),
        ));

        // #[cfg(feature = "debug-render")]
        // app.add_plugins(RapierDebugRenderPlugin {
        //     mode: DebugRenderMode::COLLIDER_SHAPES | DebugRenderMode::JOINTS,
        //     ..Default::default()
        // }).add_systems(PostUpdate, dirty_rects_gizmos.run_if(in_state(AppState::Game)));
    }
}

pub fn reset_world(
    mut commands: Commands,
    particles_instances: Query<Entity, With<ParticleParent>>,
    mut chunk_manager: ResMut<ChunkManager>,
    chunks: Query<Entity, With<Terrain>>,
    objects: Query<Entity, With<Object>>
) {
    commands.entity(particles_instances.single()).despawn_descendants();
    commands.entity(chunks.single()).despawn_descendants();

    for entity in objects.iter() {
        commands.entity(entity).despawn_recursive();
    }

    chunk_manager.chunks.clear();
}

pub fn render_dirty_rect_updates(
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut images: ResMut<Assets<Image>>,
    chunk_manager: Res<ChunkManager>
) {
    dirty_rects_resource.render.iter_mut().for_each(|(position, rect)| {
        if let Some(chunk) = chunk_manager.get_chunk_data(position) {
            let image = images.get_mut(chunk.texture.clone()).unwrap();
            chunk.update_texture_part(image, *rect);
            chunk.update_lighting(&mut images, *rect);
        }
    });

    dirty_rects_resource.render.clear();
}
