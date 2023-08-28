use crate::constants::CHUNK_SIZE;

use super::helpers::get_cell_index;

pub fn classify_cell(x: i64, y: i64, matrix: &mut Vec<i32>, _size: i64) -> u8 {
    let mut mask = 0;

    if is_set(x - 1, y - 1, matrix) {
        mask |= 1; 
    }
    if is_set(x, y - 1, matrix) {
        mask |= 2; 
    }
    if is_set(x - 1, y, matrix) {
        mask |= 4; 
    }
    if is_set(x, y, matrix) {
        mask |= 8; 
    }

    mask
}

pub fn is_set(x: i64, y: i64, matrix: &mut Vec<i32>) -> bool {
    if x <= 0 || x > CHUNK_SIZE - 1 || y <= 0 || y > CHUNK_SIZE - 1 {
        false
    }
    else {
        matrix[get_cell_index(x, y)] > 0
    }
}


pub fn marching_squares(object_count: i32, matrix: &mut Vec<i32>, size: i64) -> Vec<Vec<Vec<(i64, i64)>>> {
    let mut boundaries: Vec<Vec<Vec<(f64, f64)>>> = vec![vec![vec![]]; object_count as usize];
    for x in 0..size {
        for y in 0..size {
            let index = get_cell_index(x, y);
            if matrix[index] > 0 && matrix[index] < 15 && classify_cell(x, y, matrix, size) > 0 {
                let object = matrix[index];

                let mut vertices: Vec<(f64, f64)> = vec![];

                let (mut current_x, mut current_y) = (x, y);
                let (mut dx, mut dy) = (0, 0);

                let mut directions: Vec<(i64, i64)> = vec![];

                'march: loop {
                    match classify_cell(current_x, current_y, matrix, size) {
                        1 | 5 | 13 => { dx = 0; dy = -1;  },
                        2 | 3 | 7 => { dx = 1; dy = 0 },
                        8 | 10 | 11 => { dx = 0; dy = 1 },
                        4 | 12 | 14 => { dx = -1; dy = 0 },
                        6 => { if dx == 0 && dy == -1 { dx = 1; dy = 0 } else { dx = -1; dy = 0 } },
                        9 => { if dx == 1 && dy == 0 { dx = 0; dy = -1 } else { dx = 0; dy = 1 } },

                        _ => panic!()
                    }

                    directions.push((dx, dy));
                    vertices.push((current_x as f64 + dx as f64 / 2.0,  current_y as f64 + dy as f64 / 2.0));

                    current_x += dx;
                    current_y += dy; 

                    if x == current_x && y == current_y {
                        break 'march;
                    }
                }

                // boundaries[matrix[index] as usize - 1].append(&mut vertices);
                (current_x, current_y) = (x, y);
                for (dx, dy) in directions {
                    if current_x > 0 && current_x < CHUNK_SIZE && current_y > 0 && current_y < CHUNK_SIZE {
                        matrix[get_cell_index(current_x, current_y)] = -1;
                    }
                    current_x += dx;
                    current_y += dy;
                }

                boundaries[object as usize - 1].push(vertices);
            }
        }
    }

    dbg!(boundaries);

    vec![vec![vec![]]]
}