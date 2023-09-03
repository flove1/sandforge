use std::cmp::Ordering;

use crate::constants::CHUNK_SIZE;

use super::helpers::get_cell_index;

fn classify_cell(x: i64, y: i64, matrix: &mut [i32], _size: i64) -> u8 {
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

fn is_set(x: i64, y: i64, matrix: &mut [i32]) -> bool {
    if x < 0 || x > CHUNK_SIZE - 1 || y < 0 || y > CHUNK_SIZE - 1 {
        false
    }
    else {
        matrix[get_cell_index(x, y)] != 0
    }
}


pub fn marching_squares(object_count: i32, matrix: &mut [i32], size: i64) -> Vec<Vec<(f64, f64)>> {
    let mut visited = vec![false; size.pow(2) as usize];

    let mut objects: Vec<Vec<Vec<(f64, f64)>>> = vec![vec![vec![]]; object_count as usize];
    for x in 0..size {
        for y in 0..size {
            let index = get_cell_index(x, y);
            if matrix[index] > 0 && !visited[index] {
                {
                    let mask = classify_cell(x, y, matrix, size);
                    if mask == 0 || mask == 15 {
                        continue;
                    }
                }
                let object_label = matrix[index];

                let mut vertices: Vec<(f64, f64)> = vec![];

                let (mut current_x, mut current_y) = (x, y);
                let (mut dx, mut dy) = (0, 0);

                'march: loop {
                    if current_x > 0 && current_x < CHUNK_SIZE && current_y > 0 && current_y < CHUNK_SIZE {
                        visited[get_cell_index(current_x, current_y)] = true;
                    }
                    match classify_cell(current_x, current_y, matrix, size) {
                        1 | 5 | 13 => { dx = 0; dy = -1;  },
                        2 | 3 | 7 => { dx = 1; dy = 0 },
                        8 | 10 | 11 => { dx = 0; dy = 1 },
                        4 | 12 | 14 => { dx = -1; dy = 0 },
                        6 => { if dx == 0 && dy == -1 { dx = 1; dy = 0 } else { dx = -1; dy = 0 } },
                        9 => { if dx == 1 && dy == 0 { dx = 0; dy = 1 } else { dx = 0; dy = -1 } },

                        _ => panic!()
                    }

                    if x == current_x + dx && y == current_y + dy {
                        break 'march;
                    } 

                    vertices.push((current_x as f64 + dx as f64 / 2.0,  current_y as f64 + dy as f64 / 2.0));

                    current_x += dx;
                    current_y += dy;
                }

                objects[object_label as usize - 1].push(vertices);
            }
        }
    }

    let mut triangles = vec![];

    for object in objects {
        let mut boundaries: Vec<&Vec<(f64, f64)>> = object.iter().filter(|boundary| boundary.len() > 0).collect();

        boundaries.sort_by(|boundary_1, boundary_2| {
            if boundary_1.len() > boundary_2.len() {
                Ordering::Less
            }
            else {
                Ordering::Greater
            }
        });

        let mut points = vec![];
        let mut contours: Vec<Vec<usize>> = vec![];
        let mut offset = 0;

        for boundary in boundaries {
            let mut simplified_boundary = douglas_peucker(&boundary);
            points.append(&mut simplified_boundary);
            contours.push((offset..simplified_boundary.len()).chain(offset..=offset).collect());
            offset += simplified_boundary.len();
        }
        
        if !points.is_empty() {            
            let triangle_indeces = cdt::triangulate_contours(&points, &contours).unwrap();

            for triangle in triangle_indeces {
                triangles.push(vec![points[triangle.0], points[triangle.1], points[triangle.2]]);
            }
            // for boundary in boundaries.iter() {
            //     let simplified_boundary = douglas_peucker(&boundary);
            //     triangles.push(simplified_boundary);
            // }
        }
    }

    triangles
}

fn distance_between_points(p1: &(f64, f64), p2: &(f64, f64)) -> f64 {
    ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt()
}

fn distance_to_line(point: &(f64, f64), line_start: &(f64, f64), line_end: &(f64, f64)) -> f64 {
    let line_length = distance_between_points(line_start, line_end);
    let numerator = ((line_end.1 - line_start.1) * point.0 - (line_end.0 - line_start.0) * point.1 + line_end.0 * line_start.1 - line_end.1 * line_start.0).abs();
    numerator / line_length
}

fn douglas_peucker(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut dmax = 0.0;
    let mut index = 0;

    for i in 1..(points.len() - 1) {
        let d = distance_to_line(&points[i], &points[0], &points[points.len() - 1]);
        if d > dmax {
            index = i;
            dmax = d;
        }
    }

    let mut result = Vec::new();
    if dmax >= 1.0 {
        let mut result_1 = douglas_peucker(&points[..=index]);
        let mut result_2 = douglas_peucker(&points[index..]);
        result.append(&mut result_1);
        result.append(&mut result_2);
    } else {
        result.push(points[0].clone());
        result.push(points[points.len() - 1].clone());
    }

    result
}
