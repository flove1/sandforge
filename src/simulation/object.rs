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
    constants::CHUNK_SIZE,
    gui::{ Cell, Inventory, ToastEvent },
};

use super::{
    chunk::ChunkState,
    chunk_groups::{ build_chunk_group, ChunkGroupCustom },
    chunk_manager::ChunkManager,
    dirty_rect::{ update_dirty_rects, DirtyRects },
    materials::{ Material, MaterialInstance, PhysicsType },
    mesh::{ douglas_peucker, ChunkColliderEveny, ACTOR_MASK, OBJECT_MASK },
    particle::{ Particle, ParticleInstances },
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
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<Option<Pixel>>,
    pub placed: bool,
}

#[derive(Component, Clone)]
pub struct ExplosionParameters {
    pub radius: i32,
    pub timer: Timer,
}

impl Object {
    pub fn from_pixels(
        pixels: Vec<Option<Material>>,
        width: u16,
        height: u16
    ) -> Result<Self, String> {
        if pixels.len() != ((width * height) as usize) {
            return Err("incorrect_size".to_string());
        }

        Ok(Self {
            width,
            height,
            placed: false,
            pixels: pixels
                .into_iter()
                .map(|material| {
                    material.map(|material| {
                        let mut pixel = Pixel::new(material.into());
                        pixel.material.physics_type = PhysicsType::Rigidbody;
                        pixel
                    })
                })
                .collect(),
        })
    }

