use std::time::Duration;

use benimator::FrameRate;
use bevy::prelude::*;

use crate::{animation::{Animation, AnimationState, DespawnOnFinish}, assets::SpriteSheets};

#[derive(Component)]
pub struct DamageFlash{
    start_timer: Timer,
    exit_timer: Timer
}

impl Default for DamageFlash {
    fn default() -> Self {
        Self {
            start_timer: Timer::new(Duration::from_millis(100), TimerMode::Once),
            exit_timer: Timer::new(Duration::from_millis(50), TimerMode::Once)
        }
    }
}

pub fn damage_flash(
    mut commands: Commands,
    mut flashing_query: Query<(&mut DamageFlash, Entity, &mut Sprite)>,
    time: Res<Time>
) {
    for (mut effect, entity, mut sprite) in flashing_query.iter_mut() {
        if !effect.start_timer.finished() {
            sprite.color = Color::rgba(255.0, 255.0, 255.0, 1.0);
            effect.start_timer.tick(time.delta());
        }
        else if !effect.exit_timer.finished() {
            sprite.color = Color::RED;
            effect.exit_timer.tick(time.delta());
        }
        else {
            sprite.color = Color::default();
            commands.entity(entity).remove::<DamageFlash>();
        }
    }
}

#[derive(Component)]
pub struct Death{
    timer: Timer,
}

impl Default for Death {
    fn default() -> Self {
        Self {
            timer: Timer::new(Duration::from_millis(500), TimerMode::Once),
        }
    }
}

pub fn death(
    mut commands: Commands,
    mut effect_q: Query<(&mut Death, Entity, &mut Sprite, &Transform)>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    time: Res<Time>,
    sprites: Res<SpriteSheets>,
) {
    for (mut effect, entity, mut sprite, transform) in effect_q.iter_mut() {
        if !effect.timer.finished() {
            effect.timer.tick(time.delta());
            let percentage = 1.0 - effect.timer.elapsed().as_secs_f32() / effect.timer.duration().as_secs_f32() / 2.0 + 0.5;

            sprite.color = Color::rgb_from_array([percentage; 3]);
        }
        else {
            commands.entity(entity).despawn_recursive();

            commands.spawn((
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
                    transform: *transform,
                    ..Default::default()
                },
                AnimationState::default(),
                Animation(
                    benimator::Animation
                        ::from_indices(132..=143, FrameRate::from_fps(12.0))
                        .once()
                ),
                DespawnOnFinish,
            ));
        }
    }
}
