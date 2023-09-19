use serde::{Serialize, Deserialize};
use lazy_static::lazy_static;

use crate::constants::CHUNK_SIZE;
use crate::helpers::line_from_pixels;

use super::cell::{Cell, SimulationType};
use super::chunk::ChunkApi;


#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub enum MatterType {
    Empty,
    Static {
        name: String,
        color: [u8; 4],
        color_offset: u8,
    },
    Powder {
        name: String,
        color: [u8; 4],
        color_offset: u8,
    },
    Liquid {
        name: String,
        color: [u8; 4],
        color_offset: u8,
    },
    Gas {
        name: String,
        color: [u8; 4],
        color_offset: u8,
    },
}

macro_rules! check_matter {
    ($val:expr, $var:path) => {
        match $val {
            $var{..} => true,
            _ => false
        }
    }
}

impl Default for MatterType {
    fn default() -> Self {
        Self::Empty
    }
}

lazy_static! {
    pub static ref MATTERS: Vec<MatterType> = {
        let mut elements = vec![MatterType::Empty];
        
        elements.append(
            &mut serde_yaml::from_str(
                &std::fs::read_to_string("elements.yaml").unwrap()
            ).unwrap()
        );

        elements
    };
}

pub fn update_particle<'a, 'b>(cell: &mut Cell, api: &mut ChunkApi<'a, 'b>, _dt: f32) {
    if let SimulationType::Particle { x, y, dx, dy, collided } = &mut cell.simulation {
        if *collided {
            return;
        }
        
        let mut last_x = 0;
        let mut last_y = 0;

        let mut operation = |dx, dy| {
            let cell = api.get(dx, dy);

            if check_matter!(cell.element, MatterType::Empty) || !matches!(cell.simulation, SimulationType::Ca) {
                last_x = dx;
                last_y = dy;
                true
            }
            else {
                false
            }
            
        };

        let return_to_ca = line_from_pixels(
            0, 
            0, 
            (*dx * CHUNK_SIZE as f32).floor() as i32, 
            (*dy * CHUNK_SIZE as f32).floor() as i32, 
            &mut operation
        );

        if return_to_ca {
            *collided = true;
        }
        else {
            *x = *x + *dx;
            *y = *y + *dy;
            *dy = f32::min(*dy + (1.0 / CHUNK_SIZE as f32) / 10.0, dy.signum() * 9.1 * (1.0 / CHUNK_SIZE as f32) / 10.0);
        }

        // api.update(cell.clone());
        api.keep_alive(last_x, last_y);
    }
    else {
        panic!("particle method called for non-particle cell");
    }
}

pub fn update_sand<'a, 'b>(cell: &Cell, api: &mut ChunkApi<'a, 'b>, _dt: f32) {
    let dx = api.rand_dir();
    
    if check_matter!(api.get(0, 1).element, MatterType::Empty) {
        if api.once_in(5) && check_matter!(api.get(dx, 1).element, MatterType::Empty) {
            api.swap(dx, 1);
        }
        else {
            api.swap(0, 1);
        }
    } 
    else if check_matter!(api.get(dx, 1).element, MatterType::Empty)  {
        api.swap(dx, 1);
    } 
    else if check_matter!(api.get(-dx, 1).element, MatterType::Empty) {
        api.swap(-dx, 1);
    } 
    else if check_matter!(api.get(0, 1).element, MatterType::Liquid) {
        api.swap(0, 1);
    }

    api.update(cell.clone());
}

// pub fn update_fire<'a, 'b>(cell: &mut Cell, mut api: ChunkApi<'a, 'b>, _dt: f32) -> ChunkApi<'a, 'b> {
//     let directions = [(0, 1), (1, 0), (0, -1), (-1, 0), (1, 1), (-1, -1), (1, -1), (-1, 1)];

//     for (dx, dy) in directions {
//         let cell = api.get(dx, dy);

//         let modifier = if dy == -1 { 0.5 } else { 1.0 };

//         match cell.element {
//             MatterType::Wood => {
//                 if api.once_in((100.0 * modifier) as i32) {
//                     api.set(dx, dy, Cell::new_with_rb(MatterType::Fire, cell.clock, 80))
//                 }
//             }

//             MatterType::Coal => {
//                 if api.once_in((500.0 * modifier) as i32) {
//                     api.set(dx, dy, Cell::new_with_rb(MatterType::Fire, cell.clock, 100))
//                 }
//             }

//             MatterType::Gas => {
//                 if api.once_in(10) {
//                     api.set(dx, dy, Cell::new_with_rb(MatterType::Fire, cell.clock, 10))
//                 }
//             }   
            
//             _ => {}
//         }
//     }



