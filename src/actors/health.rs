use bevy::{ prelude::*, sprite::MaterialMesh2dBundle };
use bevy_rapier2d::dynamics::Velocity;

use crate::constants::CHUNK_SIZE;

use super::{ effects::{DamageFlash, Death}, enemy::Enemy, player:: Player  };

#[derive(Reflect, Component, Clone)]
pub struct Health {
    pub current: f32,
    pub total: f32,
}

#[derive(Reflect, Component)]
pub struct HealthBar {
    subject: Entity,
}

#[derive(Component)]
pub struct HealthBarOverlay {
    pub offset: Vec2,
    pub width: f32,
}

#[derive(Component)]
pub struct HealthBarFill;

#[derive(Component)]
pub struct HealthBarBackground;

pub(crate) fn create_health_bars(
    mut commands: Commands,
    added_health_q: Query<(Entity, &HealthBarOverlay), Added<HealthBarOverlay>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>
) {
    let health_color = Color::YELLOW;
    let missing_color = Color::BLACK;

    for (entity, overlay_settings) in added_health_q.iter() {
        commands
            .spawn((
                HealthBar {
                    subject: entity,
                },
                SpatialBundle::from_transform(
                    Transform::from_scale(Vec3::splat(1.0 / (CHUNK_SIZE as f32)))
                ),
            ))
            .with_children(|parent| {
                // Current health
                parent.spawn((
                    HealthBarFill,
                    MaterialMesh2dBundle {
                        mesh: meshes
                            .add(Rectangle::from_size(Vec2::new(overlay_settings.width, 3.0)))
                            .into(),
                        material: materials.add(ColorMaterial::from(health_color)),
                        transform: Transform::from_translation(
                            overlay_settings.offset.extend(101.0)
                        ),
                        ..Default::default()
                    },
                ));
                // Missing health
                parent.spawn((
                    MaterialMesh2dBundle {
                        mesh: meshes
                            .add(Rectangle::from_size(Vec2::new(overlay_settings.width, 3.0)))
                            .into(),
                        material: materials.add(ColorMaterial::from(missing_color)),
                        transform: Transform::from_translation(
                            overlay_settings.offset.extend(100.0)
                        ),
                        ..Default::default()
                    },
                ));
            });
    }
}

pub fn update_health_bars(
    mut health_bar_fill_q: Query<(&Parent, &mut Transform), With<HealthBarFill>>,
    health_bar_q: Query<&HealthBar>,
    actor_q: Query<(&Health, &HealthBarOverlay), Changed<Health>>
) {
    for (parent, mut transform) in health_bar_fill_q.iter_mut() {
        let bar = health_bar_q.get(parent.get()).unwrap();
        let Ok((health, overlay_settings)) = actor_q.get(bar.subject) else {
            continue
        };

        let percent = (health.current as f32).max(0.0) / (health.total as f32);

        transform.translation.x = ((percent - 1.0) * overlay_settings.width) / 2.0;
        transform.scale.x = percent;
    }
}

pub fn update_health_bar_translation(
    mut commands: Commands,
    mut health_bar_q: Query<(Entity, &HealthBar, &mut Transform), With<HealthBar>>,
    parent_q: Query<&Transform, Without<HealthBar>>
) {
    for (entity, bar, mut transform) in health_bar_q.iter_mut() {
        if let Ok(transform_parent) = parent_q.get(bar.subject.clone()) {
            transform.translation = transform_parent.translation;
        }
        else {
            commands.entity(entity).despawn_recursive();
        }
    }
}

#[derive(Event)]
pub struct DamageEvent {
    pub target: Entity,
    pub value: f32,
    pub knockback: Vec2,
}

pub fn process_damage_events(
    mut commands: Commands,
    mut damage_ev: EventReader<DamageEvent>,
    mut player_q: Query<(&mut Health, &mut Velocity), (With<Player>, Without<Enemy>)>,
    mut enemy_q: Query<(&mut Health, &mut Velocity, Option<&Death>), With<Enemy>>
) {
    for ev in damage_ev.read() {
        if let Ok((mut health, mut velocity)) = player_q.get_mut(ev.target) {
        } else if let Ok((mut health, mut velocity, death)) = enemy_q.get_mut(ev.target) {
            health.current -= ev.value;
            velocity.linvel += ev.knockback;

            if health.current > 0.25 {
                commands.entity(ev.target).insert(DamageFlash::default());
            }
            else if death.is_none() {
                commands.entity(ev.target).insert(Death::default());
            }
            
        }
    }
}

