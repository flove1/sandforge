use std::{ f32::consts::{ FRAC_PI_2, PI }, time::{ Duration, SystemTime, UNIX_EPOCH } };

use bevy::{
    prelude::*,
    sprite::{ MaterialMesh2dBundle, Mesh2dHandle },
    utils::HashMap,
    window::PrimaryWindow,
};
use bevy_egui::{ egui::Id, EguiContexts };
use bevy_math::ivec2;
use bevy_rapier2d::prelude::*;
use itertools::Itertools;

use crate::{
    actors::{ actor::Actor, enemy::Enemy, health::DamageEvent },
    camera::TrackingCamera,
    constants::{ CHUNK_SIZE, PARTICLE_Z },
    gui::{ Cell, Inventory, ToastEvent },
};

use super::{
    chunk::ChunkState,
    chunk_groups::{ build_chunk_group, ChunkGroupCustom },
    chunk_manager::ChunkManager,
    colliders::{ douglas_peucker, ChunkColliderEveny, OBJECT_MASK },
    dirty_rect::{ update_dirty_rects, DirtyRects },
    materials::{ Material, PhysicsType },
    particle::{ Particle, ParticleBundle, ParticleParent },
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
            sleeping: Sleeping::default(),
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
}

#[derive(Component)]
pub struct FallApartOnCollision;

#[derive(Component, Clone)]
pub struct ExplosionParameters {
    pub radius: i32,
    pub timer: Timer,
}

