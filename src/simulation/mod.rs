use std::time::Duration;

use bevy::{ prelude::*, time::common_conditions::on_timer, transform::TransformSystem };
use bevy_rapier2d::{
    plugin::{ systems::sync_removals, NoUserData, PhysicsSet, RapierPhysicsPlugin },
    render::{ DebugRenderContext, DebugRenderMode, RapierDebugRenderPlugin },
};

use crate::{
    generation::{ GenerationPlugin, LevelData },
    state::GameState,
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
    colliders::{ process_chunk_collider_events, ChunkColliderEvent },
    dirty_rect::{ dirty_rects_gizmos, DirtyRects },
    object::{
        fill_objects,
        object_collision_damage,
        // process_explosive,
        process_projectiles,
        unfill_objects,
        Object,
    },
    particle::{
        particle_modify_velocity,
        particle_set_parent,
        particle_setup,
        particles_update,
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
            .add_plugins(GenerationPlugin)
            .add_event::<ChunkColliderEvent>()
            .add_systems(OnExit(GameState::GameOver), reset_world)
            .add_systems(Startup, (manager_setup, particle_setup))
            .add_systems(PreUpdate, update_loaded_chunks.run_if(in_state(GameState::Game)))
            .add_systems(
                PostUpdate,
                chunk_set_parent.run_if(
                    in_state(GameState::Game).or_else(in_state(GameState::Splash))
                )
            )
            .add_systems(
                Update,
                (
                    (particle_set_parent, particle_modify_velocity, particles_update).chain(),
                    chunks_update.chain().run_if(on_timer(Duration::from_millis(10))),
                )
                    .chain()
                    .run_if(in_state(GameState::Game))
            )
            .add_systems(
                PostUpdate,
                (render_dirty_rect_updates, process_chunk_collider_events).run_if(
                    in_state(GameState::Game)
                )
            )
            .add_systems(
                FixedUpdate,
                (
                    unfill_objects.before(PhysicsSet::SyncBackend),
                    (
                        object_collision_damage,
                        // process_explosive,
                        process_projectiles,
                    ).after(PhysicsSet::Writeback),
                    fill_objects,
                )
                    .chain()
                    .run_if(in_state(GameState::Game))
            )
            .insert_resource(Msaa::Off)
            .init_resource::<DirtyRects>();

        app.configure_sets(
            FixedUpdate,
            (PhysicsSet::SyncBackend, PhysicsSet::StepSimulation, PhysicsSet::Writeback)
                .chain()
                .before(TransformSystem::TransformPropagate)
                .run_if(in_state(GameState::Game))
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

        app.init_resource::<DirtyRectRender>().add_systems(Update, (
            toggle_colliders,
            toggle_dirty_rects,
        ));

        app.add_plugins(RapierDebugRenderPlugin {
            enabled: false,
            mode: DebugRenderMode::COLLIDER_SHAPES,
            ..Default::default()
        }).add_systems(
            PostUpdate,
            dirty_rects_gizmos.run_if(
                in_state(GameState::Game).and_then(resource_equals(DirtyRectRender(true)))
            )
        );
    }
}

#[derive(Resource, Default, PartialEq, PartialOrd)]
pub struct DirtyRectRender(bool);

pub fn toggle_colliders(mut ctx: ResMut<DebugRenderContext>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::F1) {
        ctx.enabled = !ctx.enabled;
    }
}

pub fn toggle_dirty_rects(mut ctx: ResMut<DirtyRectRender>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::F2) {
        ctx.0 = !ctx.0;
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
    level: Res<LevelData>,
    chunk_manager: Res<ChunkManager>
) {
    dirty_rects_resource.render.iter_mut().for_each(|(position, rect)| {
        if let Some(chunk) = chunk_manager.get_chunk_data(position) {
            chunk.update_textures_part(&mut images, level.0.lighting, *rect);
        }
    });

    dirty_rects_resource.render.clear();
}
