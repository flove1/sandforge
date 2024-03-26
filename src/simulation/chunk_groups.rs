use crate::constants::CHUNK_SIZE;
use bevy::prelude::*;

use super::pixel::Pixel;

// 6 7 8
// 3 4 5
// 0 1 2
pub struct ChunkGroup3x3 {
    pub center: Option<*mut Pixel>,
    pub corners: [Option<*mut Pixel>; 4],
    pub sides: [Option<*mut Pixel>; 4],
}

impl ChunkGroup3x3 {
    fn chunk_offset_to_id(&self, chunk_offset: IVec2) -> i32 {
        (chunk_offset.y + 1) * 3 + chunk_offset.x + 1
    }

    pub fn get(&self, pixel_offset: IVec2) -> Option<&Pixel> {
        let chunk_offset = pixel_offset.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }

        let id = self.chunk_offset_to_id(chunk_offset);

        let local_position = pixel_offset.rem_euclid(IVec2::ONE * CHUNK_SIZE);
        let local_index = (local_position.y * CHUNK_SIZE + local_position.x) as usize;

        match id {
            4 => Some(unsafe {
                self.center
                    .as_ref()
                    .unwrap()
                    .add(local_index)
                    .as_ref()
                    .unwrap()
            }),
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

    pub fn get_mut(&mut self, pixel_offset: IVec2) -> Option<&mut Pixel> {
        let chunk_offset = pixel_offset.div_euclid(IVec2::ONE * CHUNK_SIZE);

        if chunk_offset.min_element() < -1 || chunk_offset.max_element() > 1 {
            return None;
        }

        let id = self.chunk_offset_to_id(chunk_offset);

        let local_position = pixel_offset.rem_euclid(IVec2::ONE * CHUNK_SIZE);
        let local_index = (local_position.y * CHUNK_SIZE + local_position.x) as usize;

        match id {
            4 => Some(unsafe {
                self.center
                    .as_mut()
                    .unwrap()
                    .add(local_index)
                    .as_mut()
                    .unwrap()
            }),
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

unsafe impl Send for ChunkGroup3x3 {}

impl std::ops::Index<IVec2> for ChunkGroup3x3 {
    type Output = Pixel;
    #[track_caller]
    fn index(&self, idx: IVec2) -> &Self::Output {
        self.get(idx).expect("Invalid index position.")
    }
}
impl std::ops::IndexMut<IVec2> for ChunkGroup3x3 {
    #[track_caller]
    fn index_mut(&mut self, idx: IVec2) -> &mut Self::Output {
        self.get_mut(idx).expect("Invalid index position.")
    }
}

pub struct ChunkGroupCustom {
    pub chunks: Vec<Option<*mut Pixel>>,
    pub size: u8,
    pub position: IVec2,
}

impl ChunkGroupCustom {
    pub fn get(&self, global_position: IVec2) -> Option<&Pixel> {
        let chunk_position = global_position.div_euclid(IVec2::ONE * CHUNK_SIZE) - self.position;
        let pixel_position = global_position.rem_euclid(IVec2::ONE * CHUNK_SIZE);

        let chunk_index = (chunk_position.y * self.size as i32 + chunk_position.x) as usize;
        let pixel_index = (pixel_position.y * CHUNK_SIZE + pixel_position.x) as usize;

        let Some(chunk) = self.chunks.get(chunk_index) else {
            return None;
        };

        chunk
            .as_ref()
            .map(|chunk| unsafe { chunk.add(pixel_index).as_ref().unwrap() })
    }

    pub fn get_mut(&mut self, global_position: IVec2) -> Option<&mut Pixel> {
        let chunk_position = global_position.div_euclid(IVec2::ONE * CHUNK_SIZE) - self.position;
        let pixel_position = global_position.rem_euclid(IVec2::ONE * CHUNK_SIZE);

        let chunk_index = (chunk_position.y * self.size as i32 + chunk_position.x) as usize;
        let pixel_index = (pixel_position.y * CHUNK_SIZE + pixel_position.x) as usize;

        let Some(chunk) = self.chunks.get_mut(chunk_index) else {
            return None;
        };

        chunk
            .as_mut()
            .map(|chunk| unsafe { chunk.add(pixel_index).as_mut().unwrap() })
    }
}

unsafe impl Send for ChunkGroupCustom {}

impl std::ops::Index<IVec2> for ChunkGroupCustom {
    type Output = Pixel;
    #[track_caller]
    fn index(&self, idx: IVec2) -> &Self::Output {
        self.get(idx).expect("Invalid index position.")
    }
}
impl std::ops::IndexMut<IVec2> for ChunkGroupCustom {
    #[track_caller]
    fn index_mut(&mut self, idx: IVec2) -> &mut Self::Output {
        self.get_mut(idx).expect("Invalid index position.")
    }
}