impl Object {
    pub fn from_pixels(pixels: Vec<Option<Pixel>>, size: IVec2) -> Result<Self, String> {
        if pixels.len() != ((size.x * size.y) as usize) {
            return Err("incorrect_size".to_string());
        }

        Ok(Self {
            size,
            placed: false,
            pixels,
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
            .filter(|(_, object_pixel)| object_pixel.is_some())
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

pub fn process_explosion(
    mut commands: Commands,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut object_q: Query<(Entity, &Transform, &mut ExplosionParameters), With<Object>>,
    mut chunk_collider_ev: EventWriter<ChunkColliderEveny>,
    time: Res<Time>
) {
    let DirtyRects { current: dirty_rects, render: render_rects, .. } = &mut *dirty_rects_resource;
    let clock = chunk_manager.clock();

    for (entity, transform, mut explosion_parameters) in object_q.iter_mut() {
        explosion_parameters.timer.tick(time.delta());

        if !explosion_parameters.timer.finished() {
            continue;
        }

        let position = (transform.translation.xy() * (CHUNK_SIZE as f32)).round().as_ivec2();
        let chunk_position = position.div_euclid(IVec2::splat(CHUNK_SIZE));

        let Some(mut chunk_group) = build_chunk_group(&mut chunk_manager, chunk_position) else {
            continue;
        };

        for x in -explosion_parameters.radius..=explosion_parameters.radius {
            for y in -explosion_parameters.radius..=explosion_parameters.radius {
                if x.pow(2) + y.pow(2) > explosion_parameters.radius.pow(2) {
                    continue;
                }

                let pixel_position = position + ivec2(x, y);

                if
                    let Some(pixel) = chunk_group.get_mut(
                        pixel_position - chunk_position * CHUNK_SIZE
                    )
                {
                    *pixel = Pixel::default().with_clock(clock);

                    update_dirty_rects(
                        dirty_rects,
                        pixel_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        pixel_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2()
                    );

                    update_dirty_rects(
                        render_rects,
                        pixel_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        pixel_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2()
                    );
                }
            }
        }

        chunk_collider_ev.send_batch(
            (-1..=1)
                .cartesian_product(-1..=1)
                .map(|(x, y)| ChunkColliderEveny(chunk_position + ivec2(x, y)))
        );

        commands.entity(entity).despawn_recursive();
    }
}

pub fn process_fall_apart_on_collision(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut object_q: Query<(Entity, &Transform, &mut Object), With<FallApartOnCollision>>
) {
    for (entity, transform, mut object) in object_q.iter_mut() {
        if
            rapier_context
                .contact_pairs_with(entity)
                .filter(|pair| pair.has_any_active_contacts())
                .next()
                .is_some()
        {
            let (chunk_group_position, mut chunk_group) = object.create_chunk_group(
                transform,
                &mut chunk_manager
            );

            for (position, object_pixel) in object.iterate_over_pixels(transform) {
                if
                    let Some(world_pixel) = chunk_group.get_mut(
                        position - chunk_group_position * CHUNK_SIZE
                    )
                {
                    {
                        match world_pixel.physics_type {
                            PhysicsType::Powder | PhysicsType::Liquid(_) | PhysicsType::Gas => {
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
                            PhysicsType::Static | PhysicsType::Rigidbody => {
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
    mut objects: Query<
        (&Transform, &mut Object, &Sleeping, &Velocity, &mut ExternalImpulse),
        Without<Camera>
    >
) {
    for (transform, mut object, sleeping, velocity, mut impulse) in objects.iter_mut() {
        if sleeping.sleeping && object.placed {
            continue;
        }

        let (chunk_group_position, mut chunk_group) = object.create_chunk_group(
            transform,
            &mut chunk_manager
        );

        for (position, object_pixel) in object.iterate_over_pixels(transform) {
            if
                let Some(world_pixel) = chunk_group.get_mut(
                    position - chunk_group_position * CHUNK_SIZE
                )
            {
                match world_pixel.physics_type {
                    PhysicsType::Powder | PhysicsType::Liquid(_) | PhysicsType::Gas => {
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
                    PhysicsType::Static | PhysicsType::Rigidbody => {
                        continue;
                    }
                    _ => {}
                }

                *world_pixel = object_pixel.clone().unwrap().with_physics(PhysicsType::Rigidbody);

                dirty_rects_resource.request_update(position);
                dirty_rects_resource.request_render(position);
            }
        }

        object.placed = true;
    }
}

pub fn unfill_objects(
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut objects: Query<(&Transform, &mut Object, &Sleeping), Without<Camera>>
) {
    let clock = chunk_manager.clock();

    for (transform, mut object, sleeping) in objects.iter_mut() {
        if sleeping.sleeping {
            continue;
        }

        let (chunk_group_position, mut chunk_group) = object.create_chunk_group(
            transform,
            &mut chunk_manager
        );

        for (position, object_pixel) in object.iterate_over_pixels(transform) {
            if let Some(pixel) = chunk_group.get_mut(position - chunk_group_position * CHUNK_SIZE) {
                if pixel.physics_type == PhysicsType::Rigidbody {
                    *pixel = Pixel::default().with_clock(clock);

                    dirty_rects_resource.request_render(position);
                }
            }
        }

        object.placed = false;
    }
}

pub fn object_collision_damage(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    time: Res<Time>,
    mut damage_ev: EventWriter<DamageEvent>,
    mut object_q: Query<(Entity, &Transform, &Collider, &Object, &mut Velocity)>,
    actor_q: Query<(Entity, &Transform), With<Enemy>>
) {
    for (entity, transform, collider, object, mut velocity) in object_q.iter_mut() {
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
    mut egui_context: EguiContexts,
    mut events: EventWriter<ToastEvent>
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
                    events.send(ToastEvent {
                        content: "Inventory if full!".to_string(),
                        level: egui_notify::ToastLevel::Error,
                        duration: Duration::from_secs(2),
                    });

                    return false;
                }

                let (chunk_group_position, mut chunk_group) = object.create_chunk_group(
                    transform,
                    &mut chunk_manager
                );

                for (position, pixel) in object.iterate_over_pixels(transform) {
                    if
                        let Some(pixel) = chunk_group.get_mut(
                            position - chunk_group_position * CHUNK_SIZE
                        )
                    {
                        if pixel.physics_type == PhysicsType::Rigidbody {
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
