use benimator::FrameRate;
use bevy::{ prelude::*, utils::HashMap };
use bevy_rapier2d::{
    dynamics::{ GravityScale, Velocity },
    geometry::{ Collider, CollisionGroups, Group },
};
use seldom_state::{ prelude::{ AnyState, StateMachine }, trigger::IntoTrigger };

use crate::{
    actors::{
        actor::{ Actor, ActorBundle, ActorFlags, ActorHitboxBundle, MovementType },
        animation::{
            create_animation_end_trigger,
            create_run_trigger,
            FallAnimation,
            IdleAnimation,
            JumpAnimation,
            LandAnimation,
            MoveAnimation,
        },
        enemy::{ EnemyAI, EnemyBundle },
    },
    animation::{ Animation, AnimationState },
    assets::SpriteAssetCollection,
    constants::{ CHUNK_SIZE, ENEMY_Z },
    generation::level::Level,
    simulation::{
        colliders::{ ENEMY_MASK, HITBOX_MASK, PLAYER_MASK },
        materials::{ Material, Reaction },
        object::Projectile,
    },
};

#[derive(Resource)]
pub struct Registries {
    pub materials: HashMap<String, Material>,

    // ugly workaround since i can't clone StateMachine...
    pub enemies: HashMap<
        String,
        Box<dyn (Fn(Vec2) -> (EnemyBundle, ActorHitboxBundle)) + Sync + Send>
    >,
    pub levels: Vec<Level>,
}

