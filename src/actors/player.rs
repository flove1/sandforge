use std::f32::consts::PI;
use std::mem;
use std::time::Duration;

use benimator::FrameRate;
use bevy::audio::Volume;

use bevy::render::view::RenderLayers;
use bevy::{
    prelude::*,
    render::{ mesh::{ Indices, PrimitiveTopology }, render_asset::RenderAssetUsages },
    sprite::{ MaterialMesh2dBundle, Mesh2dHandle },
    window::PrimaryWindow,
};
use bevy_math::{ ivec2, vec2, vec3 };
use bevy_rapier2d::geometry::Sensor;
use bevy_rapier2d::{
    dynamics::{ ImpulseJoint, SpringJointBuilder, Velocity },
    geometry::{ Collider, ColliderMassProperties, CollisionGroups, Group },
};
use bevy_rapier2d::{ pipeline::QueryFilter, plugin::RapierContext };
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

use crate::{
    animation::{ Animation, AnimationState, DespawnOnFinish },
    assets::{ AudioAssetCollection, SpriteAssetCollection },
    camera::{ TrackingCamera, ACTOR_RENDER_LAYER, LIGHTING_RENDER_LAYER },
    constants::{ CHUNK_SIZE, PARTICLE_Z, PLAYER_Z },
    raycast::raycast,
    registries::Registries,
    simulation::{
        chunk_groups::build_chunk_group,
        chunk_manager::ChunkManager,
        colliders::{ ENEMY_MASK, HITBOX_MASK, PLAYER_MASK },
        dirty_rect::DirtyRects,
        materials::PhysicsType,
        object::{ Object, ObjectBundle, Projectile },
        particle::{ Particle, ParticleBundle, ParticleMovement },
        pixel::Pixel,
    },
};

use super::{
    actor::{
        Actor,
        ActorBundle,
        ActorFlags,
        ActorHitboxBundle,
        AttackParameters,
        MovementType,
        StorredRotation,
    },
    animation::{
        create_animation_end_trigger,
        create_run_trigger,
        AttackAnimation,
        FallAnimation,
        IdleAnimation,
        JumpAnimation,
        LandAnimation,
        MoveAnimation,
    },
    enemy::Enemy,
    health::{ DamageEvent, KnockbackResistance },
};

use bitflags::bitflags;

pub const ATLAS_COLUMNS: usize = 9;
pub const ATLAS_ROWS: usize = 18;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum PlayerActions {
    Run,
    Crouch,
    Jump,
    Attack,
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
struct JumpIntroAnimation;

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
        const ATTACKING = 1 << 5;
    }
}

#[derive(Component, Clone)]
pub struct Player;

pub fn player_reset_position(
    mut player_q: Query<(&mut Actor, &mut Transform), With<Player>>,
    mut camera_q: Query<&mut TrackingCamera>
) {
    if let Ok((mut actor, mut transform)) = player_q.get_single_mut() {
        actor.position = vec2(-actor.size.x / 2.0, 0.0);
        transform.translation.x = 0.0;
        transform.translation.y = actor.size.y / 2.0;
        camera_q
            .single_mut()
            .set_position((actor.position + actor.size / 2.0) / (CHUNK_SIZE as f32));
        return;
    } else {
        camera_q.single_mut().set_position(Vec2::new(0.0, 17.0 / 2.0) / (CHUNK_SIZE as f32));
    }
}

