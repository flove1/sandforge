use compact_str::{CompactString, format_compact};
use dashmap::DashMap;
use serde::{Serialize, Deserialize};
use lazy_static::lazy_static;
use serde_yaml::Value;

use crate::constants::CHUNK_SIZE;
use crate::helpers::line_from_pixels;

use super::cell::{Cell, SimulationType};
use super::chunk::ChunkApi;

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Element {
    pub id: CompactString,
    pub ui_label: CompactString,
    pub color: [u8; 4],
    pub color_offset: u8,
    pub matter_type: MatterType,
    
    #[serde(default)]
    pub fire_parameters: Option<FireParameters>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct FireParameters {
    pub fire_temperature: u16,
    pub ignition_temperature: u16,
    pub fire_hp: u16,
} 

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Reaction {
    pub probability: f32, 
    pub input_element_1: String,
    pub input_element_2: String,
    pub out_element_1: String,
    pub out_element_2: String,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum MatterType {
    Empty,
    Static,
    Powder,
    Liquid(Liquid),
    Gas,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Copy)]
pub struct Liquid {
    #[serde(default = "default_one_f32")]
    pub volume: f32,
    pub dry_threshold: f32,
    pub density: u8,
    pub viscosity: u8,
    pub max_compression: f32,
}

fn default_one_f32() -> f32 {
    1.0
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

// bitflags! {
//     /// Represents a set of flags.
//     #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
//     struct Flags: u8 {
//         const ELECTROCUTED = 0b00000001;
//         const BURNING = 0b00000010;
//     }
// }

impl Default for Element {
    fn default() -> Self {
        Self {
            id: format_compact!("air"),
            ui_label: format_compact!("Air"),
            color: [0; 4],
            color_offset: 0,
            matter_type: MatterType::Empty,
            fire_parameters: None,
        }
    }
}

pub fn process_elements_config() {
    let parsed_yaml: Value = serde_yaml::from_str(
        &std::fs::read_to_string("elements.yaml").unwrap()
    ).unwrap();

    let elements: Vec<Element> = parsed_yaml.as_sequence().unwrap()
        .iter()
        .filter_map(|item| {
            serde_yaml::from_value(item.clone()).ok()
        })
        .collect();

    let reactions: Vec<Reaction> = parsed_yaml.as_sequence().unwrap()
        .iter()
        .filter_map(|item| {
            serde_yaml::from_value(item.clone()).ok()
        })
        .collect();

    elements.into_iter().for_each(|element| {
        ELEMENTS.insert(element.id.to_string(), element);
    });

    REACTIONS.clear();

    reactions.into_iter().for_each(|reaction| {
        if !REACTIONS.contains_key(&reaction.input_element_1) {
            REACTIONS.insert(reaction.input_element_1.clone(), DashMap::with_hasher(ahash::RandomState::new()));
        }

        REACTIONS.get_mut(reaction.input_element_1.as_str()).unwrap().insert(reaction.input_element_2.clone(), reaction);
    });
}

lazy_static! {
    pub static ref ELEMENTS: DashMap<String, Element, ahash::RandomState> = {
        let elements = DashMap::with_hasher(ahash::RandomState::new());

        [
            Element::default(),
            Element { 
                id: format_compact!("grass"),
                ui_label: format_compact!("Grass"),
                matter_type: MatterType::Static,
                color: [0x7d, 0xaa, 0x4d, 0xff], 
                color_offset: 10, 
                fire_parameters: Some(FireParameters {
                    fire_temperature: 125,
                    ignition_temperature: 75,
                    fire_hp: 25,
                }),
            },
            Element { 
                id: format_compact!("dirt"),
                ui_label: format_compact!("Dirt"),
                matter_type: MatterType::Static,
                color: [0x6d, 0x5f, 0x3d, 0xff], 
                color_offset: 10, 
                fire_parameters: None,
            },
            Element { 
                id: format_compact!("stone"),
                ui_label: format_compact!("Stone"),
                matter_type: MatterType::Static,
                color: [0x71, 0x77, 0x77, 0xff],
                color_offset: 25, 
                fire_parameters: None,
            }
        ].into_iter().for_each(|element| {
            elements.insert(element.id.to_string(), element);
        });

        elements
    };

    pub static ref REACTIONS: DashMap<String, DashMap<String, Reaction, ahash::RandomState>, ahash::RandomState> = {
        DashMap::with_hasher(ahash::RandomState::new())
    };
}

pub fn update_particle(mut cell: Cell, api: &mut ChunkApi, _dt: f32) {
    if let SimulationType::Particle(dx, dy) = &mut cell.simulation {
        let mut last_x = 0;
        let mut last_y = 0;

        let mut operation = |current_dx, current_dy| {
            let current_cell = api.get(current_dx, current_dy);

            if !matches!(current_cell.matter_type, MatterType::Static { .. }) {
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
    
    if matches!(api.get(0, -1).matter_type, MatterType::Empty | MatterType::Gas{..}) {
        if api.once_in(5) && matches!(api.get(dx, -1).matter_type, MatterType::Empty | MatterType::Gas{..}) {
            api.swap(dx, -1);
        }
        else {
            api.swap(0, -1);
        }
    } 
    else if matches!(api.get(dx, -1).matter_type, MatterType::Empty | MatterType::Gas{..})  {
        api.swap(dx, -1);
    } 
    else if matches!(api.get(-dx, -1).matter_type, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(-dx, -1);
    } 
    else if matches!(api.get(0, -1).matter_type, MatterType::Liquid{..}) {
        if api.once_in(30) && matches!(api.get(dx, -1).matter_type, MatterType::Empty | MatterType::Gas{..} | MatterType::Liquid{..}) {
            api.swap(dx, -1);
        }
        else {
            api.swap(0, -1);
        }
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


// Sometimes stuck on edges, needs fixing
/// CA rules:
/// 1. Remove leftovers
/// If the liquid volume is less than 0.001, the cell is set to default (empty), and the update is performed.
/// 
/// 2. Falling down
/// If the cell below is empty or gas, the liquid cell has a chance to move downward. 
/// If there's space, it may move randomly sideways sometimes. Otherwise, it continues falling.
/// 
/// If there's liquid below it checks ids.
/// if IDs match, they top cell fiils bottom one to fill it.
/// Otherwise it checks densities:
/// If the density of the current liquid is higher, it swaps places with the lower liquid.
/// 
/// If the cell position is changed then following steps are skipped
/// 
/// 3. Flow sideways
/// The cell will try fill horizontal neighbors
/// 
/// 4. Handling Excess Volume at the Top:
/// If the liquid has excess volume at the top, it may flow upward to the cell above:
/// If the cell above is empty, it flows into that cell.
/// If the cell above is another liquid of the same element ID, they might exchange volume.


/// Flowing Sideways: The liquid has a tendency to flow sideways based on viscosity and volume. It attempts to flow towards an empty space or merge with lower-density liquids when moving sideways.

// Overflow Handling: If the liquid volume exceeds a certain threshold, it tries to overflow to the cell above. If the cell above is empty or contains the same liquid, it moves some volume there, otherwise, it stops overflowing.

// Update Cell: Finally, the cell's properties are updated according to the changes that have occurred.

/// 
/// Liquid Below: 
/// 2. If the cell is above an empty or gas cell, there's a chance to randomize the falling direction.
/// If the cell below is another liquid, the liquid with higher density stays, and the lower density one moves downward.
///    
/// The liquid can move downward if the cell below is empty or gas.
/// 
/// 
/// Handling Interaction with Bottom Liquid:
/// If the liquid density is higher than the bottom liquid, it swaps places with the bottom liquid.
/// If the two liquids have the same element ID and the bottom liquid's volume is less than 1.0, they merge.
/// 
/// Handling Random Movement:
/// The liquid has a random chance to move in a random direction.
/// The movement can be constrained by viscosity.
/// 
/// Handling Flow:
/// The liquid flows in a chosen direction (dx) based on randomization and viscosity.
/// The liquid can flow to adjacent empty cells or merge with adjacent liquid cells.
/// The flow is influenced by density, volume, and viscosity of the liquids.
/// 
/// Handling Overflow:
/// If the liquid volume is greater than 1.0, excess liquid may flow upward.
/// 
/// Updating the Cell State:
/// The cell state is updated based on the applied rules.
pub fn update_liquid(mut cell: Cell, api: &mut ChunkApi, _dt: f32) {
    let MatterType::Liquid(parameters) = &mut cell.matter_type.clone() else {
        panic!();
    };

    let mut bottom_cell = api.get(0, -1);
    match &mut bottom_cell.matter_type {
        MatterType::Empty | MatterType::Gas => {
            if api.once_in(5) {
                cell.ra = api.rand_int(20) as u8;
                api.update(cell.clone());
            }

            // let dx = ((cell.ra % 2) as i32) * 2 - 1;
            
            // if api.once_in(10) && matches!(api.get(dx, -1).matter_type, MatterType::Empty | MatterType::Gas{..}) {
            //     api.swap(dx, -1);
            // }
            // else {
                api.swap(0, -1);
            // }

            return;
        },
        MatterType::Liquid(bottom_parameters) => {
            if parameters.density > bottom_parameters.density {
                api.swap(0, -1);   
                
                return;
            }

            if cell.element_id == bottom_cell.element_id && bottom_parameters.volume < 1.0 {
                let required_flow = 1.0 - bottom_parameters.volume;

                if required_flow > parameters.volume {
                    api.update(Cell::default());
                    api.set(0, -1, Cell {
                        matter_type: MatterType::Liquid(Liquid {
                            volume: bottom_parameters.volume + parameters.volume,
                            ..bottom_parameters.clone()
                        }),
                        ..bottom_cell.clone()
                    });
                    
                    return;
                }

                parameters.volume -= required_flow;
                bottom_parameters.volume += required_flow;

                api.update(Cell {
                    matter_type: MatterType::Liquid(Liquid {
                        ..parameters.clone()
                    }),
                    ..cell.clone()
                });

                api.set(0, -1, Cell {
                    matter_type: MatterType::Liquid(Liquid {
                        ..bottom_parameters.clone()
                    }),
                    ..bottom_cell.clone()
                });
            }
        },
        _ => {}
    }

    if parameters.volume > 1.0 {
        let top_cell = api.get(0, 1);

        match top_cell.matter_type {
            MatterType::Empty => {        
                let flow = parameters.volume / 2.0 - parameters.dry_threshold;
                parameters.volume -= flow;

                api.set(0, 1, Cell {
                    matter_type: MatterType::Liquid(Liquid {
                        volume: flow,
                        ..parameters.clone()
                    }),
                    ..cell.clone()
                })
            },
            MatterType::Liquid(top_parameters) => {
                if cell.element_id == top_cell.element_id && top_parameters.volume < parameters.volume {
                    let flow = ((top_parameters.volume + parameters.volume) / 2.0 - top_parameters.volume) / 2.0;
                    parameters.volume -= flow;
                    
                    api.set(0, 1, Cell{
                        matter_type: MatterType::Liquid(Liquid {
                            volume: top_parameters.volume + flow,
                            ..top_parameters.clone()
                        }),
                        ..top_cell.clone()
                    })
                }
            },
            // MatterType::Static => {
            //     let dx = api.rand_dir();

            //     for dx in [dx, -dx] {
            //         let dx_cell = api.get(dx, 1);

            //         match dx_cell.matter_type {
            //             MatterType::Empty | MatterType::Gas => {
            //                 api.swap(dx, 1);
            //                 break;
            //             },
            //             MatterType::Liquid(..) => {
            //                 if cell.element_id != dx_cell.element_id {
            //                     api.swap(dx, 1);
            //                     break;
            //                 }
            //             },
            //             _ => {},
            //         }
            //     }
            // }
            _ => {}
        }
    }

    let dx = api.rand_dir();
    let mut dy_s = [0, 0];
    let mut flows = 0;

    let modifier = 1.0;
    let max_offset = i32::clamp(10 - parameters.viscosity as i32 + 1, 1, 10);

    'offset_loop: for offset in 1..=(max_offset as f32 * modifier) as i32 {
        if parameters.volume < parameters.dry_threshold {
            api.keep_alive(0, 0);
            api.update(Cell::default());
            return;
        }

        let max_flow = f32::min(parameters.volume / 3.0, 1.0);
        let mut exit_flag = false;

        for (dx, dy) in vec![dx, -dx].into_iter().zip(dy_s.iter_mut()) {
            let mut dx_cell = api.get(dx * offset, *dy);
        
            match &mut dx_cell.matter_type { 
                MatterType::Empty => {
                    let flow = max_flow / 2.0;
                    flows += 1;
                    parameters.volume -= flow;

                    if api.get(dx * offset, *dy - 1).matter_type == MatterType::Empty {
                        api.set(dx * offset, *dy - 1, Cell{
                            clock: cell.clock.saturating_sub(1),
                            matter_type: MatterType::Liquid(Liquid{
                                volume: flow,
                                ..parameters.clone()
                            }),
                            ..cell.clone()
                        });
                        *dy -= 1;
                    }
                    else {
                        api.set(dx * offset, *dy, Cell{
                            clock: cell.clock.saturating_sub(1),
                            matter_type: MatterType::Liquid(Liquid{
                                volume: flow,
                                ..parameters.clone()
                            }),
                            ..cell.clone()
                        })
                    }
                },
                MatterType::Liquid(dx_parameters) => {
                    if cell.element_id == dx_cell.element_id {
                        let flow = [parameters.volume - dx_parameters.volume, max_flow / 2.0, 1.0]
                            .into_iter()
                            .reduce(|a, b| f32::min(a, b))
                            .unwrap();
    
                        if dx_parameters.volume < parameters.volume {
                            parameters.volume -= flow;
    
                            flows += 1;
                            api.set(dx * offset, *dy, Cell{
                                matter_type: MatterType::Liquid(Liquid{
                                    volume: dx_parameters.volume + flow,
                                    ..dx_parameters.clone()
                                }), 
                                ..dx_cell.clone()
                            })
                        }
                    }
                    else if parameters.density > dx_parameters.density {
                        let offset_directions = [(dx * (offset + 1), *dy), (dx * offset, *dy + 1)];
                        let mut succesfully_moved = false;

                        for (dx_next, dy_next) in offset_directions {
                            let offset_cell = api.get(dx_next, dy_next);

                            if offset_cell.matter_type == MatterType::Empty {
                                api.set(dx_next, dy_next, Cell{
                                    clock: dx_cell.clock.saturating_sub(1),
                                    ..dx_cell.clone()
                                });
                            }
                            else if offset_cell.element_id == dx_cell.element_id {
                                let MatterType::Liquid(offset_parameters) = offset_cell.matter_type else {
                                    panic!();
                                };

                                let maximum_receive_flow = f32::clamp(offset_parameters.max_compression - offset_parameters.volume, 0.0, 1.0);
                                if maximum_receive_flow > dx_parameters.volume {
                                    api.set(dx_next, dy_next, Cell{
                                        matter_type: MatterType::Liquid(Liquid{
                                            volume: offset_parameters.volume + dx_parameters.volume,
                                            ..offset_parameters.clone()
                                        }),
                                        ..offset_cell.clone()
                                    })
                                }
                                else {
                                    let flow_to_avg = (dx_parameters.volume + offset_parameters.volume) / 2.0 - offset_parameters.volume;

                                    dx_parameters.volume -= flow_to_avg;

                                    api.set(dx_next, dy_next, Cell{
                                        matter_type: MatterType::Liquid(Liquid{
                                            volume: offset_parameters.volume + flow_to_avg,
                                            ..offset_parameters.clone()
                                        }),
                                        ..offset_cell.clone()
                                    });
                                    
                                    continue;
                                }    
                            }
                            else {
                                continue;
                            }

                            let flow = max_flow / 2.0;
                            parameters.volume -= flow;

                            flows += 1;
                            api.set(dx * offset, *dy, Cell{
                                clock: cell.clock.saturating_sub(1),
                                matter_type: MatterType::Liquid(Liquid{
                                    volume: flow,
                                    ..parameters.clone()
                                }),
                                ..cell.clone()
                            });

                            succesfully_moved = true;
                            break;
                        }

                        if !succesfully_moved {
                            exit_flag = true;

                            api.set(dx * offset, *dy, Cell{
                                ..dx_cell.clone()
                            });
                        }
                    }
                },
                _ => {
                    exit_flag = true;
                }
            }
        }
        if exit_flag {
            break 'offset_loop;
        }
    }

    if flows > 0 {
        api.keep_alive(0, 0);
    }
    
    api.update(Cell {
        matter_type: MatterType::Liquid(Liquid {
            ..parameters.clone()
        }),
        ..cell
    });
}

pub fn update_gas(mut cell: Cell, api: &mut ChunkApi, _dt: f32) {
    let mut dx = api.rand_dir();

    if matches!(api.get(dx, 0).matter_type, MatterType::Empty | MatterType::Gas{..}) && matches!(api.get(dx, 1).matter_type, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(dx, 0);
    }
    else if matches!(api.get(-dx, 0).matter_type, MatterType::Empty | MatterType::Gas{..}) && matches!(api.get(-dx, 1).matter_type, MatterType::Empty | MatterType::Gas{..}) {
        api.swap(-dx, 0);
    }
    
    if matches!(api.get(0, 1).matter_type, MatterType::Empty | MatterType::Gas{..}) {
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

    if dx0.matter_type == MatterType::Empty && dxd.matter_type == MatterType::Empty {
        // scoot double
        cell.rb = 6;
        api.swap(dx * 2, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if matches!(nbr.matter_type, MatterType::Liquid{..}) && nbr.ra % 2 != cell.ra % 2 {
            api.set(dx, dy,
                Cell {
                    ra: cell.ra,
                    ..cell.clone()
                },
            )
        }
    } else if dx0.matter_type == MatterType::Empty {
        cell.rb = 3;
        api.swap(dx, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if matches!(nbr.matter_type, MatterType::Liquid{..}) && nbr.ra % 2 != cell.ra % 2 {
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
        if matches!(api.get(-dx, 0).matter_type, MatterType::Empty) {
            // bump
            cell.ra = ((cell.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        cell.rb -= 1;
    }

    api.update(cell);
}