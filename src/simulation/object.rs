use std::f32::consts::{FRAC_PI_2, PI};

use bevy::{prelude::*, utils::HashMap};
use bevy_math::ivec2;
use bevy_rapier2d::prelude::*;
use itertools::Itertools;

use crate::constants::{CHUNK_SIZE, COLLIDER_PRECISION};

use super::{
    chunk::ChunkState, chunk_groups::ChunkGroupCustom, chunk_manager::ChunkManager, dirty_rect::{update_dirty_rects, DirtyRects}, materials::{Material, MaterialInstance, PhysicsType}, mesh::douglas_peucker, pixel::Pixel
};

#[derive(Component)]
pub struct Object {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<Option<Pixel>>,
}

impl Object {
    pub fn from_pixels(
        pixels: Vec<Option<MaterialInstance>>,
        width: u16,
        height: u16,
    ) -> Result<Self, String> {
        if pixels.len() != (width * height) as usize {
            return Err("incorrect_size".to_string());
        }

        Ok(Self {
            width,
            height,
            pixels: pixels
                .into_iter()
                .map(|material| {
                    material.map(|material| {
                        let mut pixel = Pixel::new(material, 0);
                        pixel.material.physics_type = PhysicsType::Rigidbody;
                        pixel
                    })
                })
                .collect(),
        })
    }

    pub fn create_collider(&self) -> Result<Collider, String> {
        let values = self
            .pixels
            .iter()
            .map(|pixel| if pixel.is_some() { 1.0 } else { 0.0 })
            .collect::<Vec<f64>>();

        let contour_generator =
            contour::ContourBuilder::new(self.width as u32, self.height as u32, false);
        contour_generator
            .contours(&values, &[1.0])
            .map_err(|_| "no contours were found".to_string())
            .and_then(|contours| {
                contours[0]
                    .geometry()
                    .0
                    .first()
                    .ok_or("no contours were found".to_string())
                    .map(|polygon| {
                        std::iter::once(polygon.exterior())
                            .chain(polygon.interiors().iter())
                            .map(|line| {
                                line.0
                                    .iter()
                                    .map(|point| {
                                        Vec2::new(
                                            (point.x as f32 - self.width as f32 / 2.0)
                                                / CHUNK_SIZE as f32,
                                            (point.y as f32 - self.height as f32 / 2.0)
                                                / CHUNK_SIZE as f32,
                                        )
                                    })
                                    .collect::<Vec<Vec2>>()
                            })
                            .map(|line| {
                                douglas_peucker(
                                    &line,
                                    COLLIDER_PRECISION / CHUNK_SIZE.pow(2) as f32,
                                )
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

fn build_chunk_group(
    position: Vec2,
    object: &Object,
    chunk_manager: &mut ChunkManager,
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
        if let Some(chunk) = chunk_manager
            .chunks
            .get_mut(&(IVec2::new(x, y) + chunk_group_position))
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
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    objects: Query<(&Transform, &Object), Without<Camera>>,
) {
    let DirtyRects {
        current: dirty_rects,
        render: render_rects,
        ..
    } = &mut *dirty_rects_resource;

    for (transform, object) in objects.iter() {
        let mut angle = get_rotation_angle(transform);
        let position = transform.translation.xy() * CHUNK_SIZE as f32;

        let mut angle_modifier = 1.0;

        if angle.abs() > FRAC_PI_2 {
            angle -= angle.signum() * PI;
            angle_modifier = -1.0;
        }

        let (chunk_group_position, mut chunk_group) = build_chunk_group(position, object, &mut chunk_manager);

        for (index, object_pixel) in object
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, object_pixel)| object_pixel.is_some())
        {
            let mut pixel_position = Vec2::new(
                (index as u16 % object.width) as f32 - object.width as f32 / 2.0,
                (index as u16 / object.width) as f32 - object.height as f32 / 2.0,
            ) * angle_modifier;

            pixel_position = transpose_point(pixel_position, angle);
            let floored_position = (pixel_position + position).floor().as_ivec2();

            if let Some(pixel) = chunk_group.get_mut(floored_position - chunk_group_position * CHUNK_SIZE) {
                if pixel.is_empty() {
                    *pixel = unsafe { object_pixel.as_ref().unwrap_unchecked().clone() };

                    update_dirty_rects(
                        dirty_rects,
                        floored_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        floored_position
                            .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                            .as_uvec2(),
                    );

                    update_dirty_rects(
                        render_rects,
                        floored_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        floored_position
                            .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                            .as_uvec2(),
                    );
                }
            }
        }
    }
}

pub fn unfill_objects(
    mut dirty_rects_resource: ResMut<DirtyRects>,
    mut chunk_manager: ResMut<ChunkManager>,
    objects: Query<(&Transform, &Object), Without<Camera>>,
) {
    let DirtyRects {
        render: render_rects,
        ..
    } = &mut *dirty_rects_resource;

    let clock = chunk_manager.clock();

    for (transform, object) in objects.iter() {
        let mut angle = get_rotation_angle(transform);
        let position = transform.translation.xy() * CHUNK_SIZE as f32;

        let mut angle_modifier = 1.0;

        if angle.abs() > FRAC_PI_2 {
            angle -= angle.signum() * PI;
            angle_modifier = -1.0;
        }

        let (chunk_group_position, mut chunk_group) = build_chunk_group(position, object, &mut chunk_manager);

        for (index, _) in object
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, object_pixel)| object_pixel.is_some())
        {
            let mut pixel_position = Vec2::new(
                (index as u16 % object.width) as f32 - object.width as f32 / 2.0,
                (index as u16 / object.width) as f32 - object.height as f32 / 2.0,
            ) * angle_modifier;

            pixel_position = transpose_point(pixel_position, angle);
            let floored_position = (pixel_position + position).floor().as_ivec2();

            if let Some(pixel) = chunk_group.get_mut(floored_position - chunk_group_position * CHUNK_SIZE) {
                if pixel.material.physics_type == PhysicsType::Rigidbody {
                    *pixel = Pixel::new(Material::default().into(), clock);

                    update_dirty_rects(
                        render_rects,
                        floored_position.div_euclid(IVec2::ONE * CHUNK_SIZE),
                        floored_position
                            .rem_euclid(IVec2::ONE * CHUNK_SIZE)
                            .as_uvec2(),
                    );
                }
            }
        }
    }
}
