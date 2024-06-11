use std::{ f32::consts::{ FRAC_PI_2, PI }, mem, time::{ SystemTime, UNIX_EPOCH } };

use bevy::{
    prelude::*,
    utils::{dbg, HashMap},
    window::PrimaryWindow,
};
use bevy_egui::{ egui::Id, EguiContexts };
use bevy_math::ivec2;
use bevy_rapier2d::prelude::*;
use itertools::Itertools;

use crate::{
    actors::{ enemy::Enemy, health::DamageEvent },
    camera::TrackingCamera,
    constants::{ CHUNK_SIZE, PARTICLE_Z },
    gui::{ Cell, Inventory },
};

use super::{
    chunk::ChunkState,
    chunk_groups:: ChunkGroupCustom ,
    chunk_manager::ChunkManager,
    colliders::{ douglas_peucker, ACTOR_MASK, OBJECT_MASK },
    dirty_rect:: DirtyRects ,
    materials::PhysicsType,
    particle::{ Particle, ParticleBundle },
    pixel::Pixel,
};

#[derive(Bundle)]
pub struct ObjectBundle {
    pub object: Object,
    pub transform: TransformBundle,
    pub collider: Collider,
    pub velocity: Velocity,
    pub sleeping: Sleeping,
    pub mass_properties: ColliderMassProperties,
    pub rb: RigidBody,
    pub impulse: ExternalImpulse,
    pub collision_groups: CollisionGroups,
    pub read_mass: ReadMassProperties,
}

impl Default for ObjectBundle {
    fn default() -> Self {
        Self {
            object: Default::default(),
            transform: TransformBundle::default(),
            rb: RigidBody::Dynamic,
            velocity: Velocity::zero(),
            sleeping: Sleeping {
                normalized_linear_threshold: 0.25 / (CHUNK_SIZE as f32),
                ..Default::default()
            },
            impulse: ExternalImpulse::default(),
            read_mass: ReadMassProperties::default(),
            mass_properties: ColliderMassProperties::default(),
            collision_groups: CollisionGroups::new(
                Group::from_bits_truncate(OBJECT_MASK),
                Group::all()
            ),
            collider: Collider::default(),
        }
    }
}

#[derive(Default, Clone, Component)]
pub struct Object {
    pub size: IVec2,
    pub pixels: Vec<Option<Pixel>>,
    pub placed: bool,
    pub pixel_count: usize,
}

#[derive(Component, Clone)]
pub struct Projectile {
    pub damage: f32,
    pub launched_by: Option<Entity>,
    pub left_source: bool,
    pub timer: Timer,
    pub collided_with: Vec<Entity>,
    pub explosion_on_contact: Option<ExplosionParameters>,
    pub insert_on_contact: bool,
}

#[derive(Component, Clone)]
pub struct ExplosionParameters {
    pub radius: f32,
    pub damage: f32,
    pub force: f32,
}

impl Projectile {
    pub fn new(penetration_threshold_secs: f32, damage: f32) -> Self {
        Self {
            damage,
            launched_by: None,
            timer: Timer::from_seconds(penetration_threshold_secs, TimerMode::Once),
            collided_with: vec![],
            explosion_on_contact: None,
            insert_on_contact: false,
            left_source: false,
        }
    }

    pub fn with_source(mut self, launched_by: Entity) -> Self {
        self.launched_by = Some(launched_by);
        self
    }

    pub fn with_explosion(mut self, radius: f32, damage: f32, force: f32) -> Self {
        self.explosion_on_contact = Some(ExplosionParameters {
            radius,
            damage,
            force,
        });
        self
    }

    pub fn insert_on_contact(mut self) -> Self {
        self.insert_on_contact = true;
        self
    }
}

impl Object {
    pub fn from_pixels(pixels: Vec<Option<Pixel>>, size: IVec2) -> Result<Self, String> {
        if pixels.len() != ((size.x * size.y) as usize) {
            return Err("incorrect_size".to_string());
        }

        let pixel_count = pixels.iter().filter(|pixel| pixel.is_some()).count();

        Ok(Self {
            size,
            placed: false,
            pixels,
            pixel_count
        })
    }