    pub fn create_collider(&self) -> Result<Collider, String> {
        let values = self.pixels
            .iter()
            .map(|pixel| if pixel.is_some() { 1.0 } else { 0.0 })
            .collect::<Vec<f64>>();

        let contour_generator = contour::ContourBuilder::new(
            self.width as u32,
            self.height as u32,
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
                                            ((point.x as f32) - (self.width as f32) / 2.0) /
                                                (CHUNK_SIZE as f32),
                                            ((point.y as f32) - (self.height as f32) / 2.0) /
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
}

fn get_rotation_angle(transform: &Transform) -> f32 {
    transform.rotation.to_axis_angle().1 * transform.rotation.to_axis_angle().0.z
}

fn transpose_point(mut point: Vec2, angle: f32) -> Vec2 {
    point.x = (point.x - point.y * f32::tan(angle / 2.0)).floor();
    point.y = (point.x * f32::sin(angle) + point.y).floor();
    point.x = (point.x - point.y * f32::tan(angle / 2.0)).floor();

    point
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
                    *pixel = Pixel::new(Material::default().into()).with_clock(clock);

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

fn build_object_chunk_group(
    position: Vec2,
    object: &Object,
    chunk_manager: &mut ChunkManager
) -> (IVec2, ChunkGroupCustom<Pixel>) {
    let size = f32::max(object.width as f32, object.height as f32);

    let chunk_group_position = Vec2::new(position.x - size / 2.0, position.y - size / 2.0)
        .floor()
        .as_ivec2()
        .div_euclid(IVec2::ONE * CHUNK_SIZE);

    let max_position = Vec2::new(position.x + size / 2.0, position.y + size / 2.0)
        .ceil()
        .as_ivec2()
        .div_euclid(IVec2::ONE * CHUNK_SIZE);

    let chunk_group_size = (max_position - chunk_group_position + IVec2::ONE).max_element() as u8;

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

pub fn fill_objects(
    mut commands: Commands,
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut objects: Query<
        (&Transform, &mut Object, &Sleeping, &Velocity, &mut ExternalImpulse),
        Without<Camera>
    >,
    mut materials: ResMut<Assets<ColorMaterial>>,
    particles: Query<(Entity, &Mesh2dHandle), With<ParticleInstances>>
) {
    let (particles, particle_mesh) = particles.get_single().unwrap();
    let DirtyRects { current: dirty_rects, render: render_rects, .. } = &mut *dirty_rects_resource;

    for (transform, mut object, sleeping, velocity, mut impulse) in objects.iter_mut() {
        if sleeping.sleeping && object.placed {
            continue;
        }

        let mut angle = get_rotation_angle(transform);
        let position = transform.translation.xy() * (CHUNK_SIZE as f32);

        let mut angle_modifier = 1.0;

        if angle.abs() > FRAC_PI_2 {
            angle -= angle.signum() * PI;
            angle_modifier = -1.0;
        }

        let (chunk_group_position, mut chunk_group) = build_object_chunk_group(
            position,
            &object,
            &mut chunk_manager
        );

        let Object { width, height, pixels, .. } = object.as_mut();

        for (index, object_pixel) in pixels
            .iter_mut()
            .enumerate()
            .filter(|(_, object_pixel)| object_pixel.is_some()) {
            let mut pixel_position =
                Vec2::new(
                    (((index as u16) % *width) as f32) - (*width as f32) / 2.0 + 0.5,
                    (((index as u16) / *width) as f32) - (*height as f32) / 2.0 + 0.5
                ) * angle_modifier;

            pixel_position = transpose_point(pixel_position, angle);
            let floored_position = (pixel_position + position).round().as_ivec2();

            if
                let Some(world_pixel) = chunk_group.get_mut(
                    floored_position - chunk_group_position * CHUNK_SIZE
                )
            {
                match world_pixel.material.physics_type {
                    PhysicsType::Powder | PhysicsType::Liquid(_) | PhysicsType::Gas => {
                        let particle = Particle::new(
                            std::mem::take(world_pixel).material.clone(),
                            floored_position.as_vec2(),
                            Vec2::new(fastrand::f32() - 0.5, fastrand::f32() / 2.0 + 0.5)
                        );

                        let mesh = MaterialMesh2dBundle {
                            mesh: particle_mesh.clone(),
                            material: materials.add(
                                Color::rgba_u8(
                                    particle.material.color[0],
                                    particle.material.color[1],
                                    particle.material.color[2],
                                    particle.material.color[3]
                                )
                            ),
                            transform: Transform::from_translation(
                                (particle.pos / (CHUNK_SIZE as f32)).extend(-1.0)
                            ),
                            ..Default::default()
                        };

                        commands.entity(particles).with_children(|parent| {
                            parent.spawn((particle, mesh));
                        });

                        impulse.impulse -= velocity.linvel / 100000.0;
                        impulse.torque_impulse -= velocity.angvel / 100_000_000_000.0;
                    }
                    PhysicsType::Static | PhysicsType::Rigidbody => {
                        continue;
                    }
                    _ => {}
                }

                *world_pixel = object_pixel.clone().unwrap();

                update_dirty_rects(
                    dirty_rects,
                    floored_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                    floored_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2()
                );

                update_dirty_rects(
                    render_rects,
                    floored_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                    floored_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2()
                );
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
    let DirtyRects { render: render_rects, .. } = &mut *dirty_rects_resource;

    let clock = chunk_manager.clock();

    for (transform, mut object, sleeping) in objects.iter_mut() {
        if sleeping.sleeping {
            continue;
        }

        let mut angle = get_rotation_angle(transform);
        let position = transform.translation.xy() * (CHUNK_SIZE as f32);

        let mut angle_modifier = 1.0;

        if angle.abs() > FRAC_PI_2 {
            angle -= angle.signum() * PI;
            angle_modifier = -1.0;
        }

        let (chunk_group_position, mut chunk_group) = build_object_chunk_group(
            position,
            &object,
            &mut chunk_manager
        );

        let Object { width, height, pixels, .. } = object.as_mut();

        for (index, _) in pixels
            .iter()
            .enumerate()
            .filter(|(_, object_pixel)| object_pixel.is_some()) {
            let mut pixel_position =
                Vec2::new(
                    (((index as u16) % *width) as f32) - (*width as f32) / 2.0 + 0.5,
                    (((index as u16) / *width) as f32) - (*height as f32) / 2.0 + 0.5
                ) * angle_modifier;

            pixel_position = transpose_point(pixel_position, angle);
            let floored_position = (pixel_position + position).round().as_ivec2();

            if
                let Some(pixel) = chunk_group.get_mut(
                    floored_position - chunk_group_position * CHUNK_SIZE
                )
            {
                if pixel.material.physics_type == PhysicsType::Rigidbody {
                    *pixel = Pixel::new(Material::default().into()).with_clock(clock);

                    update_dirty_rects(
                        render_rects,
                        floored_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        floored_position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2()
                    );
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

        // rapier_context.intersections_with_shape(
        //     transform.translation.xy() + velocity.linvel * time.delta_seconds(),
        //     0.0,
        //     &collider,
        //     QueryFilter::only_dynamic().groups(
        //         CollisionGroups::new(Group::from_bits_retain(ACTOR_MASK), Group::all())
        //     ),
        //     |actor_entity| {
        //         if actor_q.contains(actor_entity) {
        //             damage_ev.send(DamageEvent {
        //                 target: actor_entity,
        //                 value: velocity.linvel.length(),
        //                 knockback: velocity.linvel / 2.0,
        //             });

        //             velocity.linvel *= 0.8;
        //         }

        //         true
        //     }
        // );

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
    let (window_entity, window) = window_q.single();
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

                let DirtyRects { render: render_rects, .. } = &mut *dirty_rects_resource;

                let mut angle = get_rotation_angle(transform);
                let position = transform.translation.xy() * (CHUNK_SIZE as f32);

                let mut angle_modifier = 1.0;

                if angle.abs() > FRAC_PI_2 {
                    angle -= angle.signum() * PI;
                    angle_modifier = -1.0;
                }

                let (chunk_group_position, mut chunk_group) = build_object_chunk_group(
                    position,
                    &object,
                    &mut chunk_manager
                );

                let Object { width, height, pixels, .. } = object.as_mut();

                for (index, _) in pixels
                    .iter()
                    .enumerate()
                    .filter(|(_, object_pixel)| object_pixel.is_some()) {
                    let pixel_position =
                        Vec2::new(
                            (((index as u16) % *width) as f32) - (*width as f32) / 2.0 + 0.5,
                            (((index as u16) / *width) as f32) - (*height as f32) / 2.0 + 0.5
                        ) * angle_modifier;

                    let rotated_pixel_position = (transpose_point(pixel_position, angle) + position)
                        .round()
                        .as_ivec2();

                    if
                        let Some(pixel) = chunk_group.get_mut(
                            rotated_pixel_position - chunk_group_position * CHUNK_SIZE
                        )
                    {
                        if pixel.material.physics_type == PhysicsType::Rigidbody {
                            *pixel = Pixel::new(Material::default().into());

                            update_dirty_rects(
                                render_rects,
                                rotated_pixel_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                                rotated_pixel_position
                                    .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                                    .as_uvec2()
                            );
                        }
                    }
                }

                false
            });
        }
    }
}
