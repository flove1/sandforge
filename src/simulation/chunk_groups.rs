use crate::{constants::CHUNK_SIZE, helpers::to_index};
use bevy::{prelude::*, utils::HashMap};
use bevy_math::ivec2;
use itertools::Itertools;

use super::{chunk::ChunkState, chunk_manager::ChunkManager, pixel::Pixel};

// 6 7 8
// 3 4 5
// 0 1 2
pub struct ChunkGroup<T: Clone> {
    pub size: i32,
    pub center: *const T,
    pub corners: [Option<*const T>; 4],
    pub sides: [Option<*const T>; 4],
    pub centered: bool,
}

pub fn build_chunk_group(chunk_manager: &ChunkManager, position: IVec2, centered: bool) -> Option<ChunkGroup<Pixel>> {
    let center_ptr = if let Some(chunk) = chunk_manager.get_chunk_data(&position) {
        chunk.pixels.as_ptr()
    } else {
        return None;
    };

    let mut chunk_group = ChunkGroup {
        size: CHUNK_SIZE,
        center: center_ptr,
        sides: [None; 4],
        corners: [None; 4],
        centered
    };

    for (dx, dy) in (-1..=1).cartesian_product(-1..=1) {
        match (dx, dy) {
            (0, 0) => continue,
            // UP and DOWN
            (0, -1) | (0, 1) => {
                let Some(chunk) = chunk_manager.get_chunk_data(&(position + ivec2(dx, dy)))
                else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                let start_ptr = chunk.pixels.as_ptr();

                chunk_group.sides[if dy == -1 { 0 } else { 3 }] = Some(start_ptr);
            }
            //LEFT and RIGHT
            (-1, 0) | (1, 0) => {
                let Some(chunk) = chunk_manager.get_chunk_data(&(position + ivec2(dx, dy)))
                else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                let start_ptr = chunk.pixels.as_ptr();

                chunk_group.sides[if dx == -1 { 1 } else { 2 }] = Some(start_ptr);
            }
            //CORNERS
            (-1, -1) | (1, -1) | (-1, 1) | (1, 1) => {
                let Some(chunk) = chunk_manager.get_chunk_data(&(position + ivec2(dx, dy)))
                else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                let start_ptr = chunk.pixels.as_ptr();

                let corner_idx = match (dx, dy) {
                    (1, 1) => 3,
                    (-1, 1) => 2,
                    (1, -1) => 1,
                    (-1, -1) => 0,

                    _ => unreachable!(),
                };

                chunk_group.corners[corner_idx] = Some(start_ptr);
            }

            _ => unreachable!(),
        }
    }

    Some(chunk_group)
}

impl<T: Clone> ChunkGroup<T> {
    fn chunk_offset_to_id(&self, chunk_offset: IVec2) -> i32 {
        (chunk_offset.y + self.centered as i32) * 3 + chunk_offset.x + self.centered as i32
    }

    pub fn get(&self, pixel_offset: IVec2) -> Option<&T> {
        let chunk_offset = pixel_offset.div_euclid(IVec2::ONE * self.size);

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }

        let id = self.chunk_offset_to_id(chunk_offset);

        let local_position = pixel_offset.rem_euclid(IVec2::ONE * self.size);
        let local_index = (local_position.y * self.size + local_position.x) as usize;

        match id {
            4 => unsafe { self.center.add(local_index).as_ref() },
            0 | 2 | 6 | 8 => {
                let mut corner_index = 0;

                if chunk_offset.x > 0 {
                    corner_index += 1;
                }

                if chunk_offset.y > 0 {
                    corner_index += 2;
                }

                self.corners[corner_index]
                    .as_ref()
                    .map(|chunk| unsafe { chunk.add(local_index).as_ref().unwrap() })
            }
            1 | 3 | 5 | 7 => {
                let mut side_index = 0;

                if chunk_offset.y > 0 {
                    side_index += 3;
                }

                if chunk_offset.x != 0 {
                    side_index = ((chunk_offset.x + 1).signum() + 1) as usize;
                }

                self.sides[side_index]
                    .as_ref()
                    .map(|chunk| unsafe { chunk.add(local_index).as_ref().unwrap() })
            }
            _ => unreachable!(),
        }
    }
}

