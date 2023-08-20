use crate::constants::*;

pub fn is_between<T: PartialOrd>(value: T, min: T, max: T) -> bool {
    value >= min && value <= max
}

pub fn get_cell_index(x: i64, y: i64) -> usize {
    (y * CHUNK_SIZE + x) as usize
}