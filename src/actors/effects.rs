use std::time::Duration;

use bevy::prelude::*;

use crate::{
    constants::CHUNK_SIZE,
    gui::Score,
    registries:: Registries ,
    simulation::{
        chunk_groups::build_chunk_group,
        chunk_manager:: ChunkManager ,
        dirty_rect:: DirtyRects ,
        pixel::Pixel,
    },
};

use super::{ actor::Actor, enemy::ScopePoints };

#[derive(Component)]
pub struct DamageFlash {
    start_timer: Timer,
    exit_timer: Timer,
}

impl Default for DamageFlash {
    fn default() -> Self {
        Self {
            start_timer: Timer::new(Duration::from_millis(200), TimerMode::Once),
            exit_timer: Timer::new(Duration::from_millis(50), TimerMode::Once),
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
        } else if !effect.exit_timer.finished() {
            sprite.color = Color::RED;
            effect.exit_timer.tick(time.delta());
        } else {
            sprite.color = Color::default();
            commands.entity(entity).remove::<DamageFlash>();
        }
    }
}

#[derive(Component)]
pub struct Death {
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
    mut effect_q: Query<(&Actor, &mut Death, Entity, &mut Sprite, &ScopePoints, &Transform)>,
    mut total_score: ResMut<Score>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects: ResMut<DirtyRects>,
    time: Res<Time>,
    registries: Res<Registries>
) {
    for (actor, mut effect, entity, mut sprite, points, transform) in effect_q.iter_mut() {
        if !effect.timer.finished() {
            effect.timer.tick(time.delta());
            let percentage =
                1.0 -
                effect.timer.elapsed().as_secs_f32() / effect.timer.duration().as_secs_f32() / 2.0;

            sprite.color = Color::rgb_from_array([percentage; 3]);
        } else {
            total_score.value += points.0;
            commands.entity(entity).despawn_recursive();

            let position = (transform.translation.xy() * (CHUNK_SIZE as f32)).as_ivec2();
            let local_position = position.rem_euclid(IVec2::splat(CHUNK_SIZE));
            let chunk_position = position.div_euclid(IVec2::splat(CHUNK_SIZE));

            let Some(mut chunk_group) = build_chunk_group(&mut chunk_manager, chunk_position) else {
                continue;
            };

            let IVec2 { x: width, y: height } = actor.size.as_ivec2();

            for x in -width / 2..width / 2 {
                for y in -height / 2..height / 2 {
                    if x.pow(2) / width.pow(2) + y.pow(2) / height.pow(2) > 1 {
                        continue;
                    }

                    let Some(pixel) = chunk_group.get_mut(local_position + IVec2::new(x, y)) else {
                        continue;
                    };

                    if pixel.is_empty() {
                        *pixel = Pixel::from(registries.materials.get("enemy_death_mist").unwrap());
                        dirty_rects.request_update(position + IVec2::new(x, y));
                        dirty_rects.request_render(position + IVec2::new(x, y));
                    }
                }
            }
        }
    }
}
