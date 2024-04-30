use bevy_math::{ivec2, IVec2, UVec2, Vec2};

macro_rules! to_index {
    ($point:expr, $width:expr) => {
        ($point.y * $width + $point.x) as usize
    };
}

pub(crate) use to_index;

use crate::constants::CHUNK_SIZE;

pub struct WalkGrid {
    point: IVec2,
    current: Vec2,
    signs: IVec2,
    absolute_delta: Vec2,
}

impl WalkGrid {
    #[inline]
    pub fn new(start: IVec2, end: IVec2) -> WalkGrid {
        let delta = end - start;

        WalkGrid {
            point: start,
            current: Vec2::ZERO,
            signs: delta.signum(),
            absolute_delta: delta.abs().as_vec2()
        }
    }
}

impl Iterator for WalkGrid {
    type Item = IVec2;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current.cmple(self.absolute_delta).all() {
            let point = self.point;

            if (0.5 + self.current.x) / self.absolute_delta.x < (0.5 + self.current.y) / self.absolute_delta.y {
                self.point.x += self.signs.x;
                self.current.x += 1.0;
            } else {
                self.point.y += self.signs.y;
                self.current.y += 1.0;
            }

            Some(point)
        } else {
            None
        }
    }
}

/// * `operation` - a function that is called at each point in a line and returns a bool indicating whether the function should continue
/// 
/// Returns `true` if function wasn't finished due to `operation` condition
pub fn line_from_pixels<F: FnMut(i32, i32) -> bool>(
    point_1: IVec2,
    point_2: IVec2,
    operation: &mut F
) -> bool {
    let dx:i32 = i32::abs(point_2.x - point_1.x);
    let dy:i32 = i32::abs(point_2.y - point_1.y);
    let sx:i32 = { if point_1.x < point_2.x { 1 } else { -1 } };
    let sy:i32 = { if point_1.y < point_2.y { 1 } else { -1 } };

    let mut error:i32 = (if dx > dy  { dx } else { -dy }) / 2 ;
    let mut current_x:i32 = point_1.x;
    let mut current_y:i32 = point_1.y;

    loop {
        if !operation(current_x, current_y) {
            return true;
        };

        if current_x == point_2.x && current_y == point_2.y { break; }
        let error2:i32 = error;

        if error2 > -dx {
            error -= dy;
            current_x += sx;
        }
        if error2 < dy {
            error += dx;
            current_y += sy;
        }
    }   

    false
}

pub fn global_to_local(position: IVec2) -> (IVec2, UVec2) {
    let chunk_size = IVec2::ONE * CHUNK_SIZE;
    let chunk_position = position.div_euclid(chunk_size);
    let cell_position = position.rem_euclid(chunk_size).as_uvec2();

    (chunk_position, cell_position)
}

pub fn local_to_global(chunk_position: IVec2, cell_position: UVec2) -> IVec2 {
    chunk_position * CHUNK_SIZE + cell_position.as_ivec2()
}

pub fn modify_local_position(
    chunk_position: IVec2, 
    cell_position: UVec2,
    change: IVec2
) -> (IVec2, UVec2) {
    let mut global_position = local_to_global(chunk_position, cell_position);
    global_position += change;

    global_to_local(global_position)
}
