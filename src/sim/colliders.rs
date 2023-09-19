use std::cmp::Ordering;

use rapier2d::{prelude::{ColliderBuilder, Collider, Real, SharedShape}, na::{Point2, Isometry2}};

use crate::constants::{COLLIDER_PRECISION, PHYSICS_TO_WORLD};
       
/// Connected-component labeling
/// 
/// # Arguments
/// 
/// * `label` - A label with which to mark given objects. Must be greater than 1
/// * `condition` - A function that checks if given index in matrix is part of the object
pub fn label_matrix<F: Fn(usize) -> bool>(
    x: i32, 
    y: i32, 
    label: i32, 
    matrix: &mut [i32], 
    matrix_size: i32, 
    condition: &F
 ) {
    if x < 0 || x > matrix_size - 1 || y < 0 || y > matrix_size - 1 {
        return;
    }

    let index = (y * matrix_size + x) as usize;
    if matrix[index] != 0 || !condition(index) {
        return;
    }

    matrix[index] = label;

    label_matrix(x + 1, y, label, matrix, matrix_size, condition);
    label_matrix(x - 1, y, label, matrix, matrix_size, condition);
    label_matrix(x, y + 1, label, matrix, matrix_size, condition);
    label_matrix(x, y - 1, label, matrix, matrix_size, condition);
}

fn classify_cell(
    x: i32, 
    y: i32, 
    matrix: &[i32], 
    matrix_size: i32
) -> u8 {
    let mut mask = 0;

    if is_set(x - 1, y - 1, matrix, matrix_size) {
        mask |= 1; 
    }
    if is_set(x, y - 1, matrix, matrix_size) {
        mask |= 2; 
    }
    if is_set(x - 1, y, matrix, matrix_size) {
        mask |= 4; 
    }
    if is_set(x, y, matrix, matrix_size) {
        mask |= 8; 
    }

    mask
}

fn is_set(
    x: i32, 
    y: i32, 
    matrix: &[i32], 
    matrix_size: i32
) -> bool {
    if x < 0 || x > matrix_size - 1 || y < 0 || y > matrix_size - 1 {
        false
    }
    else {
        matrix[(y * matrix_size + x) as usize] != 0
    }
}

pub fn marching_squares(object_count: i32, matrix: &[i32], matrix_size: i32, size_modifier: f32) -> Vec<Vec<Vec<(f32, f32)>>> {
    let mut visited = vec![false; matrix_size.pow(2) as usize];

    let mut objects = vec![vec![vec![]]; object_count as usize];
    for x in 0..matrix_size {
        for y in 0..matrix_size {
            let index = (y * matrix_size + x) as usize;
            let mask = classify_cell(x, y, matrix, matrix_size);
            if matrix[index] > 0 && !visited[index] && mask != 0 && mask != 15 {
                let object_label = matrix[index];

                let mut vertices: Vec<(f32, f32)> = vec![];

                let (mut current_x, mut current_y) = (x, y);
                let (mut dx, mut dy) = (0, 0);

                'march: loop {
                    if current_x >= 0 && current_x < matrix_size && current_y >= 0 && current_y < matrix_size {
                        visited[(current_y * matrix_size + current_x) as usize] = true;
                    }
                    match classify_cell(current_x, current_y, matrix, matrix_size) {
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

                    vertices.push((
                        (current_x as f32 + (dx as f32 / 2.0) - (matrix_size as f32 / 2.0)) / PHYSICS_TO_WORLD * size_modifier,  
                        (current_y as f32 + (dy as f32 / 2.0) - (matrix_size as f32 / 2.0)) / PHYSICS_TO_WORLD * size_modifier
                    ));

                    current_x += dx;
                    current_y += dy;
                }

                objects[object_label as usize - 1].push(vertices);
            }
        }
    }

    objects
}

fn create_simplified_outlines(object_count: i32, matrix: &[i32], matrix_size: i32, size_modifier: f32) -> Vec<Vec<(f32, f32)>> {
    marching_squares(object_count, matrix, matrix_size, size_modifier)
        .iter()
        .map(|object| {
            object.iter()
                .filter(|boundary| boundary.len() >= 3)
                .max_by(|boundary_1, boundary_2| {
                    if boundary_1.len() > boundary_2.len() {
                        Ordering::Greater
                    }
                    else {
                        Ordering::Less
                    }
                }
            )
        })
        .filter_map(|boundary| {
            match boundary {
                Some(boundary) => {
                    Some(douglas_peucker(boundary, COLLIDER_PRECISION / (matrix_size.pow(2)) as f32))
                },
                None => None,
            }

        })
        .filter(|simplified_boundary| {
            simplified_boundary.len() >= 3
        })
        .collect::<Vec<Vec<(f32, f32)>>>()
}

