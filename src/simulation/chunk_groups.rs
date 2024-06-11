use crate::{ constants::CHUNK_SIZE, helpers::to_index };
use bevy::{ prelude::*, utils::HashMap };
use bevy_math::ivec2;
use itertools::Itertools;

use super::{ chunk::ChunkState, chunk_manager::ChunkManager, pixel::Pixel };

// 6 7 8
// 3 4 5
// 0 1 2
pub struct ChunkGroup<T: Clone> {
    size: i32,
    center: *mut T,
    corners: [Option<*mut T>; 4],
    sides: [Option<*mut T>; 4],
    texture: Option<TextureAccess>,
}

struct TextureAccess {
    pub center: *mut u8,
    pub corners: [Option<*mut u8>; 4],
    pub sides: [Option<*mut u8>; 4],
}

pub fn build_chunk_group_with_texture_access(
    chunk_manager: &mut ChunkManager,
    chunk_position: IVec2,
    images: &mut Assets<Image>
) -> Option<ChunkGroup<Pixel>> {
    chunk_group_helper(chunk_manager, chunk_position, Some(images))
}

pub fn build_chunk_group(
    chunk_manager: &mut ChunkManager,
    chunk_position: IVec2
) -> Option<ChunkGroup<Pixel>> {
    chunk_group_helper(chunk_manager, chunk_position, None)
}

fn chunk_group_helper(
    chunk_manager: &mut ChunkManager,
    chunk_position: IVec2,
    mut images: Option<&mut Assets<Image>>
) -> Option<ChunkGroup<Pixel>> {
    let Some(center_chunk) = chunk_manager.get_chunk_data_mut(&chunk_position) else {
        return None;
    };

    if !matches!(center_chunk.state, ChunkState::Populating | ChunkState::Active | ChunkState::Sleeping) {
        return None;
    }

    let mut chunk_group = ChunkGroup {
        size: CHUNK_SIZE,
        texture: if images.is_some() {
            Some(TextureAccess {
                center:
                    images
                        .as_mut()
                        .unwrap()
                        .get_mut(center_chunk.background.clone_weak())
                        .unwrap()
                        .data.as_mut_ptr()
                ,
                corners: [None; 4],
                sides: [None; 4],
            })
        } else {
            None
        },
        center: center_chunk.pixels.as_mut_ptr(),
        corners: [None; 4],
        sides: [None; 4],
    };

    for (dx, dy) in (-1..=1).cartesian_product(-1..=1) {
        match (dx, dy) {
            (0, 0) => {
                continue;
            }
            // UP and DOWN
            (0, -1) | (0, 1) => {
                let Some(chunk) = chunk_manager.get_chunk_data_mut(
                    &(chunk_position + ivec2(dx, dy))
                ) else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Populating | ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                if let Some(textures) = &mut chunk_group.texture {
                    textures.sides[if dy == -1 { 0 } else { 3 }] = Some(
                        images
                            .as_mut()
                            .unwrap()
                            .get_mut(chunk.background.clone_weak())
                            .unwrap()
                            .data.as_mut_ptr()
                    );
                }

                chunk_group.sides[if dy == -1 { 0 } else { 3 }] = Some(chunk.pixels.as_mut_ptr());
            }
            //LEFT and RIGHT
            (-1, 0) | (1, 0) => {
                let Some(chunk) = chunk_manager.get_chunk_data_mut(
                    &(chunk_position + ivec2(dx, dy))
                ) else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Populating | ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                if let Some(textures) = &mut chunk_group.texture {
                    textures.sides[if dx == -1 { 1 } else { 2 }] = Some(
                        images
                            .as_mut()
                            .unwrap()
                            .get_mut(chunk.background.clone_weak())
                            .unwrap()
                            .data.as_mut_ptr()
                    );
                }

                chunk_group.sides[if dx == -1 { 1 } else { 2 }] = Some(chunk.pixels.as_mut_ptr());
            }
            //CORNERS
            (-1, -1) | (1, -1) | (-1, 1) | (1, 1) => {
                let Some(chunk) = chunk_manager.get_chunk_data_mut(
                    &(chunk_position + ivec2(dx, dy))
                ) else {
                    continue;
                };

                if !matches!(chunk.state, ChunkState::Populating | ChunkState::Active | ChunkState::Sleeping) {
                    continue;
                }

                let corner_idx = match (dx, dy) {
                    (1, 1) => 3,
                    (-1, 1) => 2,
                    (1, -1) => 1,
                    (-1, -1) => 0,

                    _ => unreachable!(),
                };
                
                if let Some(textures) = &mut chunk_group.texture {
                    textures.corners[corner_idx] = Some(
                        images
                            .as_mut()
                            .unwrap()
                            .get_mut(chunk.background.clone_weak())
                            .unwrap()
                            .data.as_mut_ptr()
                    );
                }

                chunk_group.corners[corner_idx] = Some(chunk.pixels.as_mut_ptr());
            }

            _ => unreachable!(),
        }
    }

    Some(chunk_group)
}

