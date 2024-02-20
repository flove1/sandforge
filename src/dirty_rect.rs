use bevy::{asset::{Asset, Assets, Handle}, core::Name, ecs::{component::Component, entity::Entity, query::With, system::{Commands, Query, Res, ResMut, Resource}}, gizmos::gizmos::Gizmos, hierarchy::{BuildChildren, DespawnRecursiveExt}, pbr::wireframe::WireframeMaterial, prelude::default, reflect::TypePath, render::{color::Color, mesh::{shape, Mesh}, render_resource::AsBindGroup, texture::Image}, sprite::{ColorMaterial, MaterialMesh2dBundle}, transform::{commands, components::Transform}, utils::HashMap};
use bevy_math::{ivec2, IVec2, URect, UVec2, Vec2, Vec3};
use itertools::Itertools;

use crate::{constants::CHUNK_SIZE, helpers::{global_to_local, local_to_global, modify_local_position}};

pub fn dirty_rects_gizmos(
    mut gizmos: Gizmos,
    dirty_rects_resource: Res<DirtyRects>,
) {
    dirty_rects_resource.current.iter()
        .for_each(|(position, rect)| {
            gizmos.rect_2d(position.as_vec2() + ((rect.center().as_vec2() + Vec2::ONE / 4.0) / CHUNK_SIZE as f32), 0.0, rect.size().as_vec2() / CHUNK_SIZE as f32 , Color::RED);
        });
}

#[derive(Resource, Default)]
pub struct DirtyRects {
    pub current: HashMap<IVec2, URect>,
    pub new: HashMap<IVec2, URect>,
    pub render: HashMap<IVec2, URect>
}

impl DirtyRects {
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.current, &mut self.new)
    }
}

#[derive(Debug, Default)]
pub struct UpdateMessage {
    pub chunk_position: IVec2,
    pub cell_position: UVec2,
    pub awake_surrouding: bool,
}

#[derive(Debug, Default)]
pub struct RenderMessage {
    pub chunk_position: IVec2,
    pub cell_position: UVec2,
}

pub fn update_dirty_rects(
    dirty_rects: &mut HashMap<IVec2, URect>,
    chunk_position: IVec2,
    cell_position: UVec2,
) {
    if let Some(dirty_rects) = dirty_rects.get_mut(&chunk_position) {
        extend_rect_if_needed(dirty_rects, &cell_position)
    } else {
        dirty_rects.insert(
            chunk_position,
            URect::from_corners(
                cell_position.clamp(UVec2::ZERO, UVec2::ONE * (CHUNK_SIZE as u32 - 1)),
                cell_position.saturating_add(UVec2::ONE).clamp(UVec2::ZERO, UVec2::ONE * (CHUNK_SIZE as u32)),
            )
        );
    }
}

pub fn update_dirty_rects_3x3(
    dirty_rects: &mut HashMap<IVec2, URect>, 
    chunk_position: IVec2,
    cell_position: UVec2,
) {
    if let Some(rect) = dirty_rects.get_mut(&chunk_position) {
        extend_rect_if_needed(rect, &(cell_position.saturating_add(UVec2::ONE)));
        extend_rect_if_needed(rect, &(cell_position.saturating_sub(UVec2::ONE)));
    } else {
        dirty_rects.insert(
            chunk_position,
            URect::from_corners(
                cell_position.saturating_sub(UVec2::ONE).clamp(UVec2::ZERO, UVec2::ONE * (CHUNK_SIZE as u32 - 1)),
                cell_position.saturating_add(UVec2::ONE * 2).clamp(UVec2::ZERO, UVec2::ONE * (CHUNK_SIZE as u32)),
            )
        );
    }

    let chunk_offset = ivec2(
         (cell_position.x == CHUNK_SIZE as u32 - 1) as i32 - (cell_position.x == 0) as i32,
         (cell_position.y == CHUNK_SIZE as u32 - 1) as i32 - (cell_position.y == 0) as i32,
    );

    match chunk_offset {
        IVec2::ZERO => {},
        IVec2::ONE | IVec2::NEG_ONE => {
            for (x, y) in (-1..=1).cartesian_product(-1..=1) {
                let (chunk_position, cell_position) = modify_local_position(chunk_position, cell_position, ivec2(x, y));
    
                if let Some(rect) = dirty_rects.get_mut(&chunk_position) {
                    extend_rect_if_needed(rect, &cell_position)
                } else {
                    dirty_rects.insert(
                        chunk_position,
                        URect::from_corners(
                            cell_position.clamp(UVec2::ZERO, UVec2::ONE * (CHUNK_SIZE as u32 - 1)),
                            cell_position.saturating_add(UVec2::ONE * 2).clamp(UVec2::ZERO, UVec2::ONE * (CHUNK_SIZE as u32)),
                        )
                    );
                }
            }
        },
        IVec2{x, y} => {
            let (chunk_position, cell_position) = modify_local_position(chunk_position, cell_position, ivec2(x, y));

            if let Some(rect) = dirty_rects.get_mut(&chunk_position) {
                extend_rect_if_needed(rect, &cell_position)
            } else {
                dirty_rects.insert(
                    chunk_position,
                    URect::from_corners(
                        cell_position.clamp(UVec2::ZERO, UVec2::ONE * (CHUNK_SIZE as u32 - 1)),
                        cell_position.saturating_add(UVec2::ONE * 2).clamp(UVec2::ZERO, UVec2::ONE * (CHUNK_SIZE as u32)),
                    )
                );
            }
        }
    }
}

pub fn extend_rect_if_needed(rect: &mut URect, pos: &UVec2) {
    rect.min.x = u32::min(rect.min.x, pos.x).clamp(0, CHUNK_SIZE as u32 - 1);
    rect.max.x = u32::max(rect.max.x, pos.x + 1).clamp(0, CHUNK_SIZE as u32);

    rect.min.y = u32::min(rect.min.y, pos.y).clamp(0, CHUNK_SIZE as u32 - 1);
    rect.max.y = u32::max(rect.max.y, pos.y + 1).clamp(0, CHUNK_SIZE as u32);
}