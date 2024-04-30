use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_rapier2d::plugin::PhysicsSet;
use noise::{Fbm, MultiFractal, Perlin, RidgedMulti};

use crate::{
    generation::tiles::{process_tile_requests, TileGenerator, TileRequestEvent}, gui::setup_egui_fonts, registries::Registries, state::AppState
};

use self::{
    chunk_manager::{chunks_update, manager_setup, update_loaded_chunks, ChunkManager},
    dirty_rect::{dirty_rects_gizmos, DirtyRects},
    object::{fill_objects, unfill_objects},
    particle::{particle_setup, particles_update, update_partcile_meshes},
};

pub mod chunk;
pub mod chunk_groups;
pub mod chunk_manager;
pub mod dirty_rect;
pub mod material_placer;
pub mod materials;
pub mod mesh;
pub mod object;
pub mod particle;
pub mod pixel;

#[derive(Resource)]
pub struct RidgedNoise1Res(pub Arc<RidgedMulti<Perlin>>);

#[derive(Resource)]
pub struct RidgedNoise2Res(pub Arc<RidgedMulti<Perlin>>);

impl FromWorld for RidgedNoise1Res {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(Arc::new(
            RidgedMulti::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .subsec_millis(),
            )
            .set_octaves(1)
            .set_frequency(2.0),
        ))
    }
}

impl FromWorld for RidgedNoise2Res {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(Arc::new(
            RidgedMulti::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .subsec_millis(),
            )
            .set_octaves(1)
            .set_frequency(2.0),
        ))
    }
}

#[derive(Resource)]
pub struct FbmNoiseRes(pub Arc<Fbm<Perlin>>);

impl FromWorld for FbmNoiseRes {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(Arc::new(
            Fbm::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .subsec_millis(),
            )
            .set_octaves(6)
            .set_frequency(3.0), // .set_persistence(4.0),
        ))
    }
}

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkManager>()
            .init_resource::<Registries>()
            .init_resource::<TileGenerator>()
            .add_event::<TileRequestEvent>()
            .add_systems(
                OnTransition {
                    from: AppState::LoadingScreen,
                    to: AppState::InGame,
                },
                (
                    setup_egui_fonts,
                    manager_setup,
                    particle_setup,
                    update_loaded_chunks,
                    process_tile_requests,
                )
                    .chain(),
            )
            .add_systems(
                PreUpdate,
                (update_loaded_chunks, process_tile_requests)
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
            .init_resource::<RidgedNoise1Res>()
            .init_resource::<RidgedNoise2Res>()
            .init_resource::<FbmNoiseRes>();
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
            if let Some(chunk) = chunk_manager.get_chunk_data(position) {
                let image = images.get_mut(chunk.texture.clone()).unwrap();
                chunk.update_rect(image, *rect);
            }
        });

    dirty_rects_resource.render.clear();
}