pub fn player_setup(
    mut commands: Commands,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    sprites: Res<SpriteAssetCollection>
) {
    let mut player_materials = PlayerMaterials::default();
    player_materials.insert("healium".into(), 100.0);

    commands.insert_resource(player_materials);
    commands.insert_resource(PlayerSelectedMaterial::default());

    let mut entity_commands = commands.spawn((
        Name::new("Player"),
        Player,
        PlayerFlags::default(),
        SpatialListener::new(0.5),
        ActorBundle {
            actor: Actor {
                position: vec2(-5.0, 0.0),
                size: Vec2::new(10.0, 17.0),
                movement_type: MovementType::Walking {
                    speed: 1.5,
                    jump_height: 1.0,
                },
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
                    translation: vec3(0.0, 17.0 / 2.0, PLAYER_Z),
                    scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
    ));

    let run_trigger = create_run_trigger(0.25);

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

    entity_commands.insert((
        // AnimationState::default(),
        // Animation(benimator::Animation::from_indices(0..=5, FrameRate::from_fps(8.0)).repeat()),
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
            .trans::<AnyState, _>(
                move |player_q: Query<(Option<&AttackAnimation>, &PlayerFlags), With<Player>>| {
                    let (animation, flags) = player_q.get_single().unwrap();

                    if flags.contains(PlayerFlags::ATTACKING) && animation.is_none() {
                        Ok(())
                    } else {
                        Err(())
                    }
                },
                AttackAnimation
            )
            .trans::<IdleAnimation, _>(run_trigger, MoveAnimation)
            .trans::<MoveAnimation, _>(run_trigger.not(), IdleAnimation)
            .trans::<AnyState, _>(jump_start_trigger, JumpIntroAnimation)
            .trans::<DashAnimation, _>(create_animation_end_trigger(), IdleAnimation)
            .trans::<AttackAnimation, _>(create_animation_end_trigger(), IdleAnimation)
            .trans::<JumpIntroAnimation, _>(create_animation_end_trigger(), JumpAnimation)
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
                        (&PlayerFlags, &Actor, &Velocity, Option<&FallAnimation>),
                        With<Player>
                    >
                | {
                    let (flags, actor, velocity, falling_animation) = player_q.single();

                    match
                        ((!flags.contains(PlayerFlags::DASHING) &&
                            !flags.contains(PlayerFlags::ATTACKING) &&
                            falling_animation.is_none() &&
                            velocity.linvel.y < -1.0) ||
                            (flags.contains(PlayerFlags::HOOKED) &&
                                !flags.contains(PlayerFlags::ATTACKING))) &&
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
            .trans::<LandAnimation, _>(create_animation_end_trigger(), IdleAnimation)
            .on_enter::<IdleAnimation>(|entity| {
                entity.insert(
                    Animation(
                        benimator::Animation::from_indices(0..=5, FrameRate::from_fps(8.0)).repeat()
                    )
                );
                entity.insert(AnimationState::default());
            })
            .on_enter::<MoveAnimation>(|entity| {
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
                        benimator::Animation::from_indices(72..=73, FrameRate::from_fps(8.0)).once()
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
                        benimator::Animation::from_indices(63..=67, FrameRate::from_fps(8.0)).once()
                    )
                );
                entity.insert(AnimationState::default());
            })
            .on_enter::<AttackAnimation>(|entity| {
                entity.insert(
                    Animation(
                        benimator::Animation
                            ::from_indices(153..=155, FrameRate::from_fps(8.0))
                            .once()
                    )
                );
                entity.insert(AnimationState::default());
            })
            .command_on_exit::<AttackAnimation>(|world: &mut World| {
                let mut flags = world.query::<&mut PlayerFlags>().single_mut(world);
                flags.remove(PlayerFlags::ATTACKING)
            })
            .command_on_exit::<DashAnimation>(|world: &mut World| {
                let mut flags = world.query::<&mut PlayerFlags>().single_mut(world);
                flags.remove(PlayerFlags::DASHING)
            }),
    ));

    entity_commands.insert((
        InputManagerBundle::with_map(
            InputMap::default()
                .insert(PlayerActions::Run, VirtualAxis::ad())
                .insert(PlayerActions::Jump, KeyCode::Space)
                .insert(PlayerActions::Attack, KeyCode::KeyF)
                .insert(PlayerActions::Crouch, KeyCode::KeyS)
                .insert(PlayerActions::Dash, KeyCode::KeyQ)
                .insert(PlayerActions::Hook, MouseButton::Right)
                .insert(PlayerActions::Interaction, KeyCode::KeyE)
                .insert(PlayerActions::Shoot, KeyCode::KeyR)
                .insert(PlayerActions::Collect, KeyCode::KeyG)
                .insert(PlayerActions::SelectMaterialNext, MouseWheelDirection::Up)
                .insert(PlayerActions::SelectMaterialPrevious, MouseWheelDirection::Down)
                .build()
        ),
    ));

    entity_commands.insert(AttackParameters {
        value: 2.0,
        knockback_strength: 1.0,
    });

    entity_commands.insert(InventoryParameters {
        max_storage: 100.0,
    });

    entity_commands.insert(KnockbackResistance(0.0));

    entity_commands.with_children(|parent| {
        parent.spawn((
            ColliderMassProperties::Mass(0.0),
            ActorHitboxBundle {
                collider: Collider::capsule_y(17.0 / 2.0 - 10.0 / 2.0 - 2.5, 10.0 / 2.0 + 4.0),
                collision_groups: CollisionGroups::new(
                    Group::from_bits_retain(PLAYER_MASK | HITBOX_MASK),
                    Group::from_bits_retain(ENEMY_MASK)
                ),
                ..Default::default()
            },
        ));

        parent.spawn((
            Name::new("Player's lighting"),
            ColorMesh2dBundle {
                mesh: meshes.add(Mesh::from(Circle::new(6.0))).into(),
                material: materials.add(Color::WHITE.with_a(0.5)),
                transform: Transform::from_xyz(0.0, 0.0, -10.0),
                ..Default::default()
            },
            RenderLayers::layer(LIGHTING_RENDER_LAYER),
        ));

        // parent.spawn((
        //     SpriteSheetBundle {
        //         texture: sprites.heal.clone(),
        //         atlas: TextureAtlas {
        //             layout: texture_atlas_layouts.add(
        //                 TextureAtlasLayout::from_grid(Vec2::new(48.0, 48.0), 22, 1, None, None)
        //             ),
        //             index: 0,
        //         },
        //         ..Default::default()
        //     },

        //     AnimationState::default(),
        //     Animation(
        //         benimator::Animation::from_indices(0..=21, FrameRate::from_fps(8.0)).repeat()
        //     ),
        // ));
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
            &Actor,
            &mut Velocity,
            &mut PlayerFlags,
            &ActionState<PlayerActions>,
            Option<&mut JumpBuffer>,
        ),
        With<Player>
    >,
    time: Res<Time>
) {
    let (entity, actor, mut velocity, mut flags, action_state, mut jump_buffer) =
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
pub struct AttackBuffer(Timer);

#[derive(Component, Deref, DerefMut)]
#[component(storage = "SparseSet")]
pub struct AttackCooldown(Timer);

#[derive(Component)]
pub struct AttackSFX;

pub fn player_attack(
    mut commands: Commands,
    mut player_q: Query<
        (
            Entity,
            &mut PlayerFlags,
            &Transform,
            &Velocity,
            &AttackParameters,
            &ActionState<PlayerActions>,
            Option<&mut AttackCooldown>,
            Option<&mut AttackBuffer>,
        ),
        (With<Player>, Without<Enemy>)
    >,
    mut enemy_q: Query<&Transform, With<Enemy>>,
    mut damage_ev: EventWriter<DamageEvent>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    time: Res<Time>,
    rapier_context: Res<RapierContext>,
    sprites: Res<SpriteAssetCollection>,
    audio: Res<AudioAssetCollection>,
    cursor_position: Option<Res<CursorPosition>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects: ResMut<DirtyRects>
) {
    let (
        entity,
        mut flags,
        transform,
        velocity,
        attack_parameters,
        action_state,
        mut cooldown,
        mut buffer,
    ) = player_q.single_mut();

    if let Some(buffer) = buffer.as_mut() {
        buffer.tick(time.delta());

        if buffer.0.finished() {
            commands.entity(entity).remove::<AttackBuffer>();
        }
    }

    let can_attack = cooldown.as_mut().map_or(true, |cooldown| {
        cooldown.0.tick(time.delta());

        if cooldown.0.finished() {
            commands.entity(entity).remove::<AttackCooldown>();
        }

        cooldown.0.finished()
    });

    let Some(cursor_position) = cursor_position else {
        return;
    };

    if can_attack {
        if action_state.just_pressed(&PlayerActions::Attack) || buffer.is_some() {
            let rotation_modifier = (transform.rotation.y + 0.5) * 2.0;

            let pixel_radius = 18;
            let hitbox_size = (pixel_radius as f32) / (CHUNK_SIZE as f32);
            let hitbox_position =
                transform.translation.xy() +
                (((pixel_radius as f32) * 0.75) / (CHUNK_SIZE as f32)) * cursor_position.direction;

            rapier_context.intersections_with_shape(
                hitbox_position,
                0.0,
                &Collider::ball(hitbox_size),
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
                            value: attack_parameters.value + 2.0 * velocity.linvel.length(),
                            knockback: Vec2::new(
                                rotation_modifier * 4.0,
                                (enemy_transform.translation.y - transform.translation.y).clamp(
                                    -4.0,
                                    4.0
                                )
                            ) +
                            velocity.linvel / 2.0,
                            ignore_iframes: false,
                            play_sound: true,
                        });
                    }
                    true
                }
            );

            commands
                .entity(entity)
                .remove::<AttackBuffer>()
                .insert(AttackCooldown(Timer::new(Duration::from_millis(500), TimerMode::Once)))
                .insert(AudioBundle {
                    source: audio.slash.clone().into(),
                    settings: PlaybackSettings {
                        volume: Volume::new(0.5),
                        mode: bevy::audio::PlaybackMode::Remove,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        AttackSFX,
                        SpriteSheetBundle {
                            texture: sprites.attack.clone(),
                            atlas: TextureAtlas {
                                layout: texture_atlas_layouts.add(
                                    TextureAtlasLayout::from_grid(
                                        Vec2::new(48.0, 48.0),
                                        5,
                                        1,
                                        None,
                                        None
                                    )
                                ),
                                index: 0,
                            },
                            transform: Transform {
                                // translation: vec3(16.0, 0.0, PLAYER_Z ),
                                // scale: Vec3::splat(0.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        AnimationState::default(),
                        Animation(
                            benimator::Animation
                                ::from_indices(0..=2, FrameRate::from_fps(12.0))
                                .once()
                        ),
                        DespawnOnFinish,
                    ));
                });

            flags.insert(PlayerFlags::ATTACKING);

            let center = (
                transform.translation.xy() * (CHUNK_SIZE as f32) +
                (pixel_radius as f32) * 0.5 * cursor_position.direction
            ).as_ivec2();
            let chunk_position = center.div_euclid(IVec2::splat(CHUNK_SIZE));
            let pixel_radius = ((pixel_radius as f32) * 0.75) as i32;

            if let Some(mut chunk_group) = build_chunk_group(&mut chunk_manager, chunk_position) {
                for x in -pixel_radius..=pixel_radius {
                    for y in -pixel_radius..=pixel_radius {
                        let offset = IVec2::new(x, y);

                        if offset.length_squared() > pixel_radius.pow(2) {
                            continue;
                        }

                        let Some(pixel) = chunk_group
                            .get_mut(center - chunk_position * CHUNK_SIZE + offset)
                            .map(|pixel| mem::take(pixel)) else {
                            continue;
                        };

                        if
                            let Some(particle) = match pixel.physics_type {
                                | PhysicsType::Powder
                                | PhysicsType::Liquid(_)
                                | PhysicsType::Gas(_) => {
                                    Some(Particle::new(pixel.clone()))
                                }
                                PhysicsType::Static => { Some(Particle::visual(pixel.clone())) }
                                _ => { None }
                            }
                        {
                            commands.spawn(ParticleBundle {
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
                                        ((center + offset).as_vec2() / (CHUNK_SIZE as f32)).extend(
                                            PARTICLE_Z
                                        )
                                    ),
                                    ..Default::default()
                                },
                                velocity: Velocity::linear(
                                    vec2(fastrand::f32() - 0.5, fastrand::f32() * 0.5 + 1.0) /
                                        (CHUNK_SIZE as f32)
                                ),
                                particle,
                                ..Default::default()
                            });
                        }

                        dirty_rects.request_update_3x3(center + offset);
                        dirty_rects.request_render(center + offset);
                        dirty_rects.collider.insert(
                            (center + offset).div_euclid(IVec2::splat(CHUNK_SIZE))
                        );
                    }
                }
            }
        }
    } else if action_state.just_pressed(&PlayerActions::Attack) {
        commands
            .entity(entity)
            .insert(AttackBuffer(Timer::new(Duration::from_millis(100), TimerMode::Once)));
    }
}

