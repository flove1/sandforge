use actor::{ toggle_actors, ActorDebugRender };
use bevy::prelude::*;
use leafwing_input_manager::plugin::InputManagerPlugin;
use pathfinding::pathfind_apply;
use player::Player;

use crate::{
    assets::AudioAssetCollection,
    despawn_component,
    simulation::object::unfill_objects,
    state::GameState,
};

use self::{
    actor::{ render_actor_gizmos, update_actor_translation, update_actors, Actor, MovementType },
    effects::{ damage_flash, death },
    enemy::{ enemy_update, update_enemy_rotation, Enemy },
    health::{ process_damage_events, tick_iframes, DamageEvent, Health },
    pathfinding::{ gizmos_path, pathfind_start },
    player::{
        player_attack,
        player_collect_sand,
        player_dash,
        player_hook,
        player_jump,
        player_jump_extend,
        player_prune_empty_materials,
        player_reset_position,
        player_run,
        player_setup,
        player_shoot,
        player_switch_material,
        player_synchronize_attack_rotation,
        store_camera_position,
        update_player_rotation,
        update_rope_position,
        PlayerActions,
        PlayerTrackingParticles,
    },
};

pub mod actor;
pub mod enemy;
pub mod player;
pub mod pathfinding;
pub mod effects;
pub mod health;
pub mod animation;

pub struct ActorsPlugin;
impl Plugin for ActorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerTrackingParticles>()
            .add_event::<DamageEvent>()
            .add_plugins(InputManagerPlugin::<PlayerActions>::default())
            .add_systems(OnEnter(GameState::LevelInitialization), despawn_component::<Enemy>)
            .add_systems(OnEnter(GameState::LevelInitialization), player_reset_position)
            .add_systems(OnExit(GameState::GameOver), despawn_component::<Enemy>)
            .add_systems(OnEnter(GameState::GameOver), (
                despawn_component::<Player>,
                move |mut commands: Commands, audio_assets: Res<AudioAssetCollection>| {
                    commands.spawn((
                        AudioBundle {
                            source: audio_assets.death.clone(),
                            settings: PlaybackSettings::DESPAWN,
                        },
                    ));
                },
            ))
            .add_systems(OnEnter(GameState::Setup), player_setup)
            .add_systems(
                Update,
                (
                    toggle_actors,
                    player_jump,
                    (player_attack, player_synchronize_attack_rotation).chain(),
                    player_dash,
                    player_hook,
                    player_shoot,
                    player_collect_sand,
                    (player_prune_empty_materials, player_switch_material).chain(),
                ).run_if(in_state(GameState::Game))
            )
            .add_systems(PreUpdate, store_camera_position.run_if(in_state(GameState::Game)))
            .add_systems(
                PreUpdate,
                (pathfind_start, pathfind_apply).chain().run_if(in_state(GameState::Game))
            )
            .add_systems(
                FixedUpdate,
                (player_jump_extend, player_run, update_actors, enemy_update)
                    .chain()
                    .run_if(in_state(GameState::Game))
                    .before(unfill_objects)
            )
            .add_systems(
                FixedPostUpdate,
                (
                    update_player_rotation,
                    update_enemy_rotation,
                    update_actor_translation,
                    // update_health_bar_translation,
                ).run_if(in_state(GameState::Game))
            )
            .add_systems(
                PostUpdate,
                (
                    update_rope_position,
                    process_damage_events,
                    damage_flash,
                    death,
                    // update_health_bars,
                    tick_iframes,
                )
                    .chain()
                    .run_if(in_state(GameState::Game))
            )
            .register_type::<Actor>()
            .register_type::<MovementType>()
            .register_type::<Health>();

        app.init_resource::<ActorDebugRender>().add_systems(
            PostUpdate,
            (gizmos_path, render_actor_gizmos).run_if(
                in_state(GameState::Game).and_then(resource_equals(ActorDebugRender(true)))
            )
        );
    }
}
