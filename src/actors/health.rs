use bevy::{ audio::{ PlaybackMode, Volume }, prelude::* };
use bevy_rapier2d::dynamics::Velocity;

use crate::{ assets::AudioAssetCollection, state::GameState };

use super::{ effects::{ DamageFlash, Death }, enemy::Enemy, player::Player };

#[derive(Reflect, Component, Clone)]
pub struct Health {
    pub current: f32,
    pub total: f32,
}

#[derive(Component)]
pub struct KnockbackResistance(pub f32);

#[derive(Event)]
pub struct DamageEvent {
    pub target: Entity,
    pub value: f32,
    pub knockback: Vec2,
    pub ignore_iframes: bool,
    pub play_sound: bool,
}

#[derive(Component, Deref, DerefMut, Clone)]
pub struct IFrames(pub Timer);

pub fn tick_iframes(
    mut commands: Commands,
    mut iframe_q: Query<(Entity, &mut IFrames)>,
    time: Res<Time>
) {
    for (entity, mut iframe) in iframe_q.iter_mut() {
        iframe.tick(time.delta());

        if iframe.finished() {
            commands.entity(entity).remove::<IFrames>();
        }
    }
}

pub fn process_damage_events(
    mut commands: Commands,
    mut damage_ev: EventReader<DamageEvent>,
    mut player_q: Query<
        (&Transform, &mut Health, &mut Velocity, Option<&IFrames>, &KnockbackResistance),
        (With<Player>, Without<Enemy>)
    >,
    mut enemy_q: Query<
        (&Transform, &mut Health, &mut Velocity, Option<&Death>, Option<&IFrames>),
        (With<Enemy>, Without<Death>)
    >,
    mut state: ResMut<NextState<GameState>>,
    audio_assets: Res<AudioAssetCollection>
) {
    let mut added_iframes = vec![];

    for ev in damage_ev.read() {
        if
            let Ok((transform, mut health, mut velocity, iframes, knockback_resistance)) =
                player_q.get_mut(ev.target)
        {
            if (iframes.is_some() || added_iframes.contains(&ev.target)) && !ev.ignore_iframes {
                continue;
            }

            if ev.play_sound {
                commands.spawn((
                    TransformBundle::from_transform(transform.clone()),
                    AudioBundle {
                        source: audio_assets.hit.clone(),
                        settings: PlaybackSettings {
                            mode: PlaybackMode::Despawn,
                            spatial: true,
                            volume: Volume::new(0.5),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ));
            }

            health.current -= ev.value;
            velocity.linvel += ev.knockback - ev.knockback * knockback_resistance.0;

            if health.current > 0.0 {
                commands.entity(ev.target).insert(DamageFlash::default());
            } else {
                state.set(GameState::GameOver);
            }

            if !ev.ignore_iframes {
                added_iframes.push(ev.target);
                commands
                    .entity(ev.target)
                    .insert(IFrames(Timer::from_seconds(0.5, TimerMode::Once)));
            }
        } else if
            let Ok((transform, mut health, mut velocity, death, iframes)) = enemy_q.get_mut(
                ev.target
            )
        {
            if (iframes.is_some() || added_iframes.contains(&ev.target)) && !ev.ignore_iframes {
                continue;
            }

            if ev.play_sound {
                commands.spawn((
                    TransformBundle::from_transform(transform.clone()),
                    AudioBundle {
                        source: audio_assets.hit.clone(),
                        settings: PlaybackSettings {
                            mode: PlaybackMode::Despawn,
                            spatial: true,
                            volume: Volume::new(0.5),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ));
            }

            health.current -= ev.value;
            velocity.linvel += ev.knockback;

            if health.current > 0.0 {
                commands.entity(ev.target).insert(DamageFlash::default());
            } else if death.is_none() {
                commands.entity(ev.target).insert(Death::default());
            }

            if !ev.ignore_iframes {
                added_iframes.push(ev.target);
                commands
                    .entity(ev.target)
                    .insert(IFrames(Timer::from_seconds(0.5, TimerMode::Once)));
            }
        }
    }
}