    pub fn create_collider(&self) -> Result<Collider, String> {
        let values = self.pixels
            .iter()
            .map(|pixel| if pixel.is_some() { 1.0 } else { 0.0 })
            .collect::<Vec<f64>>();

        let contour_generator = contour::ContourBuilder::new(
            self.size.x as u32,
            self.size.y as u32,
            false
        );

        contour_generator
            .contours(&values, &[1.0])
            .map_err(|_| "no contours were found".to_string())
            .and_then(|contours| {
                contours[0]
                    .geometry()
                    .0.first()
                    .ok_or("no contours were found".to_string())
                    .map(|polygon| {
                        std::iter
                            ::once(polygon.exterior())
                            .chain(polygon.interiors().iter())
                            .map(|line| {
                                line.0
                                    .iter()
                                    .map(|point| {
                                        Vec2::new(
                                            ((point.x as f32) - (self.size.x as f32) / 2.0) /
                                                (CHUNK_SIZE as f32),
                                            ((point.y as f32) - (self.size.y as f32) / 2.0) /
                                                (CHUNK_SIZE as f32)
                                        )
                                    })
                                    .collect::<Vec<Vec2>>()
                            })
                            .map(|line| {
                                douglas_peucker(&line, 0.25 / (CHUNK_SIZE.pow(2) as f32))
                            })
                            .filter(|points| points.len() > 2)
                            .map(|line| {
                                line.into_iter()
                                    .map(|point| vec![point.x, point.y])
                                    .collect_vec()
                            })
                            .collect::<Vec<Vec<Vec<f32>>>>()
                    })
                    .and_then(|boundaries| {
                        if boundaries.is_empty() {
                            return Err("empty boundary was received".to_string());
                        }

                        let (vertices, holes, dimensions) = earcutr::flatten(&boundaries);

                        let Ok(triangles) = earcutr::earcut(&vertices, &holes, dimensions) else {
                            return Err("error occured during triangulation".to_string());
                        };

                        if triangles.is_empty() {
                            return Err("no triangles were constructed".to_owned());
                        }

                        let mut indices = vec![];
                        let mut converted_vertices = vec![];

                        for vertices in vertices.chunks_exact(2) {
                            converted_vertices.push(Vec2::new(vertices[0], vertices[1]));
                        }

                        for triangle in triangles.chunks_exact(3) {
                            indices.push([
                                triangle[0] as u32,
                                triangle[1] as u32,
                                triangle[2] as u32,
                            ]);
                        }

                        Ok(Collider::trimesh(converted_vertices, indices))
                    })
            })
    }

    pub fn iterate_over_pixels(
        &mut self,
        transform: &Transform
    ) -> impl Iterator<Item = (IVec2, &mut Option<Pixel>)> {
        let axis_angle = transform.rotation.to_axis_angle();

        let mut angle = axis_angle.1 * axis_angle.0.z;
        let object_position = transform.translation.xy() * (CHUNK_SIZE as f32);

        let size = self.size;

        let angle_modifier = if angle.abs() > FRAC_PI_2 {
            angle -= angle.signum() * PI;
            -1.0
        } else {
            1.0
        };

        self.pixels
            .iter_mut()
            .enumerate()
            .map(move |(index, object_pixel)| {
                let mut pixel_position =
                    Vec2::new(
                        (((index as i32) % size.x) as f32) - (size.x as f32) / 2.0 + 0.5,
                        (((index as i32) / size.x) as f32) - (size.y as f32) / 2.0 + 0.5
                    ) * angle_modifier;

                pixel_position.x = (
                    pixel_position.x -
                    pixel_position.y * f32::tan(angle / 2.0)
                ).floor();
                pixel_position.y = (pixel_position.x * f32::sin(angle) + pixel_position.y).floor();
                pixel_position.x = (
                    pixel_position.x -
                    pixel_position.y * f32::tan(angle / 2.0)
                ).floor();

                ((pixel_position + object_position).round().as_ivec2(), object_pixel)
            })
    }

