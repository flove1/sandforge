use std::time::Duration;

use benimator::FrameRate;
use bevy::{
    audio::PlaybackMode,
    prelude::*,
    render::{ extract_resource::{ ExtractResource, ExtractResourcePlugin }, view::RenderLayers },
};
use bevy_rapier2d::{ dynamics::Velocity, geometry::{ Collider, Sensor }, plugin::RapierContext };
use bevy_tween::{
    interpolation::EaseFunction,
    span_tween::{ SpanTweenBundle, SpanTweenerBundle },
    tween::ComponentTween,
};
use itertools::Itertools;

use crate::{
    actors::{
        actor::AttackParameters,
        enemy::Enemy,
        health::{ Health, KnockbackResistance },
        player::{ InventoryParameters, Player },
    },
    animation::{ Animation, AnimationState },
    assets::{ AudioAssetCollection, LayoutAssetCollection, SpriteAssetCollection },
    camera::BACKGROUND_RENDER_LAYER,
    constants::{ CHUNK_SIZE, DECORATION_Z },
    despawn_component,
    interpolator::{ InterpolateBackgroundColor, InterpolateSize },
    registries::Registries,
    remove_respurce,
    simulation::{
        chunk_groups::build_chunk_group_with_texture_access,
        chunk_manager::{ update_loaded_chunks, ChunkManager },
        dirty_rect::DirtyRects,
        materials::PhysicsType,
        pixel::Pixel,
        reset_world,
    },
    state::GameState,
};

use self::{
    chunk::{
        populate_chunk,
        process_chunk_generation_events,
        process_chunk_generation_tasks,
        push_events_to_queue,
        AwaitingNearbyChunks,
        GenerationEvent,
        GenerationQueue,
        GenerationTask,
    },
    level::Level,
    noise::{ Noise, Seed },
    poisson::EnemyPositions,
};

pub mod chunk;
pub mod level;
pub mod noise;
pub mod poisson;

pub struct GenerationPlugin;

#[derive(Component)]
pub struct Exit;

pub fn add_exit(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut images: ResMut<Assets<Image>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut dirty_rects: ResMut<DirtyRects>,
    sprites: Res<SpriteAssetCollection>,
    level_data: Res<LevelData>,
    registries: Res<Registries>
) {
    let mut chunk_group = build_chunk_group_with_texture_access(
        &mut chunk_manager,
        IVec2::ZERO,
        &mut images
    ).unwrap();

    let radius = 32;

    for x in -radius..=radius {
        for y in -radius..=radius {
            let position = IVec2::new(x, y);

            if position.length_squared() > radius.pow(2) {
                continue;
            }

            let Some(pixel) = chunk_group.get(position) else {
                continue;
            };

            if pixel.physics_type == PhysicsType::Air {
                continue;
            }

            if position.length_squared() >= (radius - 4).pow(2) {
                chunk_group
                    .set(position, Pixel::from(registries.materials.get("stone").unwrap()))
                    .expect("ok");
            } else {
                chunk_group.set(position, Pixel::default()).expect("ok");
            }

            dirty_rects.request_update(position);
            dirty_rects.request_render(position);
            dirty_rects.collider.insert(position.div_euclid(IVec2::splat(CHUNK_SIZE)));
        }
    }

    for x in -radius / 2..=radius / 2 {
        for y in -radius / 2..=radius / 2 {
            let position = IVec2::new(x, y);

            if position.length_squared() > (radius / 2).pow(2) {
                continue;
            }

            chunk_group.background_set(position, [0; 4]).expect("ok");
        }
    }

    commands.spawn((
        Name::new("Exit"),
        Exit,
        SpriteSheetBundle {
            texture: sprites.portal.clone(),
            atlas: TextureAtlas {
                layout: texture_atlas_layouts.add(
                    TextureAtlasLayout::from_grid(Vec2::new(48.0, 48.0), 8, 3, None, None)
                ),
                index: 0,
            },
            transform: Transform {
                translation: Vec3::new(0.0, 0.0 / (CHUNK_SIZE as f32), DECORATION_Z),
                scale: Vec2::splat(1.0 / (CHUNK_SIZE as f32)).extend(1.0),
                ..Default::default()
            },
            ..Default::default()
        },
        AnimationState::default(),
        Animation(benimator::Animation::from_indices(0..=15, FrameRate::from_fps(8.0)).repeat()),
        Sensor,
        Collider::ball(0.25),
        RenderLayers::layer(BACKGROUND_RENDER_LAYER),
    ));

    // let element = registries.materials.get("wood").unwrap();
    // for x in -16..=16 {
    //     for y in -8..=0 {
    //         let position = IVec2::new(x, y);

    //         chunk_group.set(position, element.into()).expect("ok");
    //     }
    // }

    (-1..=1).cartesian_product(-1..=1).for_each(|(x, y)| {
        if let Some(chunk) = chunk_manager.get_chunk_data(&IVec2::new(x, y)) {
            chunk.update_textures(&mut images, level_data.0.lighting);
        }
    });
}

