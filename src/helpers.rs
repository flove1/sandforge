use bevy_math::{IVec2, UVec2};

macro_rules! to_index {
    ($point:expr, $width:expr) => {
        ($point.y * $width + $point.x) as usize
    };
}

pub(crate) use to_index;

use crate::constants::CHUNK_SIZE;

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