    pub fn create_chunk_group(
        &self,
        transform: &Transform,
        chunk_manager: &mut ChunkManager
    ) -> (IVec2, ChunkGroupCustom<Pixel>) {
        let size = self.size.max_element() as f32;
        let position = transform.translation.xy() * (CHUNK_SIZE as f32);

        let chunk_group_position = Vec2::new(position.x - size / 2.0, position.y - size / 2.0)
            .floor()
            .as_ivec2()
            .div_euclid(IVec2::ONE * CHUNK_SIZE);

        let max_position = Vec2::new(position.x + size / 2.0, position.y + size / 2.0)
            .ceil()
            .as_ivec2()
            .div_euclid(IVec2::ONE * CHUNK_SIZE);

        let chunk_group_size = (
            max_position -
            chunk_group_position +
            IVec2::ONE
        ).max_element() as u8;

        let mut chunk_group = ChunkGroupCustom {
            chunks: HashMap::new(),
            size: CHUNK_SIZE,
        };

        for (x, y) in (0..chunk_group_size as i32).cartesian_product(0..chunk_group_size as i32) {
            if
                let Some(chunk) = chunk_manager.get_chunk_data_mut(
                    &(IVec2::new(x, y) + chunk_group_position)
                )
            {
                if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }
                chunk_group.chunks.insert(ivec2(x, y), chunk.pixels.as_mut_ptr());
            }
        }

        (chunk_group_position, chunk_group)
    }
}