impl FromWorld for Registries {
    fn from_world(world: &mut World) -> Self {
        let mut materials = HashMap::new();

        materials.insert("air".to_string(), Material::default());

        ron::de
            ::from_str::<Vec<Material>>(&std::fs::read_to_string("materials.ron").unwrap())
            .unwrap()
            .into_iter()
            .for_each(|material| {
                materials.insert(material.id.clone(), material);
            });

        ron::de
            ::from_str::<Vec<Reaction>>(&std::fs::read_to_string("reactions.ron").unwrap())
            .unwrap()
            .into_iter()
            .for_each(|reaction| {
                materials.entry(reaction.input_material_1.clone()).and_modify(|material| {
                    material.reactions
                        .get_or_insert(HashMap::default())
                        .insert(reaction.input_material_2.clone(), reaction);
                });
            });

        let sprites = world.get_resource::<SpriteAssetCollection>().cloned().unwrap();
        let mut texture_atlas_layouts = world
            .get_resource_mut::<Assets<TextureAtlasLayout>>()
            .unwrap();

        let plant_sprite = sprites.plant.clone();
        let plant_atlas = texture_atlas_layouts.add(
            TextureAtlasLayout::from_grid(Vec2::splat(64.0), 4, 6, None, None)
        );

        let mut enemies: HashMap<
            String,
            Box<dyn (Fn(Vec2) -> (EnemyBundle, ActorHitboxBundle)) + Sync + Send>
        > = HashMap::default();

        enemies.insert(
            "plant".into(),
            Box::new(move |position: Vec2| (
                EnemyBundle {
                    ai: EnemyAI::Projectiles {
                        base_material: "sand".to_string(),
                        cooldown: Timer::from_seconds(2.0, TimerMode::Repeating),
                        projectile: Projectile::new(0.1, 4.0).insert_on_contact(),
                        speed: 0.5,
                        range: 64.0,
                    },
                    name: Name::new("Plant"),
                    actor: ActorBundle {
                        actor: Actor {
                            position: position * (CHUNK_SIZE as f32),
                            size: Vec2::new(21.0, 21.0),
                            movement_type: MovementType::Floating,
                            ..Default::default()
                        },
                        collider: Collider::ball(12.0),
                        sprite: SpriteSheetBundle {
                            texture: plant_sprite.clone_weak(),
                            atlas: TextureAtlas {
                                layout: plant_atlas.clone_weak(),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation: position.extend(ENEMY_Z),
                                scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    state_machine: StateMachine::default()
                        .trans::<IdleAnimation, _>(create_run_trigger(0.25), MoveAnimation)
                        .trans::<MoveAnimation, _>(create_run_trigger(0.25).not(), IdleAnimation)
                        .on_enter::<IdleAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(0..=3, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        })
                        .on_enter::<MoveAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(4..=7, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        }),
                    ..Default::default()
                },
                ActorHitboxBundle {
                    collider: Collider::ball(6.0),
                    collision_groups: CollisionGroups::new(
                        Group::from_bits_retain(ENEMY_MASK | HITBOX_MASK),
                        Group::from_bits_retain(PLAYER_MASK)
                    ),
                    ..Default::default()
                },
            ))
        );

        let bat_sprite = sprites.bat.clone();
        let bat_atlas = texture_atlas_layouts.add(
            TextureAtlasLayout::from_grid(Vec2::splat(17.0), 6, 1, None, None)
        );

        enemies.insert(
            "bat".into(),
            Box::new(move |position: Vec2| (
                EnemyBundle {
                    name: Name::new("Bat"),
                    actor: ActorBundle {
                        actor: Actor {
                            position: position * (CHUNK_SIZE as f32),
                            size: Vec2::new(17.0, 17.0),
                            movement_type: MovementType::Floating,
                            ..Default::default()
                        },
                        collider: Collider::ball(12.0),
                        sprite: SpriteSheetBundle {
                            texture: bat_sprite.clone_weak(),
                            atlas: TextureAtlas {
                                layout: bat_atlas.clone_weak(),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation: position.extend(ENEMY_Z),
                                scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        gravity: GravityScale(0.5),
                        ..Default::default()
                    },
                    state_machine: StateMachine::default()
                        .trans::<IdleAnimation, _>(create_run_trigger(0.25), MoveAnimation)
                        .trans::<MoveAnimation, _>(create_run_trigger(0.25).not(), IdleAnimation)
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
                        .on_enter::<MoveAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(0..=5, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        }),
                    ..Default::default()
                },
                ActorHitboxBundle {
                    collider: Collider::ball(6.0),
                    collision_groups: CollisionGroups::new(
                        Group::from_bits_retain(ENEMY_MASK | HITBOX_MASK),
                        Group::from_bits_retain(PLAYER_MASK)
                    ),
                    ..Default::default()
                },
            ))
        );

        let fungus_tiny_sprite = sprites.fungus_tiny.clone();
        let fungus_tiny_atlas = texture_atlas_layouts.add(
            TextureAtlasLayout::from_grid(Vec2::new(13.0, 14.0), 12, 4, None, None)
        );

        enemies.insert(
            "fungus_tiny".into(),
            Box::new(move |position: Vec2| (
                EnemyBundle {
                    name: Name::new("fungus_tiny"),
                    actor: ActorBundle {
                        actor: Actor {
                            position: position * (CHUNK_SIZE as f32),
                            size: Vec2::new(11.0, 14.0),
                            movement_type: MovementType::Walking { speed: 2.0, jump_height: 1.0 },
                            ..Default::default()
                        },
                        collider: Collider::ball(10.0),
                        sprite: SpriteSheetBundle {
                            texture: fungus_tiny_sprite.clone_weak(),
                            atlas: TextureAtlas {
                                layout: fungus_tiny_atlas.clone_weak(),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation: position.extend(ENEMY_Z),
                                scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        gravity: GravityScale(3.0),
                        ..Default::default()
                    },
                    state_machine: StateMachine::default()
                        .trans::<IdleAnimation, _>(create_run_trigger(0.25), MoveAnimation)
                        .trans::<MoveAnimation, _>(create_run_trigger(0.25).not(), IdleAnimation)
                        .on_enter::<IdleAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(0..=11, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        })
                        .on_enter::<MoveAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(12..=17, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        }),
                    ..Default::default()
                },
                ActorHitboxBundle {
                    collider: Collider::ball(6.0),
                    collision_groups: CollisionGroups::new(
                        Group::from_bits_retain(ENEMY_MASK | HITBOX_MASK),
                        Group::from_bits_retain(PLAYER_MASK)
                    ),
                    ..Default::default()
                },
            ))
        );

        let fungus_big_sprite = sprites.fungus_big.clone();
        let fungus_big_atlas = texture_atlas_layouts.add(
            TextureAtlasLayout::from_grid(Vec2::new(34.0, 34.0), 8, 2, None, None)
        );

        enemies.insert(
            "fungus_big".into(),
            Box::new(move |position: Vec2| (
                EnemyBundle {
                    name: Name::new("fungus_big"),
                    actor: ActorBundle {
                        actor: Actor {
                            position: position * (CHUNK_SIZE as f32),
                            size: Vec2::new(24.0, 22.0),
                            movement_type: MovementType::Walking { speed: 2.0, jump_height: 0.25 },
                            ..Default::default()
                        },
                        collider: Collider::ball(10.0),
                        sprite: SpriteSheetBundle {
                            texture: fungus_big_sprite.clone_weak(),
                            atlas: TextureAtlas {
                                layout: fungus_big_atlas.clone_weak(),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation: position.extend(ENEMY_Z),
                                scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        gravity: GravityScale(3.0),
                        ..Default::default()
                    },
                    state_machine: StateMachine::default()
                        .trans::<IdleAnimation, _>(create_run_trigger(0.25), MoveAnimation)
                        .trans::<MoveAnimation, _>(create_run_trigger(0.25).not(), IdleAnimation)
                        .on_enter::<IdleAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(0..=7, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        })
                        .on_enter::<MoveAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(8..=13, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        }),
                    ..Default::default()
                },
                ActorHitboxBundle {
                    collider: Collider::ball(6.0),
                    collision_groups: CollisionGroups::new(
                        Group::from_bits_retain(ENEMY_MASK | HITBOX_MASK),
                        Group::from_bits_retain(PLAYER_MASK)
                    ),
                    ..Default::default()
                },
            ))
        );

        let rat_sprite = sprites.rat.clone();
        let rat_atlas = texture_atlas_layouts.add(
            TextureAtlasLayout::from_grid(Vec2::new(20.0, 20.0), 6, 4, None, None)
        );

        enemies.insert(
            "rat".into(),
            Box::new(move |position: Vec2| (
                EnemyBundle {
                    name: Name::new("rat"),
                    actor: ActorBundle {
                        actor: Actor {
                            position: position * (CHUNK_SIZE as f32),
                            size: Vec2::new(16.0, 6.0),
                            movement_type: MovementType::Walking { speed: 4.0, jump_height: 0.5 },
                            ..Default::default()
                        },
                        collider: Collider::ball(10.0),
                        sprite: SpriteSheetBundle {
                            texture: rat_sprite.clone_weak(),
                            atlas: TextureAtlas {
                                layout: rat_atlas.clone_weak(),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation: position.extend(ENEMY_Z),
                                scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        gravity: GravityScale(3.0),
                        ..Default::default()
                    },
                    state_machine: StateMachine::default()
                        .trans::<IdleAnimation, _>(create_run_trigger(0.25), MoveAnimation)
                        .trans::<MoveAnimation, _>(create_run_trigger(0.25).not(), IdleAnimation)
                        .on_enter::<IdleAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(0..=4, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        })
                        .on_enter::<MoveAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(6..=11, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        }),
                    ..Default::default()
                },
                ActorHitboxBundle {
                    collider: Collider::ball(6.0),
                    collision_groups: CollisionGroups::new(
                        Group::from_bits_retain(ENEMY_MASK | HITBOX_MASK),
                        Group::from_bits_retain(PLAYER_MASK)
                    ),
                    ..Default::default()
                },
            ))
        );

        let frog_sprite = sprites.frog.clone();
        let frog_atlas = texture_atlas_layouts.add(
            TextureAtlasLayout::from_grid(Vec2::new(20.0, 20.0), 9, 4, None, None)
        );

        enemies.insert(
            "frog".into(),
            Box::new(move |position: Vec2| (
                EnemyBundle {
                    name: Name::new("frog"),
                    actor: ActorBundle {
                        actor: Actor {
                            position: position * (CHUNK_SIZE as f32),
                            size: Vec2::new(8.0, 8.0),
                            movement_type: MovementType::Walking { speed: 2.0, jump_height: 2.0 },
                            ..Default::default()
                        },
                        collider: Collider::ball(10.0),
                        sprite: SpriteSheetBundle {
                            texture: frog_sprite.clone_weak(),
                            atlas: TextureAtlas {
                                layout: frog_atlas.clone_weak(),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation: position.extend(ENEMY_Z),
                                scale: Vec3::splat(1.0 / (CHUNK_SIZE as f32)),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        gravity: GravityScale(3.0),
                        ..Default::default()
                    },
                    state_machine: StateMachine::default()
                        .trans::<IdleAnimation, _>(create_run_trigger(0.25), MoveAnimation)
                        .trans::<MoveAnimation, _>(create_run_trigger(0.25).not(), IdleAnimation)
                        .trans::<AnyState, _>(
                            move |
                                In(entity): In<Entity>,
                                actor_q: Query<
                                    (&Velocity, Option<&JumpAnimation>, Option<&FallAnimation>)
                                >
                            | {
                                let (velocity, jump, fall) = actor_q.get(entity).unwrap();

                                match velocity.linvel.y > 0.25 && jump.is_none() && fall.is_none() {
                                    true => Ok(()),
                                    false => Err(()),
                                }
                            },
                            JumpAnimation
                        )
                        .trans::<JumpAnimation, _>(
                            move |In(entity): In<Entity>, actor_q: Query<&Actor>| {
                                match
                                    actor_q
                                        .get(entity)
                                        .unwrap()
                                        .flags.contains(ActorFlags::GROUNDED)
                                {
                                    true => Ok(()),
                                    false => Err(()),
                                }
                            },
                            LandAnimation
                        )
                        .trans::<JumpAnimation, _>(
                            move |In(entity): In<Entity>, velocity_q: Query<&Velocity>| {
                                match velocity_q.get(entity).unwrap().linvel.y < 0.0 {
                                    true => Ok(()),
                                    false => Err(()),
                                }
                            },
                            FallAnimation
                        )
                        .trans::<AnyState, _>(
                            move |
                                In(entity): In<Entity>,
                                actor_q: Query<(&Velocity, Option<&FallAnimation>)>
                            | {
                                let (velocity, falling_animation) = actor_q.get(entity).unwrap();

                                match falling_animation.is_none() && velocity.linvel.y < -1.0 {
                                    true => Ok(()),
                                    false => Err(()),
                                }
                            },
                            FallAnimation
                        )
                        .trans::<FallAnimation, _>(
                            move |In(entity): In<Entity>, actor_q: Query<&Actor>| {
                                match
                                    actor_q
                                        .get(entity)
                                        .unwrap()
                                        .flags.contains(ActorFlags::GROUNDED)
                                {
                                    true => Ok(()),
                                    false => Err(()),
                                }
                            },
                            LandAnimation
                        )
                        .trans::<LandAnimation, _>(create_animation_end_trigger(), IdleAnimation)
                        .on_enter::<IdleAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(0..=8, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        })
                        .on_enter::<JumpAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(9..=9, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        })
                        .on_enter::<FallAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(18..=18, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        })
                        .on_enter::<MoveAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(27..30, FrameRate::from_fps(8.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        })
                        .on_enter::<LandAnimation>(|entity| {
                            entity.insert(
                                Animation(
                                    benimator::Animation
                                        ::from_indices(27..30, FrameRate::from_fps(4.0))
                                        .repeat()
                                )
                            );
                            entity.insert(AnimationState::default());
                        }),
                    ..Default::default()
                },
                ActorHitboxBundle {
                    collider: Collider::ball(6.0),
                    collision_groups: CollisionGroups::new(
                        Group::from_bits_retain(ENEMY_MASK | HITBOX_MASK),
                        Group::from_bits_retain(PLAYER_MASK)
                    ),
                    ..Default::default()
                },
            ))
        );

        let levels = ron::de
            ::from_str::<Vec<Level>>(&std::fs::read_to_string("levels.ron").unwrap())
            .unwrap();

        Self {
            materials,
            levels,
            enemies,
        }
    }
}