pub fn player_synchronize_attack_rotation(
    player_q: Query<(&Transform, &Children), With<Player>>,
    mut sfx_q: Query<&mut Transform, (With<Parent>, With<AttackSFX>, Without<Player>)>,
    cursor_position: Option<Res<CursorPosition>>
) {
    let (player_transform, children) = player_q.single();
    let Some(cursor_position) = cursor_position else {
        return;
    };

    for entity in children.iter() {
        if let Ok(mut sfx_transform) = sfx_q.get_mut(*entity) {
            if player_transform.rotation.y == 0.0 {
                sfx_transform.rotation = Quat::from_rotation_z(cursor_position.angle);
            } else {
                sfx_transform.rotation = Quat::from_rotation_z(
                    PI * cursor_position.angle.signum() - cursor_position.angle
                );
            }
        }
    }
}

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
    ) = player_q.single_mut();

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

    if can_dash && !flags.contains(PlayerFlags::DASHING) {
        if action_state.just_pressed(&PlayerActions::Dash) || dash_buffer.is_some() {
            velocity.linvel.x += (transform.rotation.y + 0.5) * 2.0 * 6.0;
            velocity.linvel.y = 1.0;

            flags.remove(PlayerFlags::JUMPING);
            flags.insert(PlayerFlags::DASHING);

            commands
                .entity(entity)
                .remove::<DashBuffer>()
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
    pub initial_angle: f32,
}

#[derive(Component)]
pub struct RopeAnchor;

pub fn player_hook(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    sprites: Res<SpriteAssetCollection>,
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
    object_q: Query<&GlobalTransform, (With<Collider>, Without<Player>)>,
    cursor_position: Option<Res<CursorPosition>>
) {
    let (entity, mut actor, transform, mut flags, action_state, joint) = player_q.single_mut();

    if let Some(joint) = &joint {
        if commands.get_entity(joint.parent).is_none() {
            flags.remove(PlayerFlags::HOOKED);
            commands.entity(entity).remove::<ImpulseJoint>();
        }
    }

    let Some(cursor_position) = cursor_position else {
        return;
    };

    if action_state.just_pressed(&PlayerActions::Hook) {
        if
            let Some((object_entity, toi)) = rapier_context.cast_ray(
                transform.translation.xy(),
                cursor_position.direction,
                2.0,
                true,
                QueryFilter::only_fixed()
            )
        {
            let Ok(object_transform) = object_q.get(object_entity) else {
                return;
            };

            let point = transform.translation.xy() + cursor_position.direction * toi;

            actor.flags.insert(ActorFlags::INFLUENCED);
            let joint = SpringJointBuilder::new(
                (cursor_position.direction * toi).length() * 0.5,
                0.25,
                0.05
            )
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
                    vec![[length * 8.0, 0.0], [0.0, 0.0], [0.0, length * 8.0], [1.0, length * 8.0]]
                );

            flags.insert(PlayerFlags::HOOKED);

            commands
                .entity(entity)
                .insert(
                    ImpulseJoint::new(
                        rapier_context.collider_parent(object_entity).unwrap_or(object_entity),
                        joint
                    )
                );
            commands
                .spawn((
                    Rope {
                        source: entity,
                        position: point,
                        initial_angle: cursor_position.angle,
                    },
                    MaterialMesh2dBundle {
                        mesh: meshes.add(mesh).into(),
                        material: materials.add(ColorMaterial {
                            texture: Some(sprites.rope.clone()),
                            ..Default::default()
                        }),
                        transform: Transform {
                            translation: ((transform.translation.xy() + point) / 2.0).extend(
                                PLAYER_Z - 1.0
                            ),
                            scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                            rotation: Quat::from_rotation_z(cursor_position.angle - PI / 2.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    RenderLayers::layer(ACTOR_RENDER_LAYER),
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
    } else if action_state.released(&PlayerActions::Hook) {
        if joint.is_some() {
            flags.remove(PlayerFlags::HOOKED);
            commands.entity(entity).remove::<ImpulseJoint>();
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
    mut actor_q: Query<(&Transform, &mut PlayerFlags, Option<&ImpulseJoint>), With<Actor>>,
    mut meshes: ResMut<Assets<Mesh>>,
    chunk_manager: Res<ChunkManager>,
    rapier_context: Res<RapierContext>
) {
    for (entity, rope, mut transform, mesh_handle, children) in rope_q.iter_mut() {
        let Ok((actor_transform, mut actor_flags, actor_joint)) = actor_q.get_mut(
            rope.source
        ) else {
            commands.entity(entity).despawn_recursive();
            continue;
        };

        if actor_joint.is_none() {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        transform.translation.x = (actor_transform.translation.x + rope.position.x) / 2.0;
        transform.translation.y = (actor_transform.translation.y + rope.position.y) / 2.0;

        let direction = (rope.position.xy() - actor_transform.translation.xy()).normalize_or_zero();
        let length = (rope.position.xy() - actor_transform.translation.xy()).length();

        let mut intersecting = false;

        rapier_context.intersections_with_point(rope.position, QueryFilter::only_fixed(), |_| {
            intersecting = true;
            return false;
        });

        if
            raycast(
                (actor_transform.translation.xy() * (CHUNK_SIZE as f32)).as_ivec2(),
                (rope.position.xy() * (CHUNK_SIZE as f32) - direction * 4.0).as_ivec2(),
                &chunk_manager,
                |pixel|
                    !matches!(
                        pixel.physics_type,
                        PhysicsType::Static | PhysicsType::Rigidbody { .. }
                    )
            ).is_some() ||
            !intersecting
        {
            commands.entity(entity).despawn_recursive();
            actor_flags.remove(PlayerFlags::HOOKED);
            commands.entity(rope.source).remove::<ImpulseJoint>();
            continue;
        }

        let mesh = meshes.get_mut(mesh_handle.0.clone()).unwrap();

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

        if let Ok(mut end_transform) = rope_end_q.get_mut(*children.first().unwrap()) {
            end_transform.translation.y = (length * (CHUNK_SIZE as f32)) / 2.0;
            end_transform.rotation = Quat::from_rotation_z(
                -direction.to_angle() - PI / 2.0 + rope.initial_angle
            );
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
            &Velocity,
            &ActionState<PlayerActions>,
            Option<&mut ShootCooldown>,
            Option<&mut ShootBuffer>,
        ),
        With<Player>
    >,
    time: Res<Time>,
    registries: Res<Registries>,
    selected_material: Res<PlayerSelectedMaterial>,
    mut player_materials: ResMut<PlayerMaterials>,
    cursor_position: Option<ResMut<CursorPosition>>
) {
    let (entity, transform, velocity, action_state, mut shoot_cooldown, mut shoot_buffer) =
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

    let Some(cursor_position) = cursor_position else {
        return;
    };

    if can_shoot {
        if action_state.just_pressed(&PlayerActions::Shoot) || shoot_buffer.is_some() {
            if let Some(material) = player_materials.get(selected_material.0.as_str()) {
                if *material < 16.0 {
                    return;
                }

                *player_materials.entry(selected_material.0.clone()).or_insert(0.0) -= 16.0;
            }

            let size: i32 = 17;
            let sand = registries.materials.get(&selected_material.0).unwrap();
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
                        Sensor,
                        Projectile::new(0.1, 4.0).insert_on_contact().with_source(entity),
                        ObjectBundle {
                            object,
                            collider,
                            transform: TransformBundle {
                                local: Transform::from_translation(
                                    transform.translation.xy().extend(0.0)
                                ),
                                ..Default::default()
                            },
                            velocity: Velocity::linear(
                                cursor_position.direction * 1.25 + velocity.linvel / 16.0
                            ),
                            mass_properties: ColliderMassProperties::Density(16.0),
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

#[derive(Resource, Default, Deref, DerefMut)]
pub struct PlayerMaterials(IndexMap<String, f32>);

#[derive(Resource, Reflect, Deref, DerefMut)]
pub struct PlayerSelectedMaterial(pub String);

impl Default for PlayerSelectedMaterial {
    fn default() -> Self {
        Self("healium".to_string())
    }
}

#[derive(Default, Resource, Deref, DerefMut)]
pub struct PlayerTrackingParticles(Vec<(String, Entity)>);

#[derive(Component)]
pub struct CollectSFX;

#[derive(Component)]
pub struct InventoryParameters {
    pub max_storage: f32,
}

pub fn player_collect_sand(
    mut commands: Commands,
    player_q: Query<
        (Entity, &Transform, &ActionState<PlayerActions>, &InventoryParameters),
        With<Player>
    >,
    mut chunk_manager: ResMut<ChunkManager>,
    mut tracked_particles: ResMut<PlayerTrackingParticles>,
    mut dirty_rects: ResMut<DirtyRects>,
    mut player_materials: ResMut<PlayerMaterials>,
    registries: Res<Registries>,
    particle_q: Query<&Particle>,
    audio_assets: Res<AudioAssetCollection>,
    collect_q: Query<(), With<CollectSFX>>
) {
    let (entity, transform, action_state, inventory) = player_q.single();

    tracked_particles.retain_mut(|(id, entity)| {
        if !particle_q.contains(*entity) {
            let entry = player_materials.entry(id.clone()).or_insert(0.0);

            *entry = (*entry + 1.0 / 16.0).clamp(0.0, inventory.max_storage);

            if collect_q.iter().len() < 8 {
                match registries.materials.get(id).unwrap().physics_type {
                    PhysicsType::Powder => {
                        commands.spawn((
                            CollectSFX,
                            AudioBundle {
                                source: fastrand
                                    ::choice(audio_assets.powder.iter())
                                    .unwrap()
                                    .1.clone(),
                                settings: PlaybackSettings::DESPAWN.with_speed(
                                    fastrand::f32() * 0.5 + 1.0
                                ),
                            },
                        ));
                    }
                    PhysicsType::Liquid(_) => {
                        commands.spawn((
                            CollectSFX,
                            AudioBundle {
                                source: fastrand
                                    ::choice(audio_assets.liquid.iter())
                                    .unwrap()
                                    .1.clone(),
                                settings: PlaybackSettings::DESPAWN.with_speed(
                                    fastrand::f32() * 0.5 + 1.0
                                ),
                            },
                        ));
                    }
                    _ => {}
                }
            }

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
                        PhysicsType::Powder | PhysicsType::Liquid(..) | PhysicsType::Gas(..)
                    )
                {
                    if let Some(material) = player_materials.get_mut(&pixel.material.id) {
                        if *material >= inventory.max_storage {
                            continue;
                        }
                    }

                    let pixel = mem::take(pixel);

                    tracked_particles.push((
                        pixel.material.id.clone(),
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
    mut player_q: Query<
        (&PlayerFlags, &mut Transform, &Velocity, &mut StorredRotation),
        With<Player>
    >,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>
) {
    let (flags, mut transform, velocity, mut rotation) = player_q.single_mut();
    let (camera, camera_global_transform) = camera_q.single();

    if !flags.contains(PlayerFlags::ATTACKING) && !flags.contains(PlayerFlags::DASHING) {
        if
            window_q
                .get_single()
                .ok()
                .filter(|window| window.cursor_position().is_some())
                .map_or(velocity.linvel.x.is_sign_negative(), |window| {
                    let point = camera
                        .viewport_to_world(
                            camera_global_transform,
                            window.cursor_position().unwrap()
                        )
                        .map(|ray| ray.origin.truncate())
                        .unwrap();

                    (point.yx() - transform.translation.yx()).to_angle().is_sign_negative()
                })
        {
            rotation.0 = Quat::from_rotation_y(-(180f32).to_radians());
        } else {
            rotation.0 = Quat::IDENTITY;
        }
    }

    transform.rotation = rotation.0;
}

#[derive(Resource)]
pub struct CursorPosition {
    direction: Vec2,
    world_position: Vec2,
    angle: f32,
}

pub fn store_camera_position(
    mut commands: Commands,
    player_q: Query<&Transform, With<Player>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>
) {
    let player_transform = player_q.single();
    let (camera, camera_transform) = camera_q.single();

    match
        window_q
            .get_single()
            .ok()
            .map(|window| window.cursor_position())
            .filter(|position| position.is_some())
            .map(|cursor_position| {
                let world_position = camera
                    .viewport_to_world(camera_transform, cursor_position.unwrap())
                    .map(|ray| ray.origin.truncate())
                    .unwrap();

                let direction = (
                    world_position - player_transform.translation.xy()
                ).normalize_or_zero();
                let angle = direction.to_angle();

                CursorPosition {
                    direction,
                    world_position,
                    angle,
                }
            })
    {
        Some(result) => commands.insert_resource(result),
        None => commands.remove_resource::<CursorPosition>(),
    };
}
