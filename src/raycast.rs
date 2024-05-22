use bevy_math::IVec2;

use crate::{
    constants::CHUNK_SIZE,
    helpers::{to_index, WalkGrid},
    simulation::{chunk_manager::ChunkManager, materials::PhysicsType, pixel::Pixel},
};

pub fn raycast(start: IVec2, end: IVec2, chunk_manager: &ChunkManager) -> Option<(IVec2, Pixel)> {
    let mut chunk_position = start.div_euclid(IVec2::splat(CHUNK_SIZE));
    let mut chunk_ptr = chunk_manager.get_chunk_data(&chunk_position)
        .map(|chunk| chunk.pixels.as_ptr());

    if chunk_ptr.is_none() {
        return None;
    }

    for point in WalkGrid::new(start, end) {
        let current_chunk_position = point.div_euclid(IVec2::splat(CHUNK_SIZE));

        if current_chunk_position != chunk_position {
            chunk_position = current_chunk_position;
            chunk_ptr = chunk_manager
                .get_chunk_data(&current_chunk_position)
                .map(|chunk| chunk.pixels.as_ptr());

            if chunk_ptr.is_none() {
                return None;
            }
        }

        if let Some(pixel) = chunk_ptr.map(|ptr| 
            unsafe { &*ptr.add(to_index!(point.rem_euclid(IVec2::splat(CHUNK_SIZE)), CHUNK_SIZE)) }
        ).filter(|pixel| !matches!(pixel.material.physics_type, PhysicsType::Air | PhysicsType::Gas | PhysicsType::Liquid(..))) {
            return Some((point, pixel.clone()))
        }
    }

    None
} 
