use crate::helpers::to_index;
use bevy::{prelude::*, utils::HashMap};

// 6 7 8
// 3 4 5
// 0 1 2
pub struct ChunkGroup3x3<T: Clone> {
    pub size: i32,
    pub center: *mut T,
    pub corners: [Option<*mut T>; 4],
    pub sides: [Option<*mut T>; 4],
}

impl<T: Clone> ChunkGroup3x3<T> {
    fn chunk_offset_to_id(&self, chunk_offset: IVec2) -> i32 {
        (chunk_offset.y + 1) * 3 + chunk_offset.x + 1
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

unsafe impl<T: Clone> Send for ChunkGroup3x3<T> {}

impl<T: Clone> std::ops::Index<IVec2> for ChunkGroup3x3<T> {
    type Output = T;
    #[track_caller]
    fn index(&self, idx: IVec2) -> &Self::Output {
        self.get(idx).expect("Invalid index position.")
    }
}
impl<T: Clone> std::ops::IndexMut<IVec2> for ChunkGroup3x3<T> {
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