pub fn remove_exit(mut commands: Commands, exit_q: Query<Entity, With<Exit>>) {
    if !exit_q.is_empty() {
        commands.entity(exit_q.single()).despawn_recursive();
    }
}

#[derive(Component)]
pub struct Open;

pub fn update_portal_sprite(
    mut exit_q: Query<(&mut AnimationState, &mut Animation), Changed<Open>>
) {
    if let Ok((mut state, mut animation)) = exit_q.get_single_mut() {
        *state = AnimationState::default();
        *animation = Animation(
            benimator::Animation::from_indices(16..=20, FrameRate::from_fps(8.0)).repeat()
        );
    }
}

pub fn move_actors_to_exit(
    mut commands: Commands,
    enemy_q: Query<Entity, With<Enemy>>,
    mut player_q: Query<(Entity, &Transform, &mut Velocity), With<Player>>,
    exit_q: Query<(Entity, &Transform, Option<&Open>), With<Exit>>,
    mut game_state: ResMut<NextState<GameState>>,
    rapier_context: Res<RapierContext>
) {
    let Ok((entity, transform, open)) = exit_q.get_single() else {
        return;
    };

    if !enemy_q.is_empty() {
        return;
    } else if open.is_none() {
        commands.entity(entity).insert(Open);
    }

    let (player_entity, player_transform, mut player_velocity) = player_q.single_mut();
    if player_transform.translation.xy().distance(transform.translation.xy()) < 2.0 {
        let delta = transform.translation.xy() - player_transform.translation.xy();
        if delta.length() > 8.0 / (CHUNK_SIZE as f32) {
            player_velocity.linvel +=
                ((delta.signum() * delta.length_recip()) / (CHUNK_SIZE as f32)) * 4.0;
        }
    }

    if rapier_context.intersection_pair(entity, player_entity).is_some() {
        game_state.set(GameState::LevelInitialization);
    }
}

#[derive(Default, Resource, Deref, DerefMut)]
pub struct LevelCounter(pub u32);

#[derive(Component)]
struct UiSplashScreen;

#[derive(Component)]
pub struct LevelUpMenu;

#[derive(Component)]
pub enum LevelUpButton {
    Health,
    Damage,
    Inventory,
    KnockbackResistance,
}

#[derive(Component)]
pub struct LoadingIcon;

#[derive(Component)]
pub struct LoadingText;

