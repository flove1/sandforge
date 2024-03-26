use bevy_math::Vec2;

pub fn douglas_peucker(vertices: &[Vec2], epsilon: f32) -> Vec<Vec2> {
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
        let rec_results1 = douglas_peucker(&vertices[0..farthest_point_index], epsilon);
        let rec_results2 = douglas_peucker(&vertices[farthest_point_index..(end + 1)], epsilon);

        [rec_results1, rec_results2[1..rec_results2.len()].to_vec()].concat()
    } else {
        vec![vertices[0], vertices[end]]
    }
}

fn perpendicular_squared_distance(point: Vec2, line: (Vec2, Vec2)) -> f32 {
    let x_diff = line.1.x - line.0.x;
    let y_diff = line.1.y - line.0.y;
    let numerator =
        (y_diff * point.x - x_diff * point.y + line.1.x * line.0.y - line.1.y * line.0.x).abs();
    let numerator_squared = numerator * numerator;
    let denominator_squared = y_diff * y_diff + x_diff * x_diff;
    numerator_squared / denominator_squared
}