pub fn create_polyline_colliders(object_count: i32, matrix: &[i32], matrix_size: i32) -> Vec<(Collider, (f32, f32))> {
    create_simplified_outlines(object_count, matrix, matrix_size, 1.0).iter()
        .map(|simplified_boundary| {
            let indices = 
                (0..simplified_boundary.len() as u32)
                .zip((1..simplified_boundary.len() as u32 - 1).chain(0..=9))
                .map(|(i1, i2)| {
                    [i1, i2]
                })
                .collect::<Vec<[u32; 2]>>();

            ColliderBuilder::polyline(
                simplified_boundary.iter()
                    .map(|point| {
                        Point2::new(point.0 as f32, point.1 as f32)
                    }).collect(),
                    Some(indices)
                // Some((0..simplified_boundary.len()).chain(0..=0).collect::<Vec<usize>>()),
            ).build()
        })
        .map(|mut collider| {
            let vertices = collider.shape().as_polyline().unwrap().vertices();
            let count = vertices.len();

            let center = vertices.iter() 
                .map(|center| {
                    (center.x / count as f32, center.y / count as f32)
                })
                .fold((0.0, 0.0), |sum, value| {
                    ((sum.0 + value.0), (sum.1 + value.1))
                });
            collider.set_restitution(0.1);
            collider.set_friction(0.0);

            (collider, center)
        })
        .collect::<Vec<(Collider, (f32, f32))>>()
}

pub fn create_triangulated_collider(matrix: &[i32], matrix_size: i32) -> (Collider, (f32, f32)) {
    create_simplified_outlines(1, matrix, matrix_size, 1.0).iter()
        .map(|simplified_boundary| {
            simplified_boundary.iter()
                .map(|point| {
                    (point.0 as f64, point.1 as f64)
                })
                .collect::<Vec<(f64, f64)>>()
        })
        .map(|simplified_boundary| {
            let triangulatio_result = 
                cdt::triangulate_contours(
                    &simplified_boundary, 
                    &[(0..simplified_boundary.len()).chain(0..=0).collect::<Vec<usize>>()]
                );

            (simplified_boundary, triangulatio_result)
        })
        
        .map(|(simplified_boundary, triangulatio_result)| {
            (
                simplified_boundary.iter()
                    .map(|pos| (pos.0 as f32, pos.1 as f32))
                    .collect::<Vec<(f32, f32)>>(), 
                triangulatio_result
            )
        })
        .filter(|(_, triangulation_result)| { 
            if triangulation_result.is_err() {
                println!("{}", triangulation_result.clone().as_deref().unwrap_err());
                return false;
            }
            triangulation_result.is_ok()
        })
        .map(|(simplified_boundary, triangulation_result)| {
            ColliderBuilder::compound(
                triangulation_result.unwrap().iter()
                    .map(|i| {
                        (
                            Isometry2::default(),
                            SharedShape::triangle(
                                Point2::new(simplified_boundary[i.0].0, simplified_boundary[i.0].1),
                                Point2::new(simplified_boundary[i.1].0, simplified_boundary[i.1].1),
                                Point2::new(simplified_boundary[i.2].0, simplified_boundary[i.2].1),
                            )
                        )
                    })
                    .collect::<Vec<(Isometry2<Real>, SharedShape)>>()
            ).build()
        })
        .map(|mut collider| {
            let shapes = collider.shape().as_compound().unwrap().shapes();
            let count = shapes.len();

            let center = shapes.iter() 
                .map(|shape| {
                    shape.1.as_triangle().unwrap().center()
                })
                .map(|center| {
                    (center.x / count as f32, center.y / count as f32)
                })
                .fold((0.0, 0.0), |sum, value| {
                    ((sum.0 + value.0), (sum.1 + value.1))
                });

            collider.set_density(10.0);
            collider.set_restitution(0.1);

            (collider, center)
        })
        .next().unwrap()
}

fn perpendicular_squared_distance(point: (f32, f32), line: ((f32, f32), (f32, f32))) -> f32 {
    let x_diff = line.1.0 - line.0.0;
    let y_diff = line.1.1 - line.0.1;
    let numerator =
        (y_diff * point.0 - x_diff * point.1 + line.1.0 * line.0.1 - line.1.1 * line.0.0).abs();
    let numerator_squared = numerator * numerator;
    let denominator_squared = y_diff * y_diff + x_diff * x_diff;
    numerator_squared / denominator_squared
}

fn douglas_peucker(vertices: &[(f32, f32)], epsilon: f32) -> Vec<(f32, f32)> {
    let mut d_squared_max = 0.0;
    let mut farthest_point_index = 0;
    let end = vertices.len() - 1;
    if end < 3 {
        return vertices.to_vec();
    }
    let line = (vertices[0], vertices[end - 1]);
    for (i, _) in vertices.iter().enumerate().take(end - 1).skip(1) {
        let d_squared = perpendicular_squared_distance(vertices[i], line);
        if d_squared > d_squared_max {
            farthest_point_index = i;
            d_squared_max = d_squared;
        }
    }

    if d_squared_max > epsilon {
        let rec_results1 =
            douglas_peucker(&vertices[0..farthest_point_index], epsilon);
        let rec_results2 =
            douglas_peucker(&vertices[farthest_point_index..(end + 1)], epsilon);

        [rec_results1, rec_results2[1..rec_results2.len()].to_vec()].concat()
    } else {
        vec![vertices[0], vertices[end]]
    }
}