//     if api.once_in(2) {
//         cell.rb -= 1;
//     }

//     if cell.rb == 0 {
//         cell.element = MatterType::Empty;
//     }

//     api.set(0, 0, *cell);
//     api
// }

pub fn update_liquid<'a, 'b>(cell: &mut Cell, api: &mut ChunkApi<'a, 'b>, _dt: f32) {
    let mut dx = api.rand_dir();

    if check_matter!(api.get(0, 1).element, MatterType::Empty) {
        if api.once_in(5) {
            //randomize direction when falling sometimes
            cell.ra = api.rand_int(20) as u8;
        }

        if api.once_in(5) && check_matter!(api.get(((cell.ra % 2) * 2) as i32 - 1, 1).element, MatterType::Empty) {
            api.swap(((cell.ra % 2) * 2) as i32 - 1, 1);
        }
        else {
            api.swap(0, 1);
        }

        api.update(cell.clone());
        return
    }
    else if check_matter!(api.get(dx, 0).element, MatterType::Empty) && check_matter!(api.get(dx, 1).element, MatterType::Empty) {
        api.swap(dx, 0);

        api.update(cell.clone());
        return
    }
    else if check_matter!(api.get(-dx, 0).element, MatterType::Empty) && check_matter!(api.get(-dx, 1).element, MatterType::Empty) {
        api.swap(-dx, 0);

        api.update(cell.clone());
        return
    }

    dx = if cell.ra % 2 == 0 { 1 } else { -1 };    

    let dx0 = api.get(dx, 0);
    let dxd = api.get(dx * 2, 0);

    if dx0.element == MatterType::Empty && dxd.element == MatterType::Empty {
        // scoot double
        cell.rb = 6;
        api.swap(dx * 2, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if check_matter!(nbr.element, MatterType::Liquid) {
            if nbr.ra % 2 != cell.ra % 2 {
                api.set(dx, dy,
                    Cell {
                        ra: cell.ra,
                        ..cell.clone()
                    },
                )
            }
        }
    } else if dx0.element == MatterType::Empty {
        cell.rb = 3;
        api.swap(dx, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if check_matter!(nbr.element, MatterType::Liquid) {
            if nbr.ra % 2 != cell.ra % 2 {
                api.set(
                    dx,
                    dy,
                    Cell {
                        ra: cell.ra,
                        ..cell.clone()
                    },
                )
            }
        }
    } else if cell.rb == 0 {
        if check_matter!(api.get(-dx, 0).element, MatterType::Empty) {
            // bump
            cell.ra = ((cell.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        cell.rb -= 1;
    }

    api.update(cell.clone());
}

pub fn update_gas<'a, 'b>(cell: &mut Cell, api: &mut ChunkApi<'a, 'b>, _dt: f32) {
    let mut dx = api.rand_dir();

    if check_matter!(api.get(dx, 0).element, MatterType::Empty) && check_matter!(api.get(dx, -1).element, MatterType::Empty) {
        api.swap(dx, 0);
    }
    else if check_matter!(api.get(-dx, 0).element, MatterType::Empty) && check_matter!(api.get(-dx, -1).element, MatterType::Empty) {
        api.swap(-dx, 0);
    }
    
    if check_matter!(api.get(0, -1).element, MatterType::Empty) {
        api.swap(0, -1);
        if api.once_in(20) {
            //randomize direction when falling sometimes
            cell.ra = api.rand_int(20) as u8;
        }

        api.update(cell.clone());
        return
    }

    dx = if cell.ra % 2 == 0 { 1 } else { -1 };    

    let dx0 = api.get(dx, 0);
    let dxd = api.get(dx * 2, 0);

    if dx0.element == MatterType::Empty && dxd.element == MatterType::Empty {
        // scoot double
        cell.rb = 6;
        api.swap(dx * 2, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if check_matter!(nbr.element, MatterType::Liquid) {
            if nbr.ra % 2 != cell.ra % 2 {
                api.set(dx, dy,
                    Cell {
                        ra: cell.ra,
                        ..cell.clone()
                    },
                )
            }
        }
    } else if dx0.element == MatterType::Empty {
        cell.rb = 3;
        api.swap(dx, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if check_matter!(nbr.element, MatterType::Liquid) {
            if nbr.ra % 2 != cell.ra % 2 {
                api.set(
                    dx,
                    dy,
                    Cell {
                        ra: cell.ra,
                        ..cell.clone()
                    },
                )
            }
        }
    } else if cell.rb == 0 {
        if check_matter!(api.get(-dx, 0).element, MatterType::Empty) {
            // bump
            cell.ra = ((cell.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        cell.rb -= 1;
    }

    api.update(cell.clone());
}