pub fn process_projectiles(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mut damage_ev: EventWriter<DamageEvent>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut projectile_q: Query<(Entity, &Transform, &mut Object, &mut Projectile, &Velocity)>,
    actor_q: Query<&Transform, (With<Enemy>, Without<Projectile>)>,
    sensor_q: Query<Entity, With<Sensor>>,
    rigidbody_q: Query<Entity, (With<RigidBody>, Without<Sensor>)>,
    time: Res<Time>
) {
    for (entity, transform, mut object, mut parameters, velocity) in projectile_q.iter_mut() {
        if object.placed {
            continue;
        }

        if !parameters.left_source {
            if
                parameters.launched_by.is_none() ||
                rapier_context.intersection_pair(entity, parameters.launched_by.unwrap()).is_none()
            {
                parameters.left_source = true;
            }
        }

        let collided_with = rapier_context
            .intersection_pairs_with(entity)
            .filter(|pair| pair.2)
            .map(|pair| if pair.0 == entity { pair.1 } else { pair.0 })
            .filter_map(|collider_entity| {
                let rb_entity = rapier_context
                    .collider_parent(collider_entity)
                    .unwrap_or(collider_entity);

                if
                    (parameters.launched_by.is_some() &&
                        parameters.launched_by.unwrap() == rb_entity &&
                        !parameters.left_source) ||
                    parameters.collided_with.contains(&rb_entity)
                {
                    return None;
                }

                (!sensor_q.contains(collider_entity) && rigidbody_q.contains(rb_entity)).then_some(
                    rb_entity
                )
            })
            .collect_vec();

        for actor_entity in collided_with.iter() {
            if parameters.launched_by.map_or(false, |entity| {
                *actor_entity == entity
            }) {
                continue;
            }

            parameters.collided_with.push(*actor_entity);
            damage_ev.send(DamageEvent {
                target: *actor_entity,
                value: parameters.damage,
                knockback: velocity.linvel / 2.0,
                ignore_iframes: false,
                play_sound: true,
            });
        }

        if parameters.collided_with.is_empty() {
            continue;
        }

        parameters.timer.tick(time.delta());

        if parameters.timer.finished() {
            let (chunk_group_position, mut chunk_group) = object.create_chunk_group(
                transform,
                &mut chunk_manager
            );

            if let Some(explosion) = parameters.explosion_on_contact.as_ref() {
                let global_position = (transform.translation.xy() * (CHUNK_SIZE as f32)).as_ivec2();
                let local_position = global_position - chunk_group_position * CHUNK_SIZE;

                for x in -explosion.radius as i32..=explosion.radius as i32 {
                    for y in -explosion.radius as i32..=explosion.radius as i32 {
                        let offset = ivec2(x, y);

                        if (offset.length_squared() as f32) > explosion.radius.powi(2) {
                            continue;
                        }

                        let Some(pixel) = chunk_group.get_mut(local_position + offset) else {
                            continue;
                        };

                        if let Some(durability) = &mut pixel.durability {
                            *durability -= explosion.damage;
                            if *durability <= 0.0 {
                                *pixel = Pixel::default().with_clock(chunk_manager.clock());
                            }
                        }

                        dirty_rects_resource.request_update(global_position + offset);
                        dirty_rects_resource.request_render(global_position + offset);
                    }
                }

                rapier_context.intersections_with_shape(
                    transform.translation.xy(),
                    0.0,
                    &Collider::ball(explosion.radius / (CHUNK_SIZE as f32)),
                    QueryFilter::only_dynamic().groups(
                        CollisionGroups::new(Group::all(), Group::from_bits_retain(ACTOR_MASK))
                    ),
                    |entity| {
                        let rb = rapier_context.collider_parent(entity).unwrap_or(entity);
                        let Ok(actor_transform) = actor_q.get(rb) else {
                            return true;
                        };

                        damage_ev.send(DamageEvent {
                            target: entity,
                            value: explosion.damage,
                            knockback: explosion.force *
                            (
                                actor_transform.translation.xy() - transform.translation.xy()
                            ).normalize(),
                            ignore_iframes: false,
                            play_sound: true,
                        });

                        true
                    }
                );
            }

            if parameters.insert_on_contact {
                for (position, object_pixel) in object.iterate_over_pixels(transform) {
                    if object_pixel.is_none() {
                        continue;
                    }

                    let Some(world_pixel) = chunk_group.get_mut(
                        position - chunk_group_position * CHUNK_SIZE
                    ) else {
                        continue;
                    };

                    {
                        match world_pixel.physics_type {
                            PhysicsType::Powder | PhysicsType::Liquid(_) | PhysicsType::Gas(..) => {
                                let pixel = std::mem::take(world_pixel);

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
                                            (position.as_vec2() / (CHUNK_SIZE as f32)).extend(
                                                PARTICLE_Z
                                            )
                                        ),
                                        ..Default::default()
                                    },
                                    velocity: Velocity::linear(
                                        Vec2::new(
                                            fastrand::f32() - 0.5,
                                            fastrand::f32() / 2.0 + 0.5
                                        ) / (CHUNK_SIZE as f32)
                                    ),
                                    particle: Particle::new(pixel),
                                    ..Default::default()
                                });
                            }
                            PhysicsType::Static | PhysicsType::Rigidbody { .. } => {
                                continue;
                            }
                            _ => {}
                        }

                        *world_pixel = object_pixel.clone().unwrap();

                        dirty_rects_resource.request_update(position);
                        dirty_rects_resource.request_render(position);
                    }
                }
            }

            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn fill_objects(
    mut commands: Commands,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut object_q: Query<
        (Entity, &Transform, &mut Object, &Velocity, &mut ExternalImpulse),
        Without<Camera>
    >
) {
    for (entity, transform, mut object, velocity, mut impulse) in object_q.iter_mut() {
        if object.placed {
            continue;
        }

        let (chunk_group_position, mut chunk_group) = object.create_chunk_group(
            transform,
            &mut chunk_manager
        );

        for (position, object_pixel) in object.iterate_over_pixels(transform) {
            if object_pixel.is_none() {
                continue;
            }

            let Some(world_pixel) = chunk_group.get_mut(
                position - chunk_group_position * CHUNK_SIZE
            ) else {
                continue;
            };

            match world_pixel.physics_type {
                PhysicsType::Powder | PhysicsType::Liquid(_) | PhysicsType::Gas(..) => {
                    let pixel = std::mem::take(world_pixel);

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
                                (position.as_vec2() / (CHUNK_SIZE as f32)).extend(PARTICLE_Z)
                            ),
                            ..Default::default()
                        },
                        velocity: Velocity::linear(
                            Vec2::new(fastrand::f32() - 0.5, fastrand::f32() / 2.0 + 0.5) /
                                (CHUNK_SIZE as f32)
                        ),
                        particle: Particle::new(pixel),
                        ..Default::default()
                    });

                    impulse.impulse -= velocity.linvel / 100000.0;
                    impulse.torque_impulse -= velocity.angvel / 100_000_000_000.0;
                }
                PhysicsType::Static | PhysicsType::Rigidbody { .. } => {
                    continue;
                }
                _ => {}
            }

            *world_pixel = object_pixel.take().unwrap().with_physics(PhysicsType::Rigidbody(entity));

            dirty_rects_resource.request_update(position);
            dirty_rects_resource.request_render(position);
        }

        object.placed = true;
    }
}