pub struct ChunkGroupMut<T: Clone> {
    pub size: i32,
    pub center: *mut T,
    pub corners: [Option<*mut T>; 4],
    pub sides: [Option<*mut T>; 4],
    pub centered: bool,
}

pub fn build_chunk_group_mut(chunk_manager: &mut ChunkManager, position: IVec2, centered: bool) -> Option<ChunkGroupMut<Pixel>> {
    let center_ptr = if let Some(chunk) = chunk_manager.get_chunk_data_mut(&position) {
        chunk.pixels.as_mut_ptr()
    } else {
        return None;
    };

    let mut chunk_group = ChunkGroupMut {
        size: CHUNK_SIZE,
        center: center_ptr,
        sides: [None; 4],
        corners: [None; 4],
        centered
    };

    for (dx, dy) in (-1..=1).cartesian_product(-1..=1) {
        match (dx, dy) {
            (0, 0) => continue,
            // UP and DOWN
            (0, -1) | (0, 1) => {
                let Some(chunk) = chunk_manager.get_chunk_data_mut(&(position + ivec2(dx, dy)))
                else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                let start_ptr = chunk.pixels.as_mut_ptr();

                chunk_group.sides[if dy == -1 { 0 } else { 3 }] = Some(start_ptr);
            }
            //LEFT and RIGHT
            (-1, 0) | (1, 0) => {
                let Some(chunk) = chunk_manager.get_chunk_data_mut(&(position + ivec2(dx, dy)))
                else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                let start_ptr = chunk.pixels.as_mut_ptr();

                chunk_group.sides[if dx == -1 { 1 } else { 2 }] = Some(start_ptr);
            }
            //CORNERS
            (-1, -1) | (1, -1) | (-1, 1) | (1, 1) => {
                let Some(chunk) = chunk_manager.get_chunk_data_mut(&(position + ivec2(dx, dy)))
                else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                let start_ptr = chunk.pixels.as_mut_ptr();

                let corner_idx = match (dx, dy) {
                    (1, 1) => 3,
                    (-1, 1) => 2,
                    (1, -1) => 1,
                    (-1, -1) => 0,

                    _ => unreachable!(),
                };

                chunk_group.corners[corner_idx] = Some(start_ptr);
            }

            _ => unreachable!(),
        }
    }

    Some(chunk_group)
}

impl<T: Clone> ChunkGroupMut<T> {
    fn chunk_offset_to_id(&self, chunk_offset: IVec2) -> i32 {
        (chunk_offset.y + self.centered as i32) * 3 + chunk_offset.x + self.centered as i32
    }

    pub fn get(&self, pixel_offset: IVec2) -> Option<&T> {
        let chunk_offset = pixel_offset.div_euclid(IVec2::ONE * self.size);

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }

        let id = self.chunk_offset_to_id(chunk_offset);

        let local_position = pixel_offset.rem_euclid(IVec2::ONE * self.size);
        let local_index = (local_position.y * self.size + local_position.x) as usize;

