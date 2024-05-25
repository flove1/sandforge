use std::f32::consts::PI;
use std::mem;
use std::process::id;
use std::time::Duration;

use benimator::FrameRate;
use bevy::input::mouse::MouseWheel;

use bevy::utils::HashMap;
use bevy::{
    prelude::*,
    render::{ mesh::{ Indices, PrimitiveTopology }, render_asset::RenderAssetUsages },
    sprite::{ MaterialMesh2dBundle, Mesh2dHandle },
    window::PrimaryWindow,
};
use bevy_egui::EguiContexts;
use bevy_math::{ ivec2, vec2, vec3 };
use bevy_rapier2d::geometry::Sensor;
use bevy_rapier2d::{
    dynamics::{ GravityScale, ImpulseJoint, SpringJointBuilder, Velocity },
    geometry::{ Collider, ColliderMassProperties, CollisionGroups, Group },
};
use bevy_rapier2d::{ na::ComplexField, pipeline::QueryFilter, plugin::RapierContext };
use indexmap::IndexMap;
use itertools::Itertools;
use leafwing_input_manager::buttonlike::MouseWheelDirection;
use leafwing_input_manager::{
    action_state::ActionState,
    axislike::VirtualAxis,
    input_map::InputMap,
    Actionlike,
    InputManagerBundle,
};
use seldom_state::{ prelude::{ AnyState, StateMachine }, trigger::IntoTrigger };

use crate::constants::{ PARTICLE_Z, PLAYER_Z };
use crate::registries::{ self, Registries };
use crate::simulation::chunk::ChunkApi;
use crate::simulation::chunk_groups::build_chunk_group;
use crate::simulation::colliders::{ ENEMY_MASK, HITBOX_MASK, PLAYER_MASK };
use crate::simulation::dirty_rect::DirtyRects;
use crate::simulation::materials::{ Material, PhysicsType };
use crate::simulation::object::{ FallApartOnCollision, Object, ObjectBundle };
use crate::simulation::particle::{ Particle, ParticleBundle, ParticleMovement };
use crate::simulation::pixel::Pixel;
use crate::{
    animation::{ Animation, AnimationState, DespawnOnFinish },
    assets::SpriteSheets,
    camera::TrackingCamera,
    constants::{ CHUNK_SIZE },
    simulation::chunk_manager::ChunkManager,
};

use super::actor::{ Actor, ActorBundle, ActorColliderBundle, ActorFlags, MovementType };
use super::enemy::Enemy;
use super::health::DamageEvent;

use bitflags::bitflags;

pub const ATLAS_COLUMNS: usize = 9;
pub const ATLAS_ROWS: usize = 17;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum PlayerActions {
    Run,
    Crouch,
    Jump,
    Kick,
    Dash,
    Hook,
    Shoot,
    Collect,
    Interaction,
    SelectMaterialNext,
    SelectMaterialPrevious,
}

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct IdleAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct RunningAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct JumpIntroAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct JumpAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct FallAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct LandAnimation;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
struct DashAnimation;

bitflags! {
    #[derive(Default, Component, Clone)]
    pub struct PlayerFlags: u32 {
        const RUNNING = 1 << 0;
        const JUMPING = 1 << 1;
        const DASHING = 1 << 2;
        const SHOOT = 1 << 3;
        const HOOKED = 1 << 4;
    }
}

#[derive(Component, Clone)]
pub struct Player;

