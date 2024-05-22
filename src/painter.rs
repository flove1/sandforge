use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    sprite::{ MaterialMesh2dBundle, Mesh2dHandle },
    utils::{ HashMap, HashSet },
    window::PrimaryWindow,
};
use bevy_egui::EguiContexts;
use bevy_math::{ ivec2, vec2, IVec2 };
use bevy_rapier2d::{
    dynamics::{ ExternalImpulse, ReadMassProperties, RigidBody, Sleeping, Velocity },
    geometry::{ Collider, ColliderMassProperties, CollisionGroups, Group },
};

use crate::{
    camera::TrackingCamera,
    constants::{ CHUNK_SIZE, PARTICLE_LAYER },
    has_window,
    helpers::WalkGrid,
    simulation::{
        chunk::Chunk,
        chunk_manager::ChunkManager,
        dirty_rect::{ update_dirty_rects, DirtyRects },
        materials::{ Material, MaterialInstance, PhysicsType },
        mesh::{ ChunkColliderEveny, ACTOR_MASK, OBJECT_MASK, TERRAIN_MASK },
        object::{ ExplosionParameters, Object, ObjectBundle },
        particle::{ Particle, ParticleInstances },
    },
    state::AppState,
};

pub struct PainterPlugin;

impl Plugin for PainterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MouseState>()
            .init_resource::<BrushRes>()
            .init_resource::<PainterObjectBuffer>()
            .add_systems(
                PreUpdate,
                mouse_system.run_if(has_window).run_if(in_state(AppState::Game))
            );
    }
}

#[derive(Default, Resource, PartialEq, Eq)]
enum MouseState {
    #[default]
    Normal,
    Painting,
}

#[derive(Resource)]
pub struct BrushRes {
    pub material: Option<Material>,
    pub brush_type: BrushType,
    pub shape: BrushShape,
    pub size: i32,
}

#[derive(Clone, PartialEq)]
pub enum BrushType {
    Cell,
    Object,
    Particle(u8),
}

#[derive(Clone, PartialEq)]
pub enum BrushShape {
    Circle,
    Square,
}

impl BrushShape {
    pub fn draw<F: FnMut(IVec2)>(&self, position: IVec2, size: i32, operation: &mut F) {
        match self {
            BrushShape::Circle => {
                for dx in -size..=size {
                    for dy in -size..=size {
                        if dx.pow(2) + dy.pow(2) > size.pow(2) {
                            continue;
                        }

                        operation(position + ivec2(dx, dy));
                    }
                }
            }
            BrushShape::Square => {
                for dx in -size..=size {
                    for dy in -size..=size {
                        operation(position + ivec2(dx, dy));
                    }
                }
            }
        }
    }
}

impl FromWorld for BrushRes {
    fn from_world(_world: &mut World) -> Self {
        Self {
            material: None,
            brush_type: BrushType::Cell,
            shape: BrushShape::Circle,
            size: 10,
        }
    }
}

#[derive(Resource, Default)]
pub struct PainterObjectBuffer {
    pub map: HashMap<IVec2, Material>,
}

