use serde::{Serialize, Deserialize};
use lazy_static::lazy_static;

use crate::constants::CHUNK_SIZE;
use crate::helpers::line_from_pixels;

use super::cell::{Cell, SimulationType};
use super::chunk::ChunkApi;


#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Element {
    pub name: String,
    pub color: [u8; 4],
    pub color_offset: u8,
    pub matter: MatterType
}

impl Default for Element {
    fn default() -> Self {
        Self {
            name: "Air".to_string(),
            color: [0; 4],
            color_offset: 0,
            matter: MatterType::Empty,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub enum MatterType {
    Empty,
    Static,
    Powder,
    Liquid,
    Gas,
}

impl ToString for MatterType {
    fn to_string(&self) -> String {
        match self {
            MatterType::Empty => "Empty".to_string(),
            MatterType::Static { .. } => "Static".to_string(),
            MatterType::Powder { .. } => "Powder".to_string(),
            MatterType::Liquid { .. } => "Liquid".to_string(),
            MatterType::Gas { .. } => "Gas".to_string(),
        }
    }
}

impl Default for MatterType {
    fn default() -> Self {
        Self::Empty 
    }
}

lazy_static! {
    pub static ref ELEMENTS: Vec<Element> = {
        let mut elements = vec![Element::default()];
        
        elements.append(
            &mut serde_yaml::from_str(
                &std::fs::read_to_string("elements.yaml").unwrap()
            ).unwrap()
        );

        elements
    };
}

pub fn update_particle(mut cell: Cell, api: &mut ChunkApi, _dt: f32) {
    if let SimulationType::Particle(dx, dy) = &mut cell.simulation {
        let mut last_x = 0;
        let mut last_y = 0;

        let mut operation = |current_dx, current_dy| {
            let current_cell = api.get(current_dx, current_dy);

            if !matches!(current_cell.element.matter, MatterType::Static { .. }) {
                last_x = current_dx;
                last_y = current_dy;
                true
            }
            else {
                false
            }
            
        };

        let return_to_ca = line_from_pixels(
            0, 
            0, 
            (*dx * CHUNK_SIZE as f32).round() as i32, 
            (*dy * CHUNK_SIZE as f32).round() as i32, 
            &mut operation
        );

        if return_to_ca {
            api.update(Cell::default());
            api.set(last_x, last_y, 
                Cell { 
                     simulation: SimulationType::Ca, 
                    ..cell
                }
            );
        }
        else {
            *dy = *dy - (9.81 / CHUNK_SIZE as f32) / 10.0;
            api.set(0, 0, Cell::default());
            api.set(last_x, last_y, cell);
        }
    }
}

pub fn update_sand(cell: Cell, api: &mut ChunkApi, _dt: f32) {
    let dx = api.rand_dir();
    
    if matches!(api.get(0, -1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
        if api.once_in(5) && matches!(api.get(dx, -1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
            api.swap(dx, -1);
        }
        else {
            api.swap(0, -1);
        }
    } 
    else if matches!(api.get(dx, -1).element.matter, MatterType::Empty | MatterType::Gas{..})  {
        api.swap(dx, -1);
    } 
    else if matches!(api.get(-dx, -1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(-dx, -1);
    } 
    else if matches!(api.get(0, -1).element.matter, MatterType::Liquid{..}) {
        api.swap(0, -1);
    }

    api.update(cell);
}

// pub fn update_fire(cell: &mut Cell, mut api: ChunkApi, _dt: f32) -> ChunkApi {
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

pub fn update_liquid(mut cell: Cell, api: &mut ChunkApi, _dt: f32) {
    let mut dx = api.rand_dir();

    if matches!(api.get(0, -1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
        if api.once_in(5) {
            //randomize direction when falling sometimes
            cell.ra = api.rand_int(20) as u8;
        }

        if api.once_in(5) && matches!(api.get(((cell.ra % 2) * 2) as i32 - 1, -1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
            api.swap(((cell.ra % 2) * 2) as i32 - 1, -1);
        }
        else {
            api.swap(0, -1);
        }

        api.update(cell);
        return
    }
    else if matches!(api.get(dx, 0).element.matter, MatterType::Empty | MatterType::Gas{..}) && matches!(api.get(dx, 1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(dx, 0);

        api.update(cell);
        return
    }
    else if matches!(api.get(-dx, 0).element.matter, MatterType::Empty | MatterType::Gas{..}) && matches!(api.get(-dx, 1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(-dx, 0);

        api.update(cell);
        return
    }

    dx = if cell.ra % 2 == 0 { 1 } else { -1 };    

    let dx0 = api.get(dx, 0);
    let dxd = api.get(dx * 2, 0);

    if dx0.element.matter == MatterType::Empty && dxd.element.matter == MatterType::Empty {
        // scoot double
        cell.rb = 6;
        api.swap(dx * 2, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if matches!(nbr.element.matter, MatterType::Liquid{..}) && nbr.ra % 2 != cell.ra % 2 {
            api.set(dx, dy,
                Cell {
                    ra: cell.ra,
                    ..cell.clone()
                },
            )
        }
    } else if dx0.element.matter == MatterType::Empty {
        cell.rb = 3;
        api.swap(dx, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if matches!(nbr.element.matter, MatterType::Liquid{..}) && nbr.ra % 2 != cell.ra % 2 {
            api.set(
                dx,
                dy,
                Cell {
                    ra: cell.ra,
                    ..cell.clone()
                },
            )
        }
    } else if cell.rb == 0 {
        if matches!(api.get(-dx, 0).element.matter, MatterType::Empty) {
            // bump
            cell.ra = ((cell.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        cell.rb -= 1;
    }

    api.update(cell);
}

pub fn update_gas(mut cell: Cell, api: &mut ChunkApi, _dt: f32) {
    let mut dx = api.rand_dir();

    if matches!(api.get(dx, 0).element.matter, MatterType::Empty | MatterType::Gas{..}) && matches!(api.get(dx, 1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(dx, 0);
    }
    else if matches!(api.get(-dx, 0).element.matter, MatterType::Empty | MatterType::Gas{..}) && matches!(api.get(-dx, 1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(-dx, 0);
    }
    
    if matches!(api.get(0, 1).element.matter, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(0, 1);
        if api.once_in(20) {
            //randomize direction when falling sometimes
            cell.ra = api.rand_int(20) as u8;
        }

        api.update(cell);
        return
    }

    dx = if cell.ra % 2 == 0 { 1 } else { -1 };    

    let dx0 = api.get(dx, 0);
    let dxd = api.get(dx * 2, 0);

    if dx0.element.matter == MatterType::Empty && dxd.element.matter == MatterType::Empty {
        // scoot double
        cell.rb = 6;
        api.swap(dx * 2, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if matches!(nbr.element.matter, MatterType::Liquid{..}) && nbr.ra % 2 != cell.ra % 2 {
            api.set(dx, dy,
                Cell {
                    ra: cell.ra,
                    ..cell.clone()
                },
            )
        }
    } else if dx0.element.matter == MatterType::Empty {
        cell.rb = 3;
        api.swap(dx, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if matches!(nbr.element.matter, MatterType::Liquid{..}) && nbr.ra % 2 != cell.ra % 2 {
            api.set(
                dx,
                dy,
                Cell {
                    ra: cell.ra,
                    ..cell.clone()
                },
            )
        }
    } else if cell.rb == 0 {
        if matches!(api.get(-dx, 0).element.matter, MatterType::Empty) {
            // bump
            cell.ra = ((cell.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        cell.rb -= 1;
    }

    api.update(cell);
}