pub fn player_setup(
    mut commands: Commands,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut player_q: Query<&mut Actor, With<Player>>,
    sprites: Res<SpriteSheets>
) {
    if let Ok(mut actor) = player_q.get_single_mut() {
        actor.position = vec2(-actor.size.x / 2.0, 0.0);
        return;
    }

    let run_trigger = move |player_q: Query<&Velocity, With<Player>>| {
        match player_q.single().linvel.x.abs() >= 0.25 {
            true => Ok(()),
            false => Err(()),
        }
    };

    let jump_start_trigger = move |
        player_q: Query<
            (
                &PlayerFlags,
                Option<&JumpIntroAnimation>,
                Option<&JumpAnimation>,
                Option<&FallAnimation>,
            ),
            With<Player>
        >
    | {
        let (flags, intro_animation, process_animation, exit_animation) = player_q.single();

        match
            flags.contains(PlayerFlags::JUMPING) &&
            intro_animation.is_none() &&
            process_animation.is_none() &&
            exit_animation.is_none()
        {
            true => Ok(()),
            false => Err(()),
        }
    };

    let animation_ended_trigger = move |player_q: Query<&AnimationState, With<Player>>| {
        match player_q.single().is_ended() {
            true => Ok(()),
            false => Err(()),
        }
    };

    commands
        .spawn((
            Name::new("Player"),
            Player,
            PlayerFlags::default(),
            ActorBundle {
                actor: Actor {
                    position: vec2(-5.0, 0.0),
                    size: Vec2::new(10.0, 17.0),
                    movement_type: MovementType::Walking,
                    ..Default::default()
                },
                mass_properties: ColliderMassProperties::Density(0.25),
                collider: Collider::capsule_y(17.0 / 2.0 - 10.0 / 2.0 - 2.5, 10.0 / 2.0 + 4.0),
                sprite: SpriteSheetBundle {
                    texture: sprites.player.clone(),
                    atlas: TextureAtlas {
                        layout: texture_atlas_layouts.add(
                            TextureAtlasLayout::from_grid(
                                Vec2::new(48.0, 48.0),
                                ATLAS_COLUMNS,
                                ATLAS_ROWS,
                                None,
                                None
                            )
                        ),
                        index: 0,
                    },
                    transform: Transform {
                        translation: vec3(0.0, 0.0, PLAYER_Z),
                        scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            GravityScale(3.0),
        ))
        .insert((
            AnimationState::default(),
            Animation(benimator::Animation::from_indices(0..=5, FrameRate::from_fps(8.0)).repeat()),
            IdleAnimation,
            StateMachine::default()
                .trans::<AnyState, _>(
                    move |player_q: Query<(Option<&DashAnimation>, &PlayerFlags), With<Player>>| {
                        let (animation, flags) = player_q.get_single().unwrap();

                        if flags.contains(PlayerFlags::DASHING) && animation.is_none() {
                            Ok(())
                        } else {
                            Err(())
                        }
                    },
                    DashAnimation
                )
                .trans::<IdleAnimation, _>(run_trigger, RunningAnimation)
                .trans::<RunningAnimation, _>(run_trigger.not(), IdleAnimation)
                .trans::<AnyState, _>(jump_start_trigger, JumpIntroAnimation)
                .trans::<DashAnimation, _>(animation_ended_trigger, IdleAnimation)
                .trans::<JumpIntroAnimation, _>(animation_ended_trigger, JumpAnimation)
                .trans::<JumpAnimation, _>(move |player_q: Query<&Actor, With<Player>>| {
                    match player_q.single().flags.contains(ActorFlags::GROUNDED) {
                        true => Ok(()),
                        false => Err(()),
                    }
                }, LandAnimation)
                .trans::<JumpAnimation, _>(move |player_q: Query<&Velocity, With<Player>>| {
                    match player_q.single().linvel.y < 0.0 {
                        true => Ok(()),
                        false => Err(()),
                    }
                }, FallAnimation)
                .trans::<AnyState, _>(
                    move |
                        player_q: Query<
                            (
                                &PlayerFlags,
                                &Actor,
                                &Velocity,
                                Option<&DashAnimation>,
                                Option<&FallAnimation>,
                            ),
                            With<Player>
                        >
                    | {
                        let (flags, actor, velocity, dashing, falling) = player_q.single();

                        match
                            ((dashing.is_none() && falling.is_none() && velocity.linvel.y < -1.5) ||
                                flags.contains(PlayerFlags::HOOKED)) &&
                            !actor.flags.contains(ActorFlags::GROUNDED)
                        {
                            true => Ok(()),
                            false => Err(()),
                        }
                    },
                    FallAnimation
                )
                .trans::<FallAnimation, _>(move |player_q: Query<&Actor, With<Player>>| {
                    let actor = player_q.single();
                    match actor.flags.contains(ActorFlags::GROUNDED) {
                        true => Ok(()),
                        false => Err(()),
                    }
                }, LandAnimation)
                .trans::<LandAnimation, _>(animation_ended_trigger, IdleAnimation)
                .on_enter::<IdleAnimation>(|entity| {
                    entity.insert(
                        Animation(
                            benimator::Animation
                                ::from_indices(0..=5, FrameRate::from_fps(8.0))
                                .repeat()
                        )
                    );
                    entity.insert(AnimationState::default());
                })
                .on_enter::<RunningAnimation>(|entity| {
                    entity.insert(
                        Animation(
                            benimator::Animation
                                ::from_indices(45..=52, FrameRate::from_fps(8.0))
                                .repeat()
                        )
                    );
                    entity.insert(AnimationState::default());
                })
                .on_enter::<JumpIntroAnimation>(|entity| {
                    entity.insert(
                        Animation(
                            benimator::Animation
                                ::from_indices(72..=73, FrameRate::from_fps(8.0))
                                .once()
                        )
                    );
                    entity.insert(AnimationState::default());
                })
                .on_enter::<JumpAnimation>(|entity| {
                    entity.insert(
                        Animation(
                            benimator::Animation
                                ::from_indices(81..=83, FrameRate::from_fps(8.0))
                                .repeat()
                        )
                    );
                    entity.insert(AnimationState::default());
                })
                .on_enter::<FallAnimation>(|entity| {
                    entity.insert(
                        Animation(
                            benimator::Animation
                                ::from_indices(90..=92, FrameRate::from_fps(8.0))
                                .repeat()
                        )
                    );
                    entity.insert(AnimationState::default());
                })
                .on_enter::<LandAnimation>(|entity| {
                    entity.insert(
                        Animation(
                            benimator::Animation
                                ::from_indices(99..=102, FrameRate::from_fps(8.0))
                                .once()
                        )
                    );
                    entity.insert(AnimationState::default());
                })
                .on_enter::<DashAnimation>(|entity| {
                    entity.insert(
                        Animation(
                            benimator::Animation
                                ::from_indices(63..=67, FrameRate::from_fps(8.0))
                                .once()
                        )
                    );
                    entity.insert(AnimationState::default());
                }),
        ))
        .insert((
            InputManagerBundle::with_map(
                InputMap::default()
                    .insert(PlayerActions::Run, VirtualAxis::ad())
                    .insert(PlayerActions::Jump, KeyCode::Space)
                    .insert(PlayerActions::Kick, KeyCode::KeyF)
                    .insert(PlayerActions::Crouch, KeyCode::KeyS)
                    .insert(PlayerActions::Dash, KeyCode::KeyQ)
                    .insert(PlayerActions::Hook, MouseButton::Right)
                    .insert(PlayerActions::Interaction, KeyCode::KeyE)
                    .insert(PlayerActions::Shoot, KeyCode::KeyR)
                    .insert(PlayerActions::Collect, KeyCode::KeyG)
                    .insert(PlayerActions::SelectMaterialNext, MouseWheelDirection::Up)
                    .insert(PlayerActions::SelectMaterialPrevious, MouseWheelDirection::Down)
                    // .insert(PlayerActions::SelectMaterial, MouseWheelDirection::)
                    .build()
            ),
        ))
        .with_children(|parent| {
            parent.spawn((
                Sensor,
                ColliderMassProperties::Mass(0.0),
                ActorColliderBundle {
                    collider: Collider::capsule_y(17.0 / 2.0 - 10.0 / 2.0 - 2.5, 10.0 / 2.0 + 4.0),
                    collision_groups: CollisionGroups::new(
                        Group::from_bits_retain(PLAYER_MASK | HITBOX_MASK),
                        Group::from_bits_retain(ENEMY_MASK)
                    ),
                    ..Default::default()
                },
            ));

            parent.spawn((
                SpriteSheetBundle {
                    texture: sprites.heal.clone(),
                    atlas: TextureAtlas {
                        layout: texture_atlas_layouts.add(
                            TextureAtlasLayout::from_grid(Vec2::new(48.0, 48.0), 22, 1, None, None)
                        ),
                        index: 0,
                    },
                    ..Default::default()
                },

                AnimationState::default(),
                Animation(
                    benimator::Animation::from_indices(0..=21, FrameRate::from_fps(8.0)).repeat()
                ),
            ));
        });
}

pub const RUN_SPEED: f32 = 1.5;
pub const JUMP_MAG: f32 = 1.0;
pub const PRESSED_JUMP_MAG: f32 = 0.025;
pub const JUMP_EXTENSION_MS: u64 = 500;
pub const JUMP_BUFFER_MS: u64 = 100;

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct JumpBuffer(Timer);

pub fn player_run(
    mut player: Query<
        (&mut Actor, &mut Velocity, &mut PlayerFlags, &ActionState<PlayerActions>),
        With<Player>
    >
) {
    let (mut actor, mut velocity, flags, action_state) = player.single_mut();

    let delta_velocity = (action_state.value(&PlayerActions::Run) * RUN_SPEED) / 8.0;

    if flags.contains(PlayerFlags::HOOKED) {
        velocity.linvel.x += delta_velocity / 2.0;
        actor.flags.insert(ActorFlags::INFLUENCED);
    } else if velocity.linvel.x.abs() > RUN_SPEED {
        if velocity.linvel.x.signum() != delta_velocity.signum() {
            velocity.linvel.x += delta_velocity;
        }
    } else {
        velocity.linvel.x = f32::clamp(velocity.linvel.x + delta_velocity, -RUN_SPEED, RUN_SPEED);
    }
}

pub fn player_jump(
    mut commands: Commands,
    mut player: Query<
        (
            Entity,
            &mut Actor,
            &mut Velocity,
            &mut PlayerFlags,
            &ActionState<PlayerActions>,
            Option<&mut JumpBuffer>,
        ),
        With<Player>
    >,
    time: Res<Time>
) {
    let (entity, mut actor, mut velocity, mut flags, action_state, mut jump_buffer) =
        player.single_mut();

    let can_jump =
        actor.flags.contains(ActorFlags::GROUNDED) && velocity.linvel.y.is_sign_negative();

    if
        flags.contains(PlayerFlags::JUMPING) &&
        actor.flags.contains(ActorFlags::GROUNDED) &&
        velocity.linvel.y.is_sign_negative()
    {
        flags.remove(PlayerFlags::JUMPING);
    }

    if let Some(buffer) = jump_buffer.as_mut() {
        buffer.tick(time.delta());

        if buffer.finished() {
            commands.entity(entity).remove::<JumpBuffer>();
        }
    }

    if can_jump {
        if action_state.just_pressed(&PlayerActions::Jump) || jump_buffer.is_some() {
            velocity.linvel.y = JUMP_MAG;
            flags.insert(PlayerFlags::JUMPING);
            commands.entity(entity).remove::<JumpBuffer>();
        }
    } else if action_state.just_pressed(&PlayerActions::Jump) {
        commands
            .entity(entity)
            .insert(JumpBuffer(Timer::new(Duration::from_millis(JUMP_BUFFER_MS), TimerMode::Once)));
    }
}

pub fn player_jump_extend(
    mut player: Query<(&mut Velocity, &ActionState<PlayerActions>, &mut PlayerFlags)>
) {
    let (mut velocity, action_state, mut flags) = player.single_mut();

    if flags.contains(PlayerFlags::JUMPING) {
        if action_state.pressed(&PlayerActions::Jump) {
            if
                action_state.current_duration(&PlayerActions::Jump) <
                Duration::from_millis(JUMP_EXTENSION_MS)
            {
                velocity.linvel.y += PRESSED_JUMP_MAG;
            }
        } else if action_state.released(&PlayerActions::Jump) {
            flags.remove(PlayerFlags::JUMPING);
        }
    }
}

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct KickBuffer(Timer);

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct KickCooldown(Timer);

pub fn player_kick(
    mut commands: Commands,
    mut player_q: Query<
        (
            Entity,
            &Transform,
            &Velocity,
            &ActionState<PlayerActions>,
            Option<&mut KickCooldown>,
            Option<&mut KickBuffer>,
        ),
        (With<Player>, Without<Enemy>)
    >,
    mut enemy_q: Query<&Transform, With<Enemy>>,
    mut damage_ev: EventWriter<DamageEvent>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    time: Res<Time>,
    rapier_context: Res<RapierContext>,
    sprites: Res<SpriteSheets>
) {
    let (entity, transform, velocity, action_state, mut kick_cooldown, mut kick_buffer) =
        player_q.single_mut();

    if let Some(kick) = kick_buffer.as_mut() {
        kick.tick(time.delta());

        if kick.0.finished() {
            commands.entity(entity).remove::<KickBuffer>();
        }
    }

    if
        kick_cooldown.as_mut().map_or(true, |kick_cooldown| {
            kick_cooldown.0.tick(time.delta());

            if kick_cooldown.0.finished() {
                commands.entity(entity).remove::<KickCooldown>();
            }

            kick_cooldown.0.finished()
        })
    {
        if action_state.just_pressed(&PlayerActions::Kick) || kick_buffer.is_some() {
            let rotation_modifier = (transform.rotation.y + 0.5) * 2.0;
            let hitbox_position =
                transform.translation.xy() +
                (vec2(3.0, 0.0) / (CHUNK_SIZE as f32)) * rotation_modifier;
            let hitbox_size = vec2(36.0 / (CHUNK_SIZE as f32), 36.0 / (CHUNK_SIZE as f32));

            rapier_context.intersections_with_shape(
                hitbox_position,
                0.0,
                &Collider::cuboid(hitbox_size.x / 2.0, hitbox_size.y / 2.0),
                QueryFilter::new()
                    .exclude_solids()
                    .groups(
                        CollisionGroups::new(
                            Group::from_bits_retain(PLAYER_MASK),
                            Group::from_bits_retain(ENEMY_MASK | HITBOX_MASK)
                        )
                    ),
                |enemy_entity| {
                    let enemy_entity = rapier_context
                        .collider_parent(enemy_entity)
                        .unwrap_or(enemy_entity);

                    if let Ok(enemy_transform) = enemy_q.get_mut(enemy_entity) {
                        damage_ev.send(DamageEvent {
                            target: enemy_entity,
                            value: 4.0 * velocity.linvel.length(),
                            knockback: Vec2::new(
                                rotation_modifier * 4.0,
                                (enemy_transform.translation.y - transform.translation.y).clamp(
                                    -4.0,
                                    4.0
                                )
                            ) +
                            velocity.linvel / 2.0,
                        });
                    }
                    true
                }
            );

            commands.entity(entity).remove::<KickBuffer>();
            commands
                .entity(entity)
                .insert(KickCooldown(Timer::new(Duration::from_millis(250), TimerMode::Once)))
                .with_children(|parent| {
                    parent.spawn((
                        SpriteSheetBundle {
                            texture: sprites.smoke.clone(),
                            atlas: TextureAtlas {
                                layout: texture_atlas_layouts.add(
                                    TextureAtlasLayout::from_grid(
                                        Vec2::new(64.0, 64.0),
                                        11,
                                        22,
                                        None,
                                        None
                                    )
                                ),
                                index: 0,
                            },
                            transform: Transform {
                                translation: vec3(16.0, 0.0, PLAYER_Z),
                                scale: Vec3::splat(0.5),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        AnimationState::default(),
                        Animation(
                            benimator::Animation
                                ::from_indices(99..=110, FrameRate::from_fps(36.0))
                                .once()
                        ),
                        DespawnOnFinish,
                    ));
                });
        }
    } else if action_state.just_pressed(&PlayerActions::Kick) {
        commands
            .entity(entity)
            .insert(KickBuffer(Timer::new(Duration::from_millis(100), TimerMode::Once)));
    }
}

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct DashDuration(Timer);

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct DashBuffer(Timer);

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct DashCooldown(Timer);

pub fn player_dash(
    mut commands: Commands,
    mut player_q: Query<
        (
            Entity,
            &mut Velocity,
            &mut PlayerFlags,
            &Transform,
            &ActionState<PlayerActions>,
            Option<&mut DashCooldown>,
            Option<&mut DashBuffer>,
            Option<&mut DashDuration>,
        ),
        With<Player>
    >,
    time: Res<Time>
) {
    let (
        entity,
        mut velocity,
        mut flags,
        transform,
        action_state,
        mut dash_cooldown,
        mut dash_buffer,
        mut dash_duration,
    ) = player_q.single_mut();

    if let Some(timer) = dash_duration.as_mut() {
        timer.tick(time.delta());

        flags.remove(PlayerFlags::DASHING);

        if timer.finished() {
            commands.entity(entity).remove::<DashDuration>();
        }
    }

    if let Some(timer) = dash_buffer.as_mut() {
        timer.tick(time.delta());

        if timer.finished() {
            commands.entity(entity).remove::<DashBuffer>();
        }
    }

    let can_dash = dash_cooldown.as_mut().map_or(true, |cooldown| {
        cooldown.0.tick(time.delta());

        if cooldown.0.finished() {
            commands.entity(entity).remove::<DashCooldown>();
        }

        cooldown.0.finished()
    });

    if can_dash {
        if action_state.just_pressed(&PlayerActions::Dash) || dash_buffer.is_some() {
            velocity.linvel.x += (transform.rotation.y + 0.5) * 2.0 * 6.0;
            velocity.linvel.y = 1.0;

            flags.remove(PlayerFlags::JUMPING);
            flags.insert(PlayerFlags::DASHING);

            commands
                .entity(entity)
                .remove::<DashBuffer>()
                .insert(DashDuration(Timer::new(Duration::from_millis(50), TimerMode::Once)))
                .insert(DashCooldown(Timer::new(Duration::from_millis(500), TimerMode::Once)));
        }
    } else if action_state.just_pressed(&PlayerActions::Dash) {
        commands
            .entity(entity)
            .insert(DashBuffer(Timer::new(Duration::from_millis(100), TimerMode::Once)));
    }
}

#[derive(Component)]
pub struct Rope {
    pub source: Entity,
    pub position: Vec2,
}

#[derive(Component)]
pub struct RopeAnchor;

#[derive(Component)]
pub struct RopeAim;

pub fn player_hook(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    sprites: Res<SpriteSheets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut player_q: Query<
        (
            Entity,
            &mut Actor,
            &Transform,
            &mut PlayerFlags,
            &ActionState<PlayerActions>,
            Option<&mut ImpulseJoint>,
        ),
        With<Player>
    >,
    mut aim_q: Query<(Entity, &mut Transform), (With<RopeAim>, Without<Player>)>,
    object_q: Query<(&GlobalTransform, Option<&Parent>, &Collider), Without<Player>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>,
    window_q: Query<&Window, With<PrimaryWindow>>
) {
    let (entity, mut actor, transform, mut flags, action_state, joint) = player_q.single_mut();
    let (camera, camera_transform) = camera_q.single();

    // TODO: add removal of anchor if terrain was changed
    if let Some(joint) = &joint {
        if commands.get_entity(joint.parent).is_none() {
            flags.remove(PlayerFlags::HOOKED);
            commands.entity(entity).remove::<ImpulseJoint>();
        }
    }

    if action_state.pressed(&PlayerActions::Hook) {
        if joint.is_some() {
            flags.remove(PlayerFlags::HOOKED);
            commands.entity(entity).remove::<ImpulseJoint>();
        } else if let Some(cursor_position) = window_q.single().cursor_position() {
            let point = camera
                .viewport_to_world(camera_transform, cursor_position)
                .map(|ray| ray.origin.truncate())
                .unwrap();

            let position =
                transform.translation.xy() +
                (point - transform.translation.xy()).clamp_length_max(2.0);

            if let Ok((entity, mut aim_transform)) = aim_q.get_single_mut() {
                aim_transform.translation = position.extend(1.0);
            } else {
                commands.spawn((
                    RopeAim,
                    SpriteBundle {
                        texture: sprites.rope_end.clone(),
                        transform: Transform::from_translation(position.extend(1.0)).with_scale(
                            Vec3::splat((1.0 / (CHUNK_SIZE as f32)) * 2.0)
                        ),
                        ..Default::default()
                    },
                ));
            }
        }
    }

    if action_state.just_released(&PlayerActions::Hook) {
        if let Ok((entity, _)) = aim_q.get_single() {
            commands.entity(entity).despawn();
        }

        if let Some(cursor_position) = window_q.single().cursor_position() {
            let point = camera
                .viewport_to_world(camera_transform, cursor_position)
                .map(|ray| ray.origin.truncate())
                .unwrap();

            let direction = (point - transform.translation.xy()).normalize_or_zero();

            if
                let Some((object_entity, toi)) = rapier_context.cast_ray(
                    transform.translation.xy(),
                    direction,
                    2.0,
                    true,
                    QueryFilter::only_fixed()
                )
            {
                let (object_transform, parent, collider) = object_q.get(object_entity).unwrap();

                let point = transform.translation.xy() + direction * toi;

                actor.flags.insert(ActorFlags::INFLUENCED);
                let joint = SpringJointBuilder::new((direction * toi).length() * 0.5, 0.25, 0.05)
                    .local_anchor1(point - object_transform.translation().xy())
                    .local_anchor2(Vec2::ZERO);

                let length = (point - transform.translation.xy()).length();

                let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
                    .with_inserted_attribute(
                        Mesh::ATTRIBUTE_POSITION,
                        vec![
                            [1.0, (length * (CHUNK_SIZE as f32)) / 2.0, 0.0],
                            [-1.0, (length * (CHUNK_SIZE as f32)) / 2.0, 0.0],
                            [-1.0, (-length * (CHUNK_SIZE as f32)) / 2.0, 0.0],
                            [1.0, (-length * (CHUNK_SIZE as f32)) / 2.0, 0.0]
                        ]
                    )
                    .with_inserted_attribute(
                        Mesh::ATTRIBUTE_UV_0,
                        vec![
                            [length * 8.0, 0.0],
                            [0.0, 0.0],
                            [0.0, length * 8.0],
                            [1.0, length * 8.0]
                        ]
                    );

                flags.insert(PlayerFlags::HOOKED);

                commands.entity(entity).insert(ImpulseJoint::new(parent.unwrap().get(), joint));
                commands
                    .spawn((
                        Rope {
                            source: entity,
                            position: point,
                        },
                        MaterialMesh2dBundle {
                            mesh: meshes.add(mesh).into(),
                            material: materials.add(ColorMaterial {
                                texture: Some(sprites.rope.clone()),
                                ..Default::default()
                            }),
                            transform: Transform {
                                translation: ((transform.translation.xy() + point) / 2.0).extend(
                                    0.0
                                ),
                                scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                                rotation: Quat::from_rotation_z(direction.to_angle() - PI / 2.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            RopeAnchor,
                            SpriteBundle {
                                transform: Transform::from_translation(Vec3::new(0.0, length, 0.0)),
                                texture: sprites.rope_end.clone(),
                                ..Default::default()
                            },
                        ));
                    });
            }
        }
    } else if action_state.released(&PlayerActions::Hook) && !aim_q.is_empty() {
        if let Ok((entity, _)) = aim_q.get_single() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn update_rope_position(
    mut commands: Commands,
    mut rope_q: Query<
        (Entity, &Rope, &mut Transform, &Mesh2dHandle, &Children),
        (With<Rope>, Without<Actor>)
    >,
    mut rope_end_q: Query<&mut Transform, (With<RopeAnchor>, Without<Actor>, Without<Rope>)>,
    actor_q: Query<(&Transform, Option<&ImpulseJoint>), With<Actor>>,
    mut meshes: ResMut<Assets<Mesh>>
) {
    for (entity, rope, mut transform, mesh_handle, children) in rope_q.iter_mut() {
        let (actor_transform, actor_joint) = actor_q.get(rope.source).unwrap();
        if actor_joint.is_none() {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        let mesh = meshes.get_mut(mesh_handle.0.clone()).unwrap();

        transform.translation.x = (actor_transform.translation.x + rope.position.x) / 2.0;
        transform.translation.y = (actor_transform.translation.y + rope.position.y) / 2.0;

        let direction = (rope.position.xy() - actor_transform.translation.xy()).normalize_or_zero();
        let length = (rope.position.xy() - actor_transform.translation.xy()).length();

        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [1.0, (length * (CHUNK_SIZE as f32)) / 2.0, 0.0],
                [-1.0, (length * (CHUNK_SIZE as f32)) / 2.0, 0.0],
                [-1.0, (-length * (CHUNK_SIZE as f32)) / 2.0, 0.0],
                [1.0, (-length * (CHUNK_SIZE as f32)) / 2.0, 0.0]
            ]
        );

        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[length * 8.0, 0.0], [0.0, 0.0], [0.0, length * 8.0], [1.0, length * 8.0]]
        );

        transform.rotation = Quat::from_rotation_z(direction.to_angle() - PI / 2.0);

        if let Ok(mut transform) = rope_end_q.get_mut(*children.first().unwrap()) {
            transform.translation.y = (length * (CHUNK_SIZE as f32)) / 2.0;
        }
    }
}

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct ShootBuffer(Timer);

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct ShootCooldown(Timer);

pub fn player_shoot(
    mut commands: Commands,
    mut player_q: Query<
        (
            Entity,
            &Transform,
            &ActionState<PlayerActions>,
            Option<&mut ShootCooldown>,
            Option<&mut ShootBuffer>,
        ),
        With<Player>
    >,
    time: Res<Time>,
    registries: Res<Registries>
) {
    let (entity, transform, action_state, mut shoot_cooldown, mut shoot_buffer) =
        player_q.single_mut();

    if let Some(timer) = shoot_buffer.as_mut() {
        timer.tick(time.delta());

        if timer.finished() {
            commands.entity(entity).remove::<ShootBuffer>();
        }
    }

    let can_shoot = shoot_cooldown.as_mut().map_or(true, |shoot_cooldown| {
        shoot_cooldown.0.tick(time.delta());

        if shoot_cooldown.0.finished() {
            commands.entity(entity).remove::<ShootCooldown>();
        }

        shoot_cooldown.0.finished()
    });

    if can_shoot {
        if action_state.just_pressed(&PlayerActions::Shoot) || shoot_buffer.is_some() {
            let rotation_modifier = (transform.rotation.y + 0.5) * 2.0;
            let hitbox_position =
                transform.translation.xy() +
                (vec2(16.0, 0.0) / (CHUNK_SIZE as f32)) * rotation_modifier;

            let size: i32 = 9;

            let sand = registries.materials.get("sand").unwrap();
            let mut pixels = vec![None; size.pow(2) as usize];

            for (x, y) in (0..size).cartesian_product(0..size) {
                if
                    (ivec2(x, y).as_vec2() - (size as f32) / 2.0).length_squared() >
                    ((size as f32) / 2.0).powi(2)
                {
                    continue;
                }

                pixels[(y * size + x) as usize] = Some(Pixel::from(sand));
            }

            if let Ok(object) = Object::from_pixels(pixels, IVec2::splat(size)) {
                if let Ok(collider) = object.create_collider() {
                    commands.spawn((
                        FallApartOnCollision,
                        ObjectBundle {
                            object,
                            collider,
                            transform: TransformBundle {
                                local: Transform::from_translation(hitbox_position.extend(0.0)),
                                ..Default::default()
                            },
                            velocity: Velocity::linear(Vec2::new(4.0 * rotation_modifier, 0.2)),
                            mass_properties: ColliderMassProperties::Density(2.0),
                            ..Default::default()
                        },
                    ));
                }
            }

            commands.entity(entity).remove::<ShootBuffer>();
            commands
                .entity(entity)
                .insert(ShootCooldown(Timer::new(Duration::from_millis(250), TimerMode::Once)));
        }
    } else if action_state.just_pressed(&PlayerActions::Shoot) {
        commands
            .entity(entity)
            .insert(ShootBuffer(Timer::new(Duration::from_millis(100), TimerMode::Once)));
    }
}

#[derive(Default, Resource, Deref, DerefMut)]
pub struct PlayerMaterials(IndexMap<String, f32>);

#[derive(Resource, Reflect, Deref, DerefMut)]
pub struct PlayerSelectedMaterial(pub String);

impl Default for PlayerSelectedMaterial {
    fn default() -> Self {
        Self("sand".to_string())
    }
}

#[derive(Default, Resource, Deref, DerefMut)]
pub struct PlayerTrackingParticles(Vec<(String, Entity)>);

pub fn player_collect_sand(
    mut commands: Commands,
    player_q: Query<(Entity, &Transform, &ActionState<PlayerActions>), With<Player>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut tracked_particles: ResMut<PlayerTrackingParticles>,
    mut dirty_rects: ResMut<DirtyRects>,
    particle_q: Query<&Particle>,
    mut player_materials: ResMut<PlayerMaterials>
) {
    let (entity, transform, action_state) = player_q.single();

    tracked_particles.retain_mut(|(id, entity)| {
        if !particle_q.contains(*entity) {
            *player_materials.entry(id.clone()).or_insert(0.0) += 1.0 / 16.0;

            return false;
        }

        true
    });

    if action_state.pressed(&PlayerActions::Collect) {
        let chunk_position = transform.translation.xy().floor().as_ivec2();
        let player_position = (transform.translation.xy().fract() * (CHUNK_SIZE as f32)).as_ivec2();

        let Some(mut chunk_group) = build_chunk_group(&mut chunk_manager, chunk_position) else {
            return;
        };

        let radius = 16;
        for x in -radius..=radius {
            for y in -radius..=radius {
                let position = ivec2(x, y);

                if position.length_squared() > radius.pow(2) {
                    continue;
                }

                let Some(pixel) = chunk_group.get_mut(player_position + position) else {
                    continue;
                };

                if
                    matches!(
                        pixel.physics_type,
                        PhysicsType::Powder | PhysicsType::Liquid(..) | PhysicsType::Gas
                    )
                {
                    let pixel = mem::take(pixel);

                    tracked_particles.push((
                        pixel.id.clone(),
                        commands
                            .spawn(ParticleBundle {
                                sprite: SpriteBundle {
                                    sprite: Sprite {
                                        color: Color::rgba_u8(
                                            pixel.color[0],
                                            pixel.color[1],
                                            pixel.color[2],
                                            pixel.color[3]
                                        ),
                                        custom_size: Some(Vec2::ONE / (CHUNK_SIZE as f32)),
                                        ..Default::default()
                                    },
                                    transform: Transform::from_translation(
                                        (
                                            transform.translation.xy() +
                                            vec2(x as f32, y as f32) / (CHUNK_SIZE as f32)
                                        ).extend(PARTICLE_Z)
                                    ),
                                    ..Default::default()
                                },
                                movement: ParticleMovement::Follow(entity),
                                particle: Particle::new(pixel),
                                ..Default::default()
                            })
                            .id(),
                    ));

                    dirty_rects.request_update(
                        player_position + position + chunk_position * CHUNK_SIZE
                    );
                    dirty_rects.request_render(
                        player_position + position + chunk_position * CHUNK_SIZE
                    );
                }
            }
        }
    }
}

pub fn player_switch_material(
    player_q: Query<&ActionState<PlayerActions>, With<Player>>,
    mut selected_material: ResMut<PlayerSelectedMaterial>,
    player_materials: Res<PlayerMaterials>
) {
    let action_state = player_q.single();
    let index = player_materials.get_index_of(&selected_material.0).unwrap_or(0);

    if action_state.just_pressed(&PlayerActions::SelectMaterialNext) {
        selected_material.0 = player_materials
            .get_index(((index as i32) - 1).rem_euclid(player_materials.len() as i32) as usize)
            .unwrap()
            .0.clone();
    } else if action_state.just_pressed(&PlayerActions::SelectMaterialPrevious) {
        selected_material.0 = player_materials
            .get_index(((index as i32) + 1).rem_euclid(player_materials.len() as i32) as usize)
            .unwrap()
            .0.clone();
    }
}

pub fn player_prune_empty_materials(
    selected_material: Res<PlayerSelectedMaterial>,
    mut player_materials: ResMut<PlayerMaterials>
) {
    player_materials.retain(|id, value| {
        if *value < 1.0 / 16.0 && &selected_material.0 != id {
            return false;
        }

        true
    });
}

pub fn update_player_rotation(
    mut player_q: Query<(&mut Transform, &Velocity), With<Player>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>
) {
    let (mut transform, velocity) = player_q.single_mut();
    let (camera, camera_global_transform) = camera_q.single();

    if
        window_q
            .get_single()
            .ok()
            .filter(|window| window.cursor_position().is_some())
            .map_or(velocity.linvel.x.is_sign_negative(), |window| {
                let point = camera
                    .viewport_to_world(camera_global_transform, window.cursor_position().unwrap())
                    .map(|ray| ray.origin.truncate())
                    .unwrap();

                (point.yx() - transform.translation.yx()).to_angle().is_sign_negative()
            })
    {
        transform.rotation = Quat::from_rotation_y(-(180f32).to_radians());
    }
}

// pub fn raycast_from_player(
//     mut gizmos: Gizmos,
//     mut player_q: Query<&mut Transform, With<Player>>,
//     camera_q: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>,
//     window_q: Query<&Window, With<PrimaryWindow>>,
//     chunk_manager: Res<ChunkManager>
// ) {
//     let transform = player_q.single_mut();
//     let (camera, camera_transform) = camera_q.single();

//     if let Some(cursor_position) = window_q.single().cursor_position() {
//         let player_position = (transform.translation.xy() * (CHUNK_SIZE as f32)).round().as_ivec2();

//         let pixel_position = (
//             camera
//                 .viewport_to_world(camera_transform, cursor_position)
//                 .map(|ray| ray.origin.truncate())
//                 .unwrap() * (CHUNK_SIZE as f32)
//         )
//             .round()
//             .as_ivec2();

//         // if let Some((point, _)) = raycast(player_position, pixel_position, &chunk_manager) {
//         //     gizmos.line_2d(
//         //         transform.translation.xy(),
//         //         point.as_vec2() / (CHUNK_SIZE as f32),
//         //         Color::RED
//         //     );
//         // } else {
//         //     gizmos.line_2d(
//         //         transform.translation.xy(),
//         //         pixel_position.as_vec2() / (CHUNK_SIZE as f32),
//         //         Color::BLUE
//         //     );
//         // }
//     }
// }

// pub fn get_object_by_click(
//     mut dirty_rects_resource: ResMut<DirtyRects>,
//     buttons: Res<ButtonInput<MouseButton>>,
//     rapier_context: Res<RapierContext>,
//     window_q: Query<(Entity, &Window), With<PrimaryWindow>>,
//     camera_q: Query<(&Camera, &GlobalTransform), With<Camera>>,
//     mut object_q: Query<(&Transform, &mut Object)>,
//     mut egui_context: EguiContexts,
//     mut events: EventWriter<ToastEvent>
// ) {
//     let (window_entity, window) = window_q.single();
//     let (camera, camera_global_transform) = camera_q.single();

//     if
//         buttons.just_pressed(MouseButton::Middle) &&
//         egui_context
//             .try_ctx_for_window_mut(window_entity)
//             .map_or(true, |ctx| !ctx.is_pointer_over_area())
//     {
//         if let Some(position) = window.cursor_position() {
//             let point = camera
//                 .viewport_to_world(camera_global_transform, position)
//                 .map(|ray| ray.origin.truncate())
//                 .unwrap();
//         }
//     }
// }
