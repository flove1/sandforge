use ahash::HashSet;
use bevy::{
    asset::{Asset, Handle},
    ecs::system::Resource,
    reflect::TypePath,
};
use compact_str::{format_compact, CompactString};
use dashmap::DashMap;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::chunk::ChunkApi;

use super::pixel::Pixel;

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Material {
    pub id: String,
    pub matter_type: PhysicsType,
    pub color: [u8; 4],
    pub color_offset: u8,

    #[serde(default)]
    pub fire_parameters: Option<FireParameters>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub enum PhysicsType {
    Empty,
    Static,
    Powder,
    Liquid(Liquid),
    Gas,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
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

impl ToString for PhysicsType {
    fn to_string(&self) -> String {
        match self {
            PhysicsType::Empty => "Empty".to_string(),
            PhysicsType::Static { .. } => "Static".to_string(),
            PhysicsType::Powder { .. } => "Powder".to_string(),
            PhysicsType::Liquid { .. } => "Liquid".to_string(),
            PhysicsType::Gas { .. } => "Gas".to_string(),
        }
    }
}

impl Default for PhysicsType {
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

impl Default for Material {
    fn default() -> Self {
        Self {
            id: "air".to_string(),
            matter_type: PhysicsType::Empty,
            fire_parameters: None,
            color: [0; 4],
            color_offset: 0
        }
    }
}


pub fn process_elements_config() {
    let parsed_yaml: Value =
        serde_yaml::from_str(&std::fs::read_to_string("elements.yaml").unwrap()).unwrap();

    let elements: Vec<Material> = parsed_yaml
        .as_sequence()
        .unwrap()
        .iter()
        .filter_map(|item| serde_yaml::from_value(item.clone()).ok())
        .collect();

    let reactions: Vec<Reaction> = parsed_yaml
        .as_sequence()
        .unwrap()
        .iter()
        .filter_map(|item| serde_yaml::from_value(item.clone()).ok())
        .collect();

    elements.into_iter().for_each(|material| {
        ELEMENTS.insert(material.id.to_string(), material);
    });

    REACTIONS.clear();

    reactions.into_iter().for_each(|reaction| {
        if !REACTIONS.contains_key(&reaction.input_element_1) {
            REACTIONS.insert(
                reaction.input_element_1.clone(),
                DashMap::with_hasher(ahash::RandomState::new()),
            );
        }

        REACTIONS
            .get_mut(reaction.input_element_1.as_str())
            .unwrap()
            .insert(reaction.input_element_2.clone(), reaction);
    });
}

lazy_static! {
    pub static ref ELEMENTS: DashMap<String, Material, ahash::RandomState> = {
        let elements = DashMap::with_hasher(ahash::RandomState::new());

        [
            Material::default(),
            Material {
                id: "grass".to_string(),
                matter_type: PhysicsType::Static,
                color: [0x7d, 0xaa, 0x4d, 0xff],
                color_offset: 10,
                fire_parameters: Some(FireParameters {
                    fire_temperature: 125,
                    ignition_temperature: 75,
                    fire_hp: 25,
                }),
            },
            Material {
                id: "dirt".to_string(),
                matter_type: PhysicsType::Static,
                color: [0x6d, 0x5f, 0x3d, 0xff],
                color_offset: 15,
                fire_parameters: None,
            },
            Material {
                id: "stone".to_string(),
                matter_type: PhysicsType::Static,
                color: [0x71, 0x77, 0x77, 0xff],
                color_offset: 20,
                fire_parameters: None,
            },
            Material {
                id: "actor".to_string(),
                matter_type: PhysicsType::Empty,
                color: [0xff, 0x00, 0x00, 0x50],
                color_offset: 0,
                fire_parameters: None,
            },
        ]
        .into_iter()
        .for_each(|material| {
            elements.insert(material.id.to_string(), material);
        });

        elements
    };
    pub static ref REACTIONS: DashMap<String, DashMap<String, Reaction, ahash::RandomState>, ahash::RandomState> =
        { DashMap::with_hasher(ahash::RandomState::new()) };
}

// pub fn update_displaced(mut pixel: Pixel, api: &mut ChunkApi, _dt: f32) {
//     let SimulationType::Displaced(dx, dy) = &mut pixel.simulation else {
//         panic!();
//     };

//     if (dx.abs() + dy.abs()) < 0.1 {
//         pixel.simulation = SimulationType::Ca;
//         api.update(pixel);
//         api.keep_alive(0, 0);
//         return;
//     }

//     let rounded_dx = (dx.round() as i32).clamp(-1, 1);
//     let rounded_dy = (dy.round() as i32).clamp(-1, 1);

//     match api.get(rounded_dx, rounded_dy).matter_type {
//         PhysicsType::Empty => {
//             api.swap(rounded_dx, rounded_dy);
//             *dx *= 0.1;
//             *dy *= 0.1;
//             api.update(pixel);
//             api.keep_alive(0, 0);
//             return;
//         },
//         PhysicsType::Static => {
//             let angle = dy.atan2(*dx) - (rounded_dy as f32).atan2(rounded_dx as f32);

//             *dx = *dx * angle.sin();
//             *dy = *dy * angle.cos();
//         },
//         _ => {}
//     }

//     let affected_horizontal: HashSet<i32> = if *dx > 0.5 {
//         [0, 1].into_iter().collect()
//     }
//     else if *dx < -0.5 {
//         [-1, 0].into_iter().collect()
//     }
//     else {
//         [-1, 0, 1].into_iter().collect()
//     };

//     let affected_vertical: HashSet<i32> = if *dy > 0.5 {
//         [0, 1].into_iter().collect()
//     }
//     else if *dy < -0.5 {
//         [-1, 0].into_iter().collect()
//     }
//     else {
//         [-1, 0, 1].into_iter().collect()
//     };

//     let affected_total_count = affected_horizontal.len() * affected_vertical.len();

//     let mut dx_sum_changes = 0.0;
//     let mut dy_sum_changes = 0.0;

//     for (collision_dx, collision_dy) in affected_horizontal.into_iter().zip(affected_vertical.into_iter()) {
//         if collision_dx == 0 && collision_dy == 0 {
//             continue;
//         }

//         let mut affected_cell = api.get(collision_dx, collision_dy);

//         match affected_cell.matter_type {
//             PhysicsType::Powder | PhysicsType::Liquid(_) | PhysicsType::Gas => {
//                 match &mut affected_cell.simulation {
//                     SimulationType::Ca => {
//                         let angle = dy.atan2(*dx) - (collision_dy as f32).atan2(collision_dx as f32);
//                         let distance = f32::sqrt(collision_dx.pow(2) as f32 + collision_dy.pow(2) as f32);

//                         let dx_change = *dx * angle.sin() / distance / affected_total_count as f32;
//                         let dy_change = *dy * angle.cos() / distance / affected_total_count as f32;

//                         affected_cell.simulation = SimulationType::Displaced(
//                             dx_change,
//                             dy_change,
//                         );
//                         dx_sum_changes += dx_change.abs();
//                         dy_sum_changes += dy_change.abs();

//                         api.set(collision_dx, collision_dy, affected_cell);
//                     },
//                     SimulationType::Displaced(dx1, dy1) => {
//                         let angle = dy.atan2(*dx) - (collision_dy as f32).atan2(collision_dx as f32);
//                         let distance = f32::sqrt(collision_dx.pow(2) as f32 + collision_dy.pow(2) as f32);

//                         let dx_change = *dx * angle.sin() / distance / affected_total_count as f32;
//                         let dy_change = *dy * angle.cos() / distance / affected_total_count as f32;

//                         *dx1 += dx_change;
//                         *dy1 += dy_change;

//                         dx_sum_changes += dx_change.abs();
//                         dy_sum_changes += dy_change.abs();

//                         api.set(collision_dx, collision_dy, affected_cell);
//                     },
//                     _ => {}
//                 }
//             },
//             _ => {},
//         }
//     }

//     *dx = f32::clamp(dx.abs() - dx_sum_changes.abs(), 0.0, dx.abs()) * dx.signum() * 0.99;
//     *dy = f32::clamp(dy.abs() - dy_sum_changes.abs(), 0.0, dy.abs()) * dy.signum() * 0.99;

//     api.update(pixel);
//     api.keep_alive(0, 0);
// }

pub fn update_sand(api: &mut ChunkApi) {
    let dx = api.rand_dir();

    if matches!(api.get_matter(0, -1), PhysicsType::Empty | PhysicsType::Gas{..}) {
        if api.once_in(5) && matches!(api.get_matter(dx, -1), PhysicsType::Empty | PhysicsType::Gas{..}) {
            api.swap(dx, -1);
        }
        else {
            api.swap(0, -1);
        }
    } 
    else if matches!(api.get_matter(dx, -1), PhysicsType::Empty | PhysicsType::Gas{..})  {
        api.swap(dx, -1);
    } 
    else if matches!(api.get_matter(-dx, -1), PhysicsType::Empty | PhysicsType::Gas{..}) {
        api.swap(-dx, -1);
    } 
    else if matches!(api.get_matter(0, -1), PhysicsType::Liquid{..}) {
        if api.once_in(30) && matches!(api.get_matter(dx, -1), PhysicsType::Empty | PhysicsType::Gas{..} | PhysicsType::Liquid{..}) {
            api.swap(dx, -1);
        }
        else {
            api.swap(0, -1);
        }
    }
}

// pub fn update_fire(pixel: &mut Pixel, mut api: ChunkApi, _dt: f32) -> ChunkApi {
//     let directions = [(0, 1), (1, 0), (0, -1), (-1, 0), (1, 1), (-1, -1), (1, -1), (-1, 1)];

//     for (dx, dy) in directions {
//         let pixel = api.get(dx, dy);

//         let modifier = if dy == -1 { 0.5 } else { 1.0 };

//         match pixel.material {
//             PhysicsType::Wood => {
//                 if api.once_in((100.0 * modifier) as i32) {
//                     api.set(dx, dy, Pixel::new_with_rb(PhysicsType::Fire, pixel.clock, 80))
//                 }
//             }

//             PhysicsType::Coal => {
//                 if api.once_in((500.0 * modifier) as i32) {
//                     api.set(dx, dy, Pixel::new_with_rb(PhysicsType::Fire, pixel.clock, 100))
//                 }
//             }

//             PhysicsType::Gas => {
//                 if api.once_in(10) {
//                     api.set(dx, dy, Pixel::new_with_rb(PhysicsType::Fire, pixel.clock, 10))
//                 }
//             }

//             _ => {}
//         }
//     }

//     if api.once_in(2) {
//         pixel.rb -= 1;
//     }

//     if pixel.rb == 0 {
//         pixel.material = PhysicsType::Empty;
//     }

//     api.set(0, 0, *pixel);
//     api
// }

// Sometimes stuck on edges, needs fixing
pub fn update_liquid(api: &mut ChunkApi) {
    let mut pixel = api.get(0, 0);
    let PhysicsType::Liquid(parameters) = &mut pixel.matter_type.clone() else {
        panic!();
    };

    let mut bottom_cell = api.get(0, -1);
    match &mut bottom_cell.matter_type {
        PhysicsType::Empty | PhysicsType::Gas => {
            if api.once_in(5) {
                pixel.ra = api.rand_int(20) as u8;
                api.update(pixel.clone());
            }

            // let dx = ((pixel.ra % 2) as i32) * 2 - 1;

            // if api.once_in(10) && matches!(api.get(dx, -1).matter_type, PhysicsType::Empty | PhysicsType::Gas{..}) {
            //     api.swap(dx, -1);
            // }
            // else {
                api.swap(0, -1);
            // }

            return;
        },
        PhysicsType::Liquid(bottom_parameters) => {
            if parameters.density > bottom_parameters.density {
                api.swap(0, -1);

                return;
            }

            if pixel.material_id == bottom_cell.material_id && bottom_parameters.volume < 1.0 {
                let required_flow = 1.0 - bottom_parameters.volume;

                if required_flow > parameters.volume {
                    api.update(Pixel::default());
                    api.set(0, -1, Pixel {
                        matter_type: PhysicsType::Liquid(Liquid {
                            volume: bottom_parameters.volume + parameters.volume,
                            ..*bottom_parameters
                        }),
                        ..bottom_cell.clone()
                    });

                    return;
                }

                parameters.volume -= required_flow;
                bottom_parameters.volume += required_flow;

                api.update(Pixel {
                    matter_type: PhysicsType::Liquid(Liquid {
                        ..parameters.clone()
                    }),
                    ..pixel.clone()
                });

                api.set(0, -1, Pixel {
                    matter_type: PhysicsType::Liquid(Liquid {
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
            PhysicsType::Empty => {
                let flow = parameters.volume / 2.0 - parameters.dry_threshold;
                parameters.volume -= flow;

                api.set(0, 1, Pixel {
                    matter_type: PhysicsType::Liquid(Liquid {
                        volume: flow,
                        ..*parameters
                    }),
                    ..pixel.clone()
                })
            },
            PhysicsType::Liquid(top_parameters) => {
                if pixel.material_id == top_cell.material_id && top_parameters.volume < parameters.volume {
                    let flow = ((top_parameters.volume + parameters.volume) / 2.0 - top_parameters.volume) / 2.0;
                    parameters.volume -= flow;

                    api.set(0, 1, Pixel{
                        matter_type: PhysicsType::Liquid(Liquid {
                            volume: top_parameters.volume + flow,
                            ..top_parameters
                        }),
                        ..top_cell.clone()
                    })
                }
            },
            // PhysicsType::Static => {
            //     let dx = api.rand_dir();

            //     for dx in [dx, -dx] {
            //         let dx_cell = api.get(dx, 1);

            //         match dx_cell.matter_type {
            //             PhysicsType::Empty | PhysicsType::Gas => {
            //                 api.swap(dx, 1);
            //                 break;
            //             },
            //             PhysicsType::Liquid(..) => {
            //                 if pixel.material_id != dx_cell.material_id {
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
            api.update(Pixel::default());
            return;
        }

        let max_flow = f32::min(parameters.volume / 3.0, 1.0);
        let mut exit_flag = false;

        for (dx, dy) in vec![dx, -dx].into_iter().zip(dy_s.iter_mut()) {
            let mut dx_cell = api.get(dx * offset, *dy);

            match &mut dx_cell.matter_type {
                PhysicsType::Empty => {
                    let flow = max_flow / 2.0;
                    flows += 1;
                    parameters.volume -= flow;

                    if api.get(dx * offset, *dy - 1).matter_type == PhysicsType::Empty {
                        api.set(dx * offset, *dy - 1, Pixel{
                            updated_at: pixel.updated_at.saturating_sub(1),
                            matter_type: PhysicsType::Liquid(Liquid{
                                volume: flow,
                                ..parameters.clone()
                            }),
                            ..pixel.clone()
                        });
                        *dy -= 1;
                    }
                    else {
                        api.set(dx * offset, *dy, Pixel{
                            updated_at: pixel.updated_at.saturating_sub(1),
                            matter_type: PhysicsType::Liquid(Liquid{
                                volume: flow,
                                ..parameters.clone()
                            }),
                            ..pixel.clone()
                        })
                    }
                },
                PhysicsType::Liquid(dx_parameters) => {
                    if pixel.material_id == dx_cell.material_id {
                        let flow = [parameters.volume - dx_parameters.volume, max_flow / 2.0, 1.0]
                            .into_iter()
                            .reduce(|a, b| f32::min(a, b))
                            .unwrap();

                        if dx_parameters.volume < parameters.volume {
                            parameters.volume -= flow;

                            flows += 1;
                            api.set(dx * offset, *dy, Pixel{
                                matter_type: PhysicsType::Liquid(Liquid{
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

                            if offset_cell.matter_type == PhysicsType::Empty {
                                api.set(dx_next, dy_next, Pixel{
                                    updated_at: dx_cell.updated_at.saturating_sub(1),
                                    ..dx_cell.clone()
                                });
                            }
                            else if offset_cell.material_id == dx_cell.material_id {
                                let PhysicsType::Liquid(offset_parameters) = offset_cell.matter_type else {
                                    panic!();
                                };

                                let maximum_receive_flow = f32::clamp(offset_parameters.max_compression - offset_parameters.volume, 0.0, 1.0);
                                if maximum_receive_flow > dx_parameters.volume {
                                    api.set(dx_next, dy_next, Pixel{
                                        matter_type: PhysicsType::Liquid(Liquid{
                                            volume: offset_parameters.volume + dx_parameters.volume,
                                            ..offset_parameters.clone()
                                        }),
                                        ..offset_cell.clone()
                                    })
                                }
                                else {
                                    let flow_to_avg = (dx_parameters.volume + offset_parameters.volume) / 2.0 - offset_parameters.volume;

                                    dx_parameters.volume -= flow_to_avg;

                                    api.set(dx_next, dy_next, Pixel{
                                        matter_type: PhysicsType::Liquid(Liquid{
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
                            api.set(dx * offset, *dy, Pixel{
                                updated_at: pixel.updated_at.saturating_sub(1),
                                matter_type: PhysicsType::Liquid(Liquid{
                                    volume: flow,
                                    ..parameters.clone()
                                }),
                                ..pixel.clone()
                            });

                            succesfully_moved = true;
                            break;
                        }

                        if !succesfully_moved {
                            exit_flag = true;

                            api.set(dx * offset, *dy, Pixel{
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

    api.update(Pixel {
        matter_type: PhysicsType::Liquid(Liquid {
            ..parameters.clone()
        }),
        ..pixel
    });
}

pub fn update_gas(api: &mut ChunkApi) {
    let mut pixel = api.get(0, 0);
    let mut dx = api.rand_dir();

    if matches!(api.get(dx, 0).matter_type, PhysicsType::Empty | PhysicsType::Gas{..}) && matches!(api.get(dx, 1).matter_type, PhysicsType::Empty | PhysicsType::Gas{..}) {
        api.swap(dx, 0);
    }
    else if matches!(api.get(-dx, 0).matter_type, PhysicsType::Empty | PhysicsType::Gas{..}) && matches!(api.get(-dx, 1).matter_type, PhysicsType::Empty | PhysicsType::Gas{..}) {
        api.swap(-dx, 0);
    }

    if matches!(api.get(0, 1).matter_type, PhysicsType::Empty | PhysicsType::Gas{..}) {
        api.swap(0, 1);
        if api.once_in(20) {
            //randomize direction when falling sometimes
            pixel.ra = api.rand_int(20) as u8;
        }

        api.update(pixel);
        return
    }

    dx = if pixel.ra % 2 == 0 { 1 } else { -1 };

    let dx0 = api.get(dx, 0);
    let dxd = api.get(dx * 2, 0);

    if dx0.matter_type == PhysicsType::Empty && dxd.matter_type == PhysicsType::Empty {
        // scoot double
        pixel.rb = 6;
        api.swap(dx * 2, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if matches!(nbr.matter_type, PhysicsType::Liquid{..}) && nbr.ra % 2 != pixel.ra % 2 {
            api.set(dx, dy,
                Pixel {
                    ra: pixel.ra,
                    ..pixel.clone()
                },
            )
        }
    } else if dx0.matter_type == PhysicsType::Empty {
        pixel.rb = 3;
        api.swap(dx, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if matches!(nbr.matter_type, PhysicsType::Liquid{..}) && nbr.ra % 2 != pixel.ra % 2 {
            api.set(
                dx,
                dy,
                Pixel {
                    ra: pixel.ra,
                    ..pixel.clone()
                },
            )
        }
    } else if pixel.rb == 0 {
        if matches!(api.get(-dx, 0).matter_type, PhysicsType::Empty) {
            // bump
            pixel.ra = ((pixel.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        pixel.rb -= 1;
    }

    api.update(pixel);
}

