use std::cmp::Ordering;

use poly2tri_rs::{Point, SweeperBuilder};
use rapier2d::{prelude::{ColliderBuilder, Collider, Real, SharedShape}, na::{Point2, Isometry2}};

use crate::constants::{COLLIDER_PRECISION, PHYSICS_TO_WORLD, PHYSICS_SCALE, CHUNK_SIZE};
       
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
    width: i32,
    height: i32,
    condition: &F
 ) {
    if x < 0 || x > width - 1 || y < 0 || y > height - 1 {
        return;
    }

    let index = (y * width + x) as usize;
    if matrix[index] != 0 || !condition(index) {
        return;
    }

    matrix[index] = label;

    label_matrix(x + 1, y, label, matrix, width, height, condition);
    label_matrix(x - 1, y, label, matrix, width, height, condition);
    label_matrix(x, y + 1, label, matrix, width, height, condition);
    label_matrix(x, y - 1, label, matrix, width, height, condition);
}

fn classify_cell(
    x: i32, 
    y: i32, 
    matrix: &[i32], 
    width: i32,
    height: i32
) -> u8 {
    let mut mask = 0;

    if is_set(x - 1, y - 1, matrix, width, height) {
        mask |= 1; 
    }
    if is_set(x, y - 1, matrix, width, height) {
        mask |= 2; 
    }
    if is_set(x - 1, y, matrix, width, height) {
        mask |= 4; 
    }
    if is_set(x, y, matrix, width, height) {
        mask |= 8; 
    }

    mask
}

fn is_set(
    x: i32, 
    y: i32, 
    matrix: &[i32],
    width: i32,
    height: i32
) -> bool {
    if x < 0 || x > width - 1 || y < 0 || y > height - 1 {
        false
    }
    else {
        matrix[(y * width + x) as usize] != 0
    }
}

pub fn marching_squares(object_count: i32, matrix: &[i32], width: i32, height: i32, size_modifier: f32) -> Vec<Vec<Vec<(f32, f32)>>> {
    let mut visited = vec![false; (width * height) as usize];

    let mut objects = vec![vec![vec![]]; object_count as usize];
    for x in 0..width {
        for y in 0..height {
            let index = (y * width + x) as usize;
            let mask = classify_cell(x, y, matrix, width, height);
            if matrix[index] > 0 && !visited[index] && mask != 0 && mask != 15 {
                let object_label = matrix[index];

                let mut vertices: Vec<(f32, f32)> = vec![];

                let (mut current_x, mut current_y) = (x, y);
                let (mut dx, mut dy) = (0, 0);

                'march: loop {
                    if current_x >= 0 && current_x < width && current_y >= 0 && current_y < height {
                        visited[(current_y * width + current_x) as usize] = true;
                    }
                    match classify_cell(current_x, current_y, matrix, width, height) {
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

                    let x = (current_x as f32 + (dx as f32 / 2.0) - (width as f32 / 2.0)) / PHYSICS_TO_WORLD;
                    let y = (current_y as f32 + (dy as f32 / 2.0) - (height as f32 / 2.0)) / PHYSICS_TO_WORLD;

                    vertices.push((
                        x + x.signum() * size_modifier,
                        y + y.signum() * size_modifier,
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

pub fn create_polyline_collider(object_count: i32, matrix: &[i32], matrix_size: i32) -> Vec<Collider> {
    let simplified_boundaries = marching_squares(object_count, matrix, matrix_size, matrix_size, 0.0)
        .into_iter()
        .map(|boundaries| {
            boundaries.into_iter()
                .filter(|boundary| boundary.len() >= 3)
                .map(|boundary| douglas_peucker(boundary.as_slice(), COLLIDER_PRECISION / (matrix_size.pow(2)) as f32 / PHYSICS_SCALE))
                .filter(|boundary| boundary.len() >= 3)
                .collect::<Vec<Vec<(f32, f32)>>>()
        })
        .collect::<Vec<Vec<Vec<(f32, f32)>>>>();
    // object -> boundary -> points

    simplified_boundaries.iter()
        .map(|boundaries| {
            boundaries.iter()
                .map(|boundary| {
                    let boundary_vertices = boundary.iter()
                        .map(|point| {
                            Point2::new(point.0, point.1)
                        }).collect();

                    let boundary_indices = 
                        (0..boundary.len() as u32)
                        .zip((1..boundary.len() as u32 - 1).chain(0..=9))
                        .map(|(i1, i2)| {
                            [i1, i2]
                        })
                        .collect::<Vec<[u32; 2]>>();
                    
                    ColliderBuilder::polyline(boundary_vertices, Some(boundary_indices)).build()
                })
                .collect::<Vec<Collider>>()
        })
        .flatten()
        .collect::<Vec<Collider>>()
}

pub fn create_triangulated_colliders(matrix: &[i32], width: i32, height: i32) -> Collider {
    let mut boundaries = marching_squares(1, matrix, width, height, 0.0).pop().unwrap()
        .into_iter()
        .filter(|boundary| boundary.len() >= 3)
        .map(|boundary| douglas_peucker(&boundary, COLLIDER_PRECISION / CHUNK_SIZE.pow(2) as f32 / PHYSICS_SCALE))
        .filter(|simplified_boundary| simplified_boundary.len() >= 3)
        .map(|simplified_boundary| {
            simplified_boundary.iter()
                .map(|point| {
                    Point::new(
                        point.0 as f64, 
                        point.1 as f64
                    )
                })
                .collect::<Vec<Point>>()
        })
        .collect::<Vec<Vec<Point>>>();

    boundaries.sort_by(|boundary_1, boundary_2| {
        if boundary_1.len() > boundary_2.len() {
            Ordering::Less
        }
        else {
            Ordering::Greater
        }
    });  

    let builder = SweeperBuilder::new(
        boundaries.remove(0)
        )
    .add_holes(boundaries.into_iter());
    
    let triangles = builder.build().triangulate();

    ColliderBuilder::compound(
        triangles
            .map(|triangle| {
                (
                    Isometry2::default(),
                    SharedShape::triangle(
                        Point2::new(triangle.points[0].x as f32, triangle.points[0].y as f32),
                        Point2::new(triangle.points[1].x as f32, triangle.points[1].y as f32),
                        Point2::new(triangle.points[2].x as f32, triangle.points[2].y as f32),
                    )
                )
            })
            .collect::<Vec<(Isometry2<Real>, SharedShape)>>()
    ).build()
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
