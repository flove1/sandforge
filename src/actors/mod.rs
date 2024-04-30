use std::time::Duration;

use bevy::{ prelude::*, time::common_conditions::on_timer };

use crate::{
    constants::CHUNK_SIZE,
    gui::egui_has_primary_context,
    simulation::{chunk_manager::manager_setup, object::unfill_objects},
    state::AppState,
};

use self::{
    actor::{ render_actor_gizmos, update_actors, Actor },
    enemy::{ spawn_enemy, update_enemy },
    pathfinding::{ gizmos_path, pathfind },
    player::{
        clear_input,
        get_input,
        player_setup,
        raycast_from_player,
        update_actors_transforms,
        update_player,
        update_player_sprite,
        Inputs,
        SavingTask,
    },
};

pub mod actor;
pub mod enemy;
pub mod player;
pub mod pathfinding;

pub struct ActorsPlugin;
impl Plugin for ActorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, get_input.run_if(in_state(AppState::InGame)))
            .add_systems(OnExit(AppState::LoadingScreen), (
                player_setup.after(manager_setup),
                spawn_enemy,
            ))
            .add_systems(
                FixedUpdate,
                (
                    update_player,
                    update_actors,
                    update_actors_transforms,
                    update_player_sprite,
                    update_enemy,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)).before(unfill_objects)
            )
            .add_systems(
                PostUpdate,
                (
                    clear_input,
                    raycast_from_player.run_if(egui_has_primary_context),
                    render_actor_gizmos,
                    pathfind,
                    gizmos_path,
                )
                    .run_if(in_state(AppState::InGame))
                    .run_if(in_state(AppState::InGame))
            )
            .init_resource::<SavingTask>()
            .init_resource::<Inputs>();
    }
}