fn splash_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    counter: Res<LevelCounter>
) {
    commands
        .spawn((
            UiSplashScreen,
            NodeBundle {
                style: Style {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: BackgroundColor(Color::BLACK),
                ..default()
            },
        ))
        .with_children(|parent| {
            if counter.0 != 0 {
                parent
                    .spawn((
                        LevelUpMenu,
                        NodeBundle {
                            style: Style {
                                column_gap: Val::Px(50.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    ))
                    .with_children(|parent| {
                        let image = asset_server.load("ui/level_up.png");
                        let slicer = TextureSlicer {
                            border: BorderRect::square(17.0),
                            center_scale_mode: SliceScaleMode::Stretch,
                            sides_scale_mode: SliceScaleMode::Stretch,
                            max_corner_scale: 1.0,
                        };

                        let buttons = [
                            (LevelUpButton::Health, "+10% HP", "ui/health_up.png"),
                            (LevelUpButton::Damage, "+1 DMG", "ui/attack_up.png"),
                            (LevelUpButton::Inventory, "+5 INV", "ui/inventory_up.png"),
                            (LevelUpButton::KnockbackResistance, "x1.5 KBR", "ui/defense_up.png"),
                        ];
                        for (button_type, text, path) in buttons {
                            parent
                                .spawn((
                                    NodeBundle {
                                        style: Style {
                                            max_width: Val::Px(140.0),
                                            flex_direction: FlexDirection::Column,
                                            align_items: AlignItems::Center,
                                            row_gap: Val::Px(20.0),
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    },
                                ))
                                .with_children(|parent| {
                                    parent
                                        .spawn((
                                            button_type,
                                            EaseFunction::QuadraticInOut,
                                            SpanTweenerBundle::new(Duration::from_millis(259)),
                                            SpanTweenBundle::new(..Duration::from_millis(250)),
                                            ButtonBundle {
                                                style: Style {
                                                    width: Val::Px(120.0),
                                                    height: Val::Px(120.0),
                                                    max_width: Val::Px(140.0),
                                                    justify_content: JustifyContent::Center,
                                                    align_items: AlignItems::Center,
                                                    ..default()
                                                },
                                                background_color: Color::GRAY.into(),
                                                image: image.clone().into(),
                                                ..default()
                                            },
                                            ImageScaleMode::Sliced(slicer.clone()),
                                        ))
                                        .with_children(|parent| {
                                            parent.spawn((
                                                EaseFunction::QuadraticInOut,
                                                SpanTweenerBundle::new(Duration::from_millis(259)),
                                                SpanTweenBundle::new(..Duration::from_millis(250)),
                                                ImageBundle {
                                                    style: Style {
                                                        width: Val::Percent(100.0),
                                                        height: Val::Percent(100.0),
                                                        ..default()
                                                    },
                                                    background_color: Color::GRAY.into(),
                                                    image: asset_server.load(path).into(),
                                                    ..default()
                                                },
                                            ));
                                        });

                                    parent.spawn(
                                        TextBundle::from_section(text, TextStyle {
                                            font_size: 28.0,
                                            ..Default::default()
                                        }).with_text_justify(JustifyText::Center)
                                    );
                                });
                        }
                    });
            }

            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(200.0),
                        height: Val::Auto,
                        position_type: PositionType::Absolute,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        right: Val::Px(50.0),
                        bottom: Val::Px(50.0),
                        ..default()
                    },
                    ..Default::default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        LoadingIcon,
                        AnimationState::default(),
                        Animation(
                            benimator::Animation
                                ::from_indices(0..=5, FrameRate::from_fps(8.0))
                                .repeat()
                        ),
                        AtlasImageBundle {
                            image: asset_server.load("ui/loading.png").into(),
                            style: Style {
                                width: Val::Percent(100.0),
                                margin: UiRect::bottom(Val::Px(-50.0)),
                                ..Default::default()
                            },
                            texture_atlas: TextureAtlas {
                                layout: texture_atlas_layouts.add(
                                    TextureAtlasLayout::from_grid(
                                        Vec2::new(48.0, 48.0),
                                        6,
                                        1,
                                        None,
                                        None
                                    )
                                ),
                                index: 0,
                            },
                            ..default()
                        },
                    ));

                    parent.spawn((
                        LoadingText,
                        TextBundle::from_section("Loading...", TextStyle {
                            font_size: 16.0,
                            ..Default::default()
                        }),
                    ));
                });
        });
}

