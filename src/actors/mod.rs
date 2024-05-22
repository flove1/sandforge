use std::time::Duration;

use bevy::prelude::*;
use bevy_rapier2d::plugin::PhysicsSet;
use leafwing_input_manager::plugin::InputManagerPlugin;

use crate::{ simulation::object::unfill_objects, state::AppState };

use self::{
    actor::{ render_actor_gizmos, update_actor_translation, update_actors, Actor, MovementType },
    effects::{ damage_flash, death },
    enemy::{ enemy_despawn, enemy_update, update_enemy_rotation },
    health::{
        create_health_bars,
        process_damage_events,
        update_health_bar_translation,
        update_health_bars,
        DamageEvent,
        Health,
        HealthBar,
    },
    pathfinding::{ gizmos_path, pathfind },
    player::{
        player_dash, player_hook, player_jump, player_jump_extend, player_kick, player_run, player_setup, player_shoot, update_player_rotation, update_rope_position, PlayerActions
    },
};

pub mod actor;
pub mod enemy;
pub mod player;
pub mod pathfinding;
pub mod effects;
pub mod health;

pub struct ActorsPlugin;
impl Plugin for ActorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DamageEvent>()
            .add_plugins(InputManagerPlugin::<PlayerActions>::default())
            .add_systems(OnEnter(AppState::WorldInitilialization), enemy_despawn)
            .add_systems(OnExit(AppState::WorldInitilialization), player_setup)
            .add_systems(
                Update,
                (create_health_bars, player_jump, player_kick, player_dash, player_hook, player_shoot).run_if(
                    in_state(AppState::Game)
                )
            )
            .add_systems(PreUpdate, pathfind.run_if(in_state(AppState::Game)))
            .add_systems(
                FixedUpdate,
                (player_jump_extend, player_run, update_actors, enemy_update)
                    .chain()
                    .run_if(in_state(AppState::Game))
                    .before(unfill_objects)
            )
            .add_systems(
                FixedPostUpdate,
                (
                    update_player_rotation,
                    update_enemy_rotation,
                    update_actor_translation,
                    update_health_bar_translation,
                ).run_if(in_state(AppState::Game))
            )
            .add_systems(
                PostUpdate,
                (
                    update_rope_position,
                    process_damage_events,
                    damage_flash,
                    death,
                    update_health_bars,
                )
                    .chain()
                    .run_if(in_state(AppState::Game))
            )
            .register_type::<Actor>()
            .register_type::<MovementType>()
            .register_type::<Health>()
            .register_type::<HealthBar>();

        // #[cfg(feature = "debug-render")]
        // app.add_systems(
        //     PostUpdate,
        //     (
        //         gizmos_path,
        //         render_actor_gizmos,
        //         raycast_from_player.run_if(egui_has_primary_context),
        //     ).run_if(in_state(AppState::Game))
        // );
    }
}