pub fn unfill_objects(
    mut commands: Commands,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut object_q: Query<(Entity, &Transform, &mut Object, &Sleeping), Without<Camera>>
) {
    for (entity, transform, mut object, sleeping) in object_q.iter_mut() {
        if sleeping.sleeping {
            continue;
        }

        let (chunk_group_position, mut chunk_group) = object.create_chunk_group(
            transform,
            &mut chunk_manager
        );

        let mut new_pixel_count = 0;

        for (position, object_pixel) in object.iterate_over_pixels(transform) {
            if object_pixel.is_some() {
                new_pixel_count += 1;
                continue;
            }

            let Some(pixel) = chunk_group.get_mut(
                position - chunk_group_position * CHUNK_SIZE
            ) else {
                continue;
            };

            if let PhysicsType::Rigidbody(pixel_parent) = pixel.physics_type {
                if entity == pixel_parent {
                    *object_pixel = Some(mem::take(pixel).reset_physics());
                    dirty_rects_resource.request_render(position);
                    new_pixel_count += 1;
                }
            }
        }

        if new_pixel_count != object.pixel_count {
            dbg(new_pixel_count);
            if new_pixel_count < 32 {
                commands.entity(entity).despawn_recursive();
                continue;
            }

            object.pixel_count += new_pixel_count;

            if let Ok(collider) = object.create_collider() {
                commands.entity(entity).insert(collider);
            }
        }

        object.placed = false;
    }
}

pub fn object_collision_damage(
    rapier_context: Res<RapierContext>,
    mut damage_ev: EventWriter<DamageEvent>,
    mut object_q: Query<(Entity, &mut Velocity), With<Object>>,
    actor_q: Query<(Entity, &Transform), With<Enemy>>
) {
    for (entity, mut velocity) in object_q.iter_mut() {
        if velocity.linvel.length() < 1.0 {
            continue;
        }

        for pair in rapier_context.contact_pairs_with(entity) {
            let actor_entity = if pair.collider1() == entity {
                pair.collider2()
            } else {
                pair.collider1()
            };

            if actor_q.contains(actor_entity) {
                damage_ev.send(DamageEvent {
                    target: actor_entity,
                    value: velocity.linvel.length(),
                    knockback: velocity.linvel / 2.0,
                    ignore_iframes: false,
                    play_sound: true,
                });

                velocity.linvel *= 0.8;
            }
        }
    }
}

pub fn get_object_by_click(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut inventory: ResMut<Inventory>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    buttons: Res<ButtonInput<MouseButton>>,
    rapier_context: Res<RapierContext>,
    window_q: Query<(Entity, &Window), With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>,
    mut object_q: Query<(&Transform, &mut Object)>,
    mut egui_context: EguiContexts
) {
    let Ok((window_entity, window)) = window_q.get_single() else {
        return;
    };

    let (camera, camera_global_transform) = camera_q.single();

    if
        buttons.just_pressed(MouseButton::Middle) &&
        egui_context
            .try_ctx_for_window_mut(window_entity)
            .map_or(true, |ctx| !ctx.is_pointer_over_area())
    {
        if let Some(position) = window.cursor_position() {
            let point = camera
                .viewport_to_world(camera_global_transform, position)
                .map(|ray| ray.origin.truncate())
                .unwrap();

            rapier_context.intersections_with_point(point, QueryFilter::exclude_fixed(), |entity| {
                let Ok((transform, mut object)) = object_q.get_mut(entity) else {
                    return true;
                };

                if let Some(result) = inventory.cells.iter_mut().find(|cell| cell.is_none()) {
                    result.replace(Cell {
                        id: Id::new(
                            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
                        ),
                        texture: None,
                        object: object.clone(),
                    });
                    commands.entity(entity).despawn();
                } else {
                    return false;
                }

                let (chunk_group_position, mut chunk_group) = object.create_chunk_group(
                    transform,
                    &mut chunk_manager
                );

                for (position, _) in object.iterate_over_pixels(transform) {
                    if
                        let Some(pixel) = chunk_group.get_mut(
                            position - chunk_group_position * CHUNK_SIZE
                        )
                    {
                        if matches!(pixel.physics_type, PhysicsType::Rigidbody { .. }) {
                            *pixel = Pixel::default();

                            dirty_rects_resource.request_render(position);
                        }
                    }
                }

                false
            });
        }
    }
}