impl<T: Clone> ChunkGroup<T> {
    pub fn chunk_offset_to_id(&self, chunk_offset: IVec2) -> i32 {
        (chunk_offset.y + 1) * 3 + chunk_offset.x + 1
    }

    pub fn get(&self, pixel_offset: IVec2) -> Option<&T> {
        let chunk_offset = pixel_offset.div_euclid(IVec2::splat(self.size));

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }

        let id = self.chunk_offset_to_id(chunk_offset);

        let local_position = pixel_offset.rem_euclid(IVec2::splat(self.size));
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

                if let Some(pointer) = self.corners[corner_index] {
                    unsafe { pointer.add(local_index).as_ref() }
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

                if let Some(pointer) = self.sides[side_index] {
                    unsafe { pointer.add(local_index).as_ref() }
                } else {
                    None
                }
            }
            _ => unreachable!(),
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
            4 => unsafe { self.center.add(local_index).as_mut() }
            0 | 2 | 6 | 8 => {
                let mut corner_index = 0;

                if chunk_offset.x > 0 {
                    corner_index += 1;
                }

                if chunk_offset.y > 0 {
                    corner_index += 2;
                }

                if let Some(pointer) = self.corners[corner_index] {
                    unsafe { pointer.add(local_index).as_mut() }
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

                if let Some(pointer) = self.sides[side_index] {
                    unsafe { pointer.add(local_index).as_mut() }
                } else {
                    None
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn set(&mut self, pixel_offset: IVec2, new_value: T) -> Result<(), String> {
        if let Some(value) = self.get_mut(pixel_offset) {
            *value = new_value;
            Ok(())
        } else {
            Err("out of bounds".to_string())
        }
    }

    pub fn background_get_mut(&mut self, pixel_offset: IVec2) -> Option<&mut [u8]> {
        if self.texture.is_none() {
            return None;
        }

        let chunk_offset = pixel_offset.div_euclid(IVec2::ONE * self.size);

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }

        let id = self.chunk_offset_to_id(chunk_offset);

        let local_position = pixel_offset.rem_euclid(IVec2::ONE * self.size);
        let local_index = (local_position.y * self.size + local_position.x) as usize;

        match id {
            4 => unsafe {
                let pointer = self.texture.as_mut().unwrap_unchecked().center;
                Some(std::slice::from_raw_parts_mut(pointer.add(local_index * 4), 4))
            }
            0 | 2 | 6 | 8 => {
                let mut corner_index = 0;

                if chunk_offset.x > 0 {
                    corner_index += 1;
                }

                if chunk_offset.y > 0 {
                    corner_index += 2;
                }

                if let Some(texture) = &mut self.texture {
                    if let Some(pointer) = texture.corners[corner_index] {
                        Some(unsafe { std::slice::from_raw_parts_mut(pointer.add(local_index * 4), 4) })
                    } else {
                        None
                    }
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

                if let Some(texture) = &mut self.texture {
                    if let Some(pointer) = texture.sides[side_index] {
                        Some(unsafe { std::slice::from_raw_parts_mut(pointer.add(local_index * 4), 4) })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn background_set(&mut self, pixel_offset: IVec2, color: [u8; 4]) -> Result<(), String> {
        if let Some(pixel_color) = self.background_get_mut(pixel_offset) {
            pixel_color.copy_from_slice(&color);
            Ok(())
        } else {
            Err("out of bounds".to_string())
        }
    }
}

unsafe impl<T: Clone> Send for ChunkGroup<T> {}

impl<T: Clone> std::ops::Index<IVec2> for ChunkGroup<T> {
    type Output = T;
    #[track_caller]
    fn index(&self, idx: IVec2) -> &Self::Output {
        self.get(idx).expect("Invalid index position.")
    }
}
impl<T: Clone> std::ops::IndexMut<IVec2> for ChunkGroup<T> {
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
