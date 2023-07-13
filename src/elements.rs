use super::cell::Cell;
use super::chunk::ChunkApi;
use super::constants::*;

#[derive(Default, Clone, Copy)]
pub enum Element {
    #[default]
    Empty,
    Stone,
    Water,
    Sand,
}

fn falling_velocity_change(cell: &mut Cell, dt: &f32){
    if cell.falling {
        cell.vel_y = f32::min(cell.vel_y + GRAVITY * dt, 10.0);
        if cell.vel_x.abs() > 0.5 {
            cell.vel_x *= 0.5;
        }
        else {
            cell.vel_x = 0.0;
        }
    }
}

pub fn update_sand(mut cell: Cell, mut api: ChunkApi, dt: f32) {
    // General rules of cells
    if cell.falling {
        falling_velocity_change(&mut cell, &dt);
    }
    else {
        if matches!(api.get(0, 1).element, Element::Empty | Element::Water) {
            cell.falling = true;
        }
        else {
            let ln = api.get(-1, 1);
            let rn = api.get(1, 1);
            if cell.vel_x > 0.1 && matches!(rn.element, Element::Empty | Element::Water) {
                api.swap(1, 1);
                api.set(0, 0, cell);
                return;
            }
            else if cell.vel_x < -0.1 && matches!(ln.element, Element::Empty | Element::Water) {
                api.swap(-1, 1);
                api.set(0, 0, cell);
                return;
            }
        }
    }

    // More specific rules
    if cell.falling || f32::sqrt(f32::powi(cell.vel_x, 2) + f32::powi(cell.vel_y, 2)) > 1.0 {
        let (x1, y1) = (0, 0);
        let (x2, y2) = (cell.vel_x.round() as i32, cell.vel_y.round() as i32);

        let dx:i32 = i32::abs(x2 - x1);
        let dy:i32 = i32::abs(y2 - y1);
        let sx:i32 = { if x1 < x2 { 1 } else { -1 } };
        let sy:i32 = { if y1 < y2 { 1 } else { -1 } };

        let mut error:i32 = (if dx > dy  { dx } else { -dy }) / 2 ;
        let mut current_x:i32 = x1;
        let mut current_y:i32 = y1;

        let mut last_x = 0;
        let mut last_y = 0;

        let mut collided = false;

        loop {
            if current_x == x2 && current_y == y2 { break; }

            let error2:i32 = error;

            if error2 > -dx {
                error -= dy;
                current_x += sx;
            }
            if error2 < dy {
                error += dx;
                current_y += sy;
            }

            let nc = api.get(current_x - last_x, dy - last_y);
            match nc.element {
                Element::Empty | Element::Water => {
                    api.swap(current_x - last_x, current_y - last_y);
                    last_x = current_x;
                    last_y = current_y;
                    cell.vel_x *= 0.9;
                },
                Element::Stone | Element::Sand => {
                    collided = true;
                    break;
                },
            }
        }

        if collided {
            if (current_x - last_x) != 0 {
                cell.vel_x = 0.0;
            }
            else if cell.falling {
                if cell.vel_x.abs() > 0.5 {
                    cell.vel_x += f32::min(cell.vel_y, GRAVITY as f32) * api.random_float(0.2, 0.5) * cell.vel_x.signum();
                }
                else {
                    cell.vel_x = f32::min(cell.vel_y, GRAVITY as f32) * api.random_float(0.2, 0.5) * (api.get_direction() as f32);
                }
                cell.vel_y *= 0.7;
                cell.falling = false;
            }
            else {
                cell.vel_x *= -0.5;
            }
        }        
        api.set(0, 0, cell);
    }
}

pub fn update_liquid(mut cell: Cell, mut api: ChunkApi, dt: f32) {
    if cell.falling {
        falling_velocity_change(&mut cell, &dt);
    }
    else {
        if matches!(api.get(0, 1).element, Element::Empty) {
            cell.falling = true;
        }
        else {
            let direction = api.get_direction();
            let ln = api.get(-1, 0);
            let rn = api.get(1, 0);

            if direction > 0 {
                if matches!(rn.element, Element::Empty) {
                    cell.vel_x = 5.0;
                }
                else if matches!(ln.element, Element::Empty) {
                    cell.vel_x = -5.0;
                }
            }
            else {
                if matches!(ln.element, Element::Empty) {
                    cell.vel_x = -5.0;
                }
                else if matches!(rn.element, Element::Empty) {
                    cell.vel_x = 5.0;
                }
            }
        }
    }

    // More specific rules
    if cell.falling || f32::sqrt(f32::powi(cell.vel_x, 2) + f32::powi(cell.vel_y, 2)) > 1.0 {
        let (x1, y1) = (0, 0);
        let (x2, y2) = (cell.vel_x.round() as i32, cell.vel_y.round() as i32);

        let dx:i32 = i32::abs(x2 - x1);
        let dy:i32 = i32::abs(y2 - y1);
        let sx:i32 = { if x1 < x2 { 1 } else { -1 } };
        let sy:i32 = { if y1 < y2 { 1 } else { -1 } };

        let mut error:i32 = (if dx > dy  { dx } else { -dy }) / 2 ;
        let mut current_x:i32 = x1;
        let mut current_y:i32 = y1;

        let mut last_x = 0;
        let mut last_y = 0;

        let mut collided = false;

        loop {
            if current_x == x2 && current_y == y2 { break; }

            let error2:i32 = error;

            if error2 > -dx {
                error -= dy;
                current_x += sx;
            }
            if error2 < dy {
                error += dx;
                current_y += sy;
            }

            let nc = api.get(current_x - last_x, dy - last_y);
            match nc.element {
                Element::Empty => {
                    api.swap(current_x - last_x, current_y - last_y);
                    last_x = current_x;
                    last_y = current_y;
                    cell.vel_x *= 0.9;
                },
                _ => {
                    collided = true;
                    break;
                },
            }
        }

        if collided {
            if (current_x - last_x) != 0 {
                cell.vel_x = 0.0;
            }
            else if cell.falling {
                if cell.vel_x.abs() > 0.5 {
                    cell.vel_x += f32::min(cell.vel_y, GRAVITY as f32) * api.random_float(0.2, 0.5) * cell.vel_x.signum();
                }
                else {
                    cell.vel_x = f32::min(cell.vel_y, GRAVITY as f32) * api.random_float(0.2, 0.5) * (api.get_direction() as f32);
                }
                cell.vel_y *= 0.7;
                cell.falling = false;
            }
            else {
                cell.vel_x *= -0.5;
            }
        }        
        api.set(0, 0, cell);
    }
}