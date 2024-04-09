use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_rapier2d::plugin::PhysicsSet;
use noise::SuperSimplex;

use crate::{
    generation::{chunk::{generate_chunk, ChunkGenerationEvent}, tiles::{debug_tiles, parse_tiles, TileGenerator}},
    registries::Registries,
    setup_egui, setup_egui_fonts,
    state::AppState,
};

use self::{
    chunk_manager::{
        chunks_update, manager_setup, update_loaded_chunks,
        ChunkManager,
    },
    dirty_rect::{dirty_rects_gizmos, DirtyRects},
    object::{fill_objects, unfill_objects},
    particle::{particle_setup, particles_update, update_partcile_meshes},
};

pub mod chunk;
pub mod chunk_groups;
pub mod chunk_manager;
pub mod dirty_rect;
pub mod materials;
pub mod mesh;
pub mod object;
pub mod particle;
pub mod pixel;

#[derive(Resource)]
pub struct Noise(SuperSimplex);

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkManager>()
            .init_resource::<Registries>()
            .insert_resource(TileGenerator {
                scale: 3,
                ..Default::default()
            })
            .add_event::<ChunkGenerationEvent>()
            .add_systems(
                OnTransition {
                    from: AppState::LoadingScreen,
                    to: AppState::InGame,
                },
                (
                    setup_egui,
                    setup_egui_fonts,
                    manager_setup,
                    particle_setup,
                    parse_tiles,
                    update_loaded_chunks,
                    generate_chunk
                ).chain(),
            )
            .add_systems(
                PreUpdate,
                (update_loaded_chunks, generate_chunk)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (
                    particles_update
                        .run_if(|chunk_manager: Res<ChunkManager>| chunk_manager.clock() % 4 == 0),
                    chunks_update.run_if(on_timer(Duration::from_millis(10))),
                )
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                PostUpdate,
                (
                    update_partcile_meshes,
                    dirty_rects_gizmos,
                    render_dirty_rect_updates,
                    debug_tiles
                )
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                (
                    unfill_objects.before(PhysicsSet::SyncBackend),
                    fill_objects
                        .after(PhysicsSet::Writeback)
                        .after(unfill_objects),
                )
                    .run_if(in_state(AppState::InGame)),
            )
            .insert_resource(Msaa::Off)
            .insert_resource(ClearColor(Color::Rgba {
                red: 0.60,
                green: 0.88,
                blue: 1.0,
                alpha: 1.0,
            }))
            .init_resource::<DirtyRects>()
            .init_resource::<Noise>();
    }
}

pub fn render_dirty_rect_updates(
    mut dirty_rects_resource: ResMut<DirtyRects>,
    chunk_manager: Res<ChunkManager>,
    mut images: ResMut<Assets<Image>>,
) {
    dirty_rects_resource
        .render
        .iter_mut()
        .for_each(|(position, rect)| {
            if let Some(chunk) = chunk_manager.get_chunk(position) {
                let image = images.get_mut(chunk.texture.clone()).unwrap();
                chunk.update_rect(image, *rect);
            }
        });

    dirty_rects_resource.render.clear();
}