        match id {
            4 => unsafe { self.center.add(local_index).as_ref() },
            0 | 2 | 6 | 8 => {
                let mut corner_index = 0;

                if chunk_offset.x > 0 {
                    corner_index += 1;
                }

                if chunk_offset.y > 0 {
                    corner_index += 2;
                }

                self.corners[corner_index]
                    .as_ref()
                    .map(|chunk| unsafe { chunk.add(local_index).as_ref().unwrap() })
            }
            1 | 3 | 5 | 7 => {
                let mut side_index = 0;

                if chunk_offset.y > 0 {
                    side_index += 3;
                }

                if chunk_offset.x != 0 {
                    side_index = ((chunk_offset.x + 1).signum() + 1) as usize;
                }

                self.sides[side_index]
                    .as_ref()
                    .map(|chunk| unsafe { chunk.add(local_index).as_ref().unwrap() })
            }
            _ => unreachable!(),
        }
    }

    pub fn set(&mut self, pixel_offset: IVec2, new_value: T) -> Result<(), String> {
        if let Some(value) = self.get_mut(pixel_offset) {
            *value = new_value;
            Ok(())
        }
        else {
            Err("out of bounds".to_string())
        }
    }

    pub fn get_mut(&mut self, pixel_offset: IVec2) -> Option<&mut T> {
        let chunk_offset = pixel_offset.div_euclid(IVec2::ONE * self.size);

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }

        let id = self.chunk_offset_to_id(chunk_offset);

        let local_position = pixel_offset.rem_euclid(IVec2::ONE * self.size);
        let local_index = (local_position.y * self.size + local_position.x) as usize;

        match id {
            4 => unsafe { self.center.add(local_index).as_mut() },
            0 | 2 | 6 | 8 => {
                let mut corner_index = 0;

                if chunk_offset.x > 0 {
                    corner_index += 1;
                }

                if chunk_offset.y > 0 {
                    corner_index += 2;
                }

                if let Some(chunk) = &mut self.corners[corner_index] {
                    Some(unsafe { chunk.add(local_index).as_mut().unwrap() })
                } else {
                    None
                }
            }
            1 | 3 | 5 | 7 => {
                let mut side_index = 0;

                if chunk_offset.y > 0 {
                    side_index += 3;
                }

                if chunk_offset.x != 0 {
                    side_index = ((chunk_offset.x + 1).signum() + 1) as usize;
                }

                if let Some(chunk) = &mut self.sides[side_index] {
                    Some(unsafe { chunk.add(local_index).as_mut().unwrap() })
                } else {
                    None
                }
            }
            _ => unreachable!(),
        }
    }
}

unsafe impl<T: Clone> Send for ChunkGroup<T> {}
unsafe impl<T: Clone> Send for ChunkGroupMut<T> {}

impl<T: Clone> std::ops::Index<IVec2> for ChunkGroup<T> {
    type Output = T;
    #[track_caller]
    fn index(&self, idx: IVec2) -> &Self::Output {
        self.get(idx).expect("Invalid index position.")
    }
}
impl<T: Clone> std::ops::Index<IVec2> for ChunkGroupMut<T> {
    type Output = T;
    #[track_caller]
    fn index(&self, idx: IVec2) -> &Self::Output {
        self.get(idx).expect("Invalid index position.")
    }
}
impl<T: Clone> std::ops::IndexMut<IVec2> for ChunkGroupMut<T> {
    #[track_caller]
    fn index_mut(&mut self, idx: IVec2) -> &mut Self::Output {
        self.get_mut(idx).expect("Invalid index position.")
    }
}

pub struct ChunkGroupCustom<T: Clone> {
    pub size: i32,
    pub chunks: HashMap<IVec2, *mut T>,
}

impl<T: Clone> ChunkGroupCustom<T> {
    pub fn get(&self, local_position: IVec2) -> Option<&T> {
        let chunk_position = local_position.div_euclid(IVec2::ONE * self.size);
        let pixel_position = local_position.rem_euclid(IVec2::ONE * self.size);

        let pixel_index = to_index!(pixel_position, self.size);

        self.chunks
            .get(&chunk_position)
            .as_ref()
            .map(|chunk| unsafe { chunk.add(pixel_index).as_ref().unwrap() })
    }

    pub fn get_mut(&mut self, local_position: IVec2) -> Option<&mut T> {
        let chunk_position = local_position.div_euclid(IVec2::ONE * self.size);
        let pixel_position = local_position.rem_euclid(IVec2::ONE * self.size);

        let pixel_index = to_index!(pixel_position, self.size);

        self.chunks
            .get(&chunk_position)
            .as_mut()
            .map(|chunk| unsafe { chunk.add(pixel_index).as_mut().unwrap() })
    }
}

unsafe impl<T: Clone> Send for ChunkGroupCustom<T> {}

impl<T: Clone> std::ops::Index<IVec2> for ChunkGroupCustom<T> {
    type Output = T;
    #[track_caller]
    fn index(&self, idx: IVec2) -> &Self::Output {
        self.get(idx).expect("Invalid index position.")
    }
}
impl<T: Clone> std::ops::IndexMut<IVec2> for ChunkGroupCustom<T> {
    #[track_caller]
    fn index_mut(&mut self, idx: IVec2) -> &mut Self::Output {
        self.get_mut(idx).expect("Invalid index position.")
    }
}