#[allow(clippy::too_many_arguments)]
fn mouse_system(
    mut commands: Commands,
    brush: Res<BrushRes>,
    window_q: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut dirty_rects: ResMut<DirtyRects>,
    mut motion_evr: EventReader<MouseMotion>,
    mut cursor_evr: EventReader<CursorMoved>,
    mut camera: Query<(&Camera, &mut Transform, &GlobalTransform), With<TrackingCamera>>,
    mut contexts: EguiContexts,
    mut mouse_state: ResMut<MouseState>,
    mut object_buffer: ResMut<PainterObjectBuffer>,
    partcle_q: Query<(Entity, &Mesh2dHandle), With<ParticleInstances>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut chunk_collider_ev: EventWriter<ChunkColliderEveny>
) {
    let (particles, particle_mesh) = partcle_q.get_single().unwrap();
    let (camera, mut camera_transform, camera_global_transform) = camera.single_mut();
    let (window_entity, window) = window_q.single();

    let mut buffer = HashMap::new();

    let mut draw_operation = |position: IVec2| {
        if brush.material.is_none() {
            return;
        }

        match brush.brush_type {
            BrushType::Particle(rate) => {
                if fastrand::u8(0..255) <= rate {
                    let particle = Particle::new(
                        brush.material.as_ref().unwrap().into(),
                        position.as_vec2(),
                        vec2(fastrand::f32() - 0.5, fastrand::f32()) * 4.0
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
                            (particle.pos / (CHUNK_SIZE as f32)).extend(2.0)
                        ),
                        ..Default::default()
                    };

                    let particle_handle = commands.spawn((particle, mesh)).id();

                    commands.entity(particles).add_child(particle_handle);

                    let chunk_position = position.div_euclid(IVec2::ONE * CHUNK_SIZE);
                    let cell_position = position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2();

                    update_dirty_rects(&mut dirty_rects.current, chunk_position, cell_position);
                    update_dirty_rects(&mut dirty_rects.render, chunk_position, cell_position);
                }
            }
            BrushType::Object => {
                if brush.material.as_ref().unwrap().matter_type == PhysicsType::Air {
                    buffer.insert(position, brush.material.as_ref().unwrap().into());
                } else {
                    object_buffer.map.insert(position, brush.material.as_ref().unwrap().clone());
                }
            }
            _ => {
                buffer.insert(position, brush.material.as_ref().unwrap().into());
            }
        }
    };

    if
        buttons.just_pressed(MouseButton::Left) &&
        contexts
            .try_ctx_for_window_mut(window_entity)
            .map_or(true, |ctx| !ctx.is_pointer_over_area())
    {
        mouse_state.set_if_neq(MouseState::Painting);
        if let Some(position) = window.cursor_position() {
            let world_position = camera
                .viewport_to_world(camera_global_transform, position)
                .map(|ray| ray.origin.truncate())
                .unwrap();

            brush.shape.draw(
                (world_position * (CHUNK_SIZE as f32)).round().as_ivec2(),
                brush.size,
                &mut draw_operation
            );
        }
    }

    if buttons.pressed(MouseButton::Left) {
        match mouse_state.as_ref() {
            MouseState::Painting => {
                if let Some(cursor_position) = window.cursor_position() {
                    let mut last_position = camera
                        .viewport_to_world(camera_global_transform, cursor_position)
                        .map(|ray| ray.origin.truncate())
                        .unwrap();

                    let movement_events = cursor_evr.read().collect::<Vec<&CursorMoved>>();
                    for event in movement_events.iter().rev() {
                        let new_position = camera
                            .viewport_to_world(camera_global_transform, event.position)
                            .map(|ray| ray.origin.truncate())
                            .unwrap();

                        for position in WalkGrid::new(
                            (last_position * (CHUNK_SIZE as f32)).round().as_ivec2(),
                            (new_position * (CHUNK_SIZE as f32)).round().as_ivec2()
                        ) {
                            brush.shape.draw(position, brush.size, &mut draw_operation);
                        }

                        last_position = new_position;
                    }
                }
            }
            _ => {}
        };
    }

    let mut affected_chunks = HashSet::new();
    for (position, material) in buffer {
        if chunk_manager.set(position, material).is_ok() {
            let chunk_position = position.div_euclid(IVec2::ONE * CHUNK_SIZE);
            let cell_position = position.rem_euclid(IVec2::ONE * CHUNK_SIZE).as_uvec2();

            affected_chunks.insert(chunk_position);

            update_dirty_rects(&mut dirty_rects.current, chunk_position, cell_position);
            update_dirty_rects(&mut dirty_rects.render, chunk_position, cell_position);
        }
    }

    chunk_collider_ev.send_batch(
        affected_chunks.into_iter().map(|position| ChunkColliderEveny(position))
    );

    cursor_evr.clear();
    motion_evr.clear();

    if buttons.just_released(MouseButton::Left) {
        if brush.brush_type == BrushType::Object {
            let mut rect: Option<IRect> = None;
            let values = object_buffer.map.drain().collect::<Vec<(IVec2, Material)>>();

            values.iter().for_each(|(pos, _)| {
                let rect = rect.get_or_insert(IRect::new(pos.x, pos.y, pos.x + 1, pos.y + 1));

                rect.min.x = i32::min(rect.min.x, pos.x);
                rect.max.x = i32::max(rect.max.x, pos.x + 1);

                rect.min.y = i32::min(rect.min.y, pos.y);
                rect.max.y = i32::max(rect.max.y, pos.y + 1);
            });

            if let Some(rect) = rect {
                let mut pixels: Vec<Option<Material>> =
                    vec![None; (rect.size().x * rect.size().y) as usize];

                values.iter().for_each(|(pos, material)| {
                    let offseted_pos = *pos - rect.min;

                    pixels[(offseted_pos.y * rect.size().x + offseted_pos.x) as usize] = Some(
                        material.clone()
                    );
                });

                if
                    let Ok(object) = Object::from_pixels(
                        pixels,
                        rect.size().x as u16,
                        rect.size().y as u16
                    )
                {
                    if let Ok(collider) = object.create_collider() {
                        commands.spawn((
                            ObjectBundle {
                                object,
                                collider,
                                transform: TransformBundle {
                                    local: Transform::from_translation(
                                        rect.center().extend(0).as_vec3() / (CHUNK_SIZE as f32)
                                    ),
                                    ..Default::default()
                                },
                                mass_properties: ColliderMassProperties::Density(2.0),
                                ..Default::default()
                            },
                            // ExplosionParameters {
                            //     radius: 64,
                            //     timer: Timer::from_seconds(4.0, TimerMode::Once),
                            // },
                        ));
                    }
                }
            }
        }

        mouse_state.set_if_neq(MouseState::Normal);
    }
}