fn level_up_button(
    mut commands: Commands,
    mut player_q: Query<
        (
            &mut Health,
            &mut AttackParameters,
            &mut InventoryParameters,
            &mut KnockbackResistance,
        ),
        With<Player>
    >,
    button_q: Query<
        (Entity, &Style, &BackgroundColor, &LevelUpButton, &Interaction, &Children),
        (With<Button>, Changed<Interaction>)
    >,
    image_q: Query<(Entity, &BackgroundColor), Without<Button>>,
    menu_q: Query<Entity, With<LevelUpMenu>>,
    audio_assets: Res<AudioAssetCollection>
) {
    let (mut health, mut attack, mut inventory, mut knockback) =
        player_q.single_mut();
    let Ok(menu_entity) = menu_q.get_single() else {
        return;
    };

    for (entity, style, color, button, interaction, children) in button_q.iter() {
        let (image_entity, image_color) = image_q.get(children[0]).unwrap();
        let size = match style.width {
            Val::Px(size) => size,
            _ => panic!("Expected fixed size"),
        };

        match *interaction {
            Interaction::Pressed => {
                commands.spawn(AudioBundle {
                    source: audio_assets.perk.clone(),
                    settings: PlaybackSettings::DESPAWN,
                });

                match button {
                    LevelUpButton::Health => {
                        let change = health.total * 0.1;
                        health.current += change;
                        health.total += change;
                    }
                    LevelUpButton::Damage => {
                        attack.value += 1.0;
                    }
                    LevelUpButton::Inventory => {
                        inventory.max_storage += 5.0;
                    }
                    LevelUpButton::KnockbackResistance => {
                        knockback.0 = knockback.0 * 1.5;
                    }
                }

                commands.entity(menu_entity).despawn_recursive();
                return;
            }
            Interaction::Hovered => {
                commands
                    .entity(image_entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                    .insert(
                        ComponentTween::new(InterpolateBackgroundColor {
                            start: image_color.0,
                            end: Color::WHITE,
                        })
                    );

                commands
                    .entity(entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                    .insert(AudioBundle {
                        source: audio_assets.button_select.clone(),
                        settings: PlaybackSettings::REMOVE,
                    })
                    .insert(
                        ComponentTween::new(InterpolateBackgroundColor {
                            start: color.0,
                            end: Color::WHITE,
                        })
                    )
                    .insert(
                        ComponentTween::new(InterpolateSize {
                            start: Vec2::splat(size),
                            end: Vec2::splat(140.0),
                        })
                    );
            }
            Interaction::None => {
                commands
                    .entity(image_entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                    .insert(
                        ComponentTween::new(InterpolateBackgroundColor {
                            start: image_color.0,
                            end: Color::GRAY,
                        })
                    );

                commands
                    .entity(entity)
                    .insert(SpanTweenerBundle::new(Duration::from_millis(250)))
                    .insert(
                        ComponentTween::new(InterpolateBackgroundColor {
                            start: color.0,
                            end: Color::GRAY,
                        })
                    )
                    .insert(
                        ComponentTween::new(InterpolateSize {
                            start: Vec2::splat(size),
                            end: Vec2::splat(120.0),
                        })
                    );
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct SeedOffset(u32);

#[derive(Resource)]
pub struct LevelData(pub Level, pub Handle<Image>);

pub fn next_level(
    mut commands: Commands,
    mut counter: ResMut<LevelCounter>,
    images: Res<Assets<Image>>,
    registries: Res<Registries>,
    layouts: ResMut<LayoutAssetCollection>,
    seed: Res<Seed>,
    seed_offset: Res<SeedOffset>
) {
    counter.0 += 1;

    let level = registries.levels
        .get((counter.0 - 1).rem_euclid(registries.levels.len() as u32) as usize)
        .unwrap();

    let level_texture = layouts.folder.get(&level.texture_path).unwrap();
    let size = images.get(level_texture).unwrap().size().as_ivec2() / CHUNK_SIZE;
    let seed = seed.0 + counter.0 + seed_offset.0;

    let noise = Noise::from_seed(seed, level.noise_type);
    let enemies = EnemyPositions::new(seed, size, level.enemies.clone());

    commands.insert_resource(AwaitingNearbyChunks::default());
    commands.insert_resource(LevelData(level.clone(), level_texture.clone()));
    commands.insert_resource(noise);
    commands.insert_resource(enemies);
    commands.insert_resource(GenerationQueue::default());
    commands.remove_resource::<FinishedGeneration>();
}

fn load_level_chunks(
    level_data: Res<LevelData>,
    images: Res<Assets<Image>>,
    mut ev_chunkgen: EventWriter<GenerationEvent>
) {
    let texture = images.get(level_data.1.clone()).unwrap();
    let texture_size = texture.size().as_ivec2() / CHUNK_SIZE;

    for x in -texture_size.x / 2..texture_size.x / 2 {
        for y in -texture_size.y / 2..texture_size.y / 2 {
            ev_chunkgen.send(GenerationEvent(IVec2::new(x, y)));
        }
    }
}

#[derive(Resource)]
pub struct ChoseLevelUp;

#[derive(Resource)]
pub struct FinishedGeneration;

fn check_generation_tasks(
    mut commands: Commands,
    mut state: ResMut<NextState<GameState>>,
    mut counter: ResMut<LevelCounter>,
    queue: Res<GenerationQueue>,
    tasks_q: Query<&GenerationTask>,
    enemy_q: Query<&Enemy>,
    icon_q: Query<Entity, With<LoadingIcon>>,
    mut seed_offset: ResMut<SeedOffset>,
    mut text_q: Query<&mut Text, With<LoadingText>>
) {
    if tasks_q.is_empty() && queue.is_empty() {
        if enemy_q.is_empty() {
            counter.0 -= 1;
            seed_offset.0 += 1;
            state.set(GameState::LevelInitialization);
            return;
        }

        commands.insert_resource(FinishedGeneration);
        text_q.single_mut().sections[0].value = "Level is ready...".to_string();

        if let Ok(entity) = icon_q.get_single() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn switch_to_game(
    mut state: ResMut<NextState<GameState>>,
    menu_q: Query<Entity, With<LevelUpMenu>>
) {
    if menu_q.is_empty() {
        state.set(GameState::Game);
    }
}

fn reset_generation(mut commands: Commands) {
    commands.insert_resource(Seed::new());
    commands.insert_resource(LevelCounter::default());
    commands.insert_resource(SeedOffset::default());
}

#[derive(Component)]
pub struct Ambient;

#[derive(Default, Resource, ExtractResource, Clone)]
pub struct ShadowColor(pub Color);

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Seed>()
            .init_resource::<LevelCounter>()
            .init_resource::<ShadowColor>()
            .add_plugins(ExtractResourcePlugin::<ShadowColor>::default())
            .add_event::<GenerationEvent>()
            .add_systems(OnEnter(GameState::Setup), reset_generation)
            .add_systems(OnEnter(GameState::Menu), despawn_component::<Ambient>)
            .add_systems(OnEnter(GameState::LevelInitialization), despawn_component::<Ambient>)
            .add_systems(
                OnTransition { from: GameState::Game, to: GameState::LevelInitialization },
                splash_setup
            )
            .add_systems(
                OnTransition { from: GameState::Setup, to: GameState::LevelInitialization },
                splash_setup
            )
            .add_systems(
                OnEnter(GameState::Game),
                move |
                    mut commands: Commands,
                    level: Res<LevelData>,
                    asset_server: Res<AssetServer>
                | {
                    commands.insert_resource(
                        ShadowColor(
                            Color::rgb_from_array([
                                level.0.shadow[0],
                                level.0.shadow[1],
                                level.0.shadow[2],
                            ])
                        )
                    );

                    commands.spawn((
                        Ambient,
                        AudioBundle {
                            source: asset_server.load(level.0.ambient.clone()),
                            settings: PlaybackSettings {
                                mode: PlaybackMode::Loop,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    ));

                    commands.insert_resource(
                        ClearColor(
                            Color::rgb(
                                level.0.background[0],
                                level.0.background[1],
                                level.0.background[2]
                            )
                        )
                    );
                }
            )
            .add_systems(
                OnEnter(GameState::LevelInitialization),
                (
                    remove_respurce::<FinishedGeneration>,
                    remove_respurce::<ChoseLevelUp>,
                    clear_generation_events,
                    reset_world,
                    next_level,
                    remove_exit,
                    load_level_chunks,
                    push_events_to_queue,
                ).chain()
            )
            .add_systems(OnTransition { from: GameState::Splash, to: GameState::Game }, (
                despawn_component::<UiSplashScreen>,
            ))
            .add_systems(
                Update,
                level_up_button.run_if(
                    in_state(GameState::Splash).and_then(not(resource_exists::<ChoseLevelUp>))
                )
            )
            .add_systems(
                Update,
                (process_chunk_generation_events, process_chunk_generation_tasks, populate_chunk)
                    .chain()
                    .run_if(in_state(GameState::Splash))
            )
            .add_systems(
                PostUpdate,
                (
                    check_generation_tasks.run_if(not(resource_exists::<FinishedGeneration>)),
                    switch_to_game.run_if(resource_exists::<FinishedGeneration>),
                )
                    .chain()
                    .run_if(in_state(GameState::Splash))
            )
            .add_systems(
                PreUpdate,
                (
                    push_events_to_queue,
                    process_chunk_generation_events,
                    process_chunk_generation_tasks,
                    populate_chunk,
                )
                    .chain()
                    .after(update_loaded_chunks)
                    .run_if(in_state(GameState::Game))
            )
            .add_systems(OnExit(GameState::Splash), add_exit)
            .add_systems(
                PreUpdate,
                (move_actors_to_exit, update_portal_sprite)
                    .chain()
                    .run_if(in_state(GameState::Game))
            );
    }
}

fn clear_generation_events(mut events: ResMut<Events<GenerationEvent>>) {
    events.clear();
}
