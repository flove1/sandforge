use bevy::{ reflect::Reflect, utils::HashMap };
use bevy_math::{ IVec2, Vec2 };
use serde::{ Deserialize, Serialize };

use super::{ chunk::ChunkApi, pixel::Pixel };

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Material {
    pub id: String,
    pub physics_type: PhysicsType,
    pub color: [u8; 4],
    pub color_offset: u8,

    pub temperarure: f32,

    #[serde(default)]
    pub fire_parameters: Option<FireParameters>,

    #[serde(default)]
    pub hot_transistion_parameters: Option<HotTransistionParameters>,

    #[serde(default)]
    pub cold_transistion_parameters: Option<ColdTransistionParameters>,

    #[serde(default)]
    pub reactions: Option<HashMap<String, Reaction>>,

    #[serde(default)]
    pub radiating_temperature: Option<f32>
}

#[derive(Reflect, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct HotTransistionParameters {
    pub threshold: f32,
    pub output_id: String
}

#[derive(Reflect, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ColdTransistionParameters {
    pub threshold: f32,
    pub output_id: String
}

#[derive(Reflect, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct FireParameters {
    pub probability: f32,
    pub fire_hp: f32,
    pub requires_oxygen: bool,

    #[serde(default)]
    pub try_to_ignite: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Reaction {
    pub probability: f32,
    pub input_material_1: String,
    pub input_material_2: String,
    pub output_material_1: String,
    pub output_material_2: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum PhysicsType {
    Air,
    Static,
    Powder,
    Liquid(Liquid),
    Gas,
    Rigidbody,
    Disturbed(Box<PhysicsType>),
}

#[derive(Reflect, Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub struct Liquid {
    pub flow_rate: u8,
}

impl ToString for PhysicsType {
    fn to_string(&self) -> String {
        match self {
            PhysicsType::Air => "Air".to_string(),
            PhysicsType::Static { .. } => "Static".to_string(),
            PhysicsType::Powder { .. } => "Powder".to_string(),
            PhysicsType::Liquid { .. } => "Liquid".to_string(),
            PhysicsType::Gas { .. } => "Gas".to_string(),
            PhysicsType::Rigidbody => "Rigidbody".to_string(),
            PhysicsType::Disturbed(..) => "Disturbed material".to_string(),
        }
    }
}

impl Default for PhysicsType {
    fn default() -> Self {
        Self::Air
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
            physics_type: PhysicsType::Air,
            fire_parameters: None,
            color: [0; 4],
            color_offset: 0,
            reactions: None,
            radiating_temperature: None,
            hot_transistion_parameters: None,
            cold_transistion_parameters: None,
            temperarure: 36.0,
        }
    }
}

// pub fn update_force(api: &mut ChunkApi) {
//     let mut pixel = api.get(0, 0);

//     let PhysicsType::Force(position, velocity) = &mut pixel.material.physics_type else {
//         unreachable!();
//     };

//     let delta = *position + *velocity;

//     for point in WalkGrid::new(IVec2::ZERO, (delta.signum() * delta.abs().ceil()).as_ivec2()) {
//         let neighbor = api.get(point.x, point.y);

//         match neighbor.material.physics_type {
//             PhysicsType::Air => todo!(),
//             PhysicsType::Static => api.update(pixel.with_material(pixel.material.)),
//             PhysicsType::Powder => todo!(),
//             PhysicsType::Liquid(_) => todo!(),
//             PhysicsType::Gas => todo!(),
//             PhysicsType::Rigidbody => todo!(),
//             PhysicsType::Actor => todo!(),
//             PhysicsType::Force(_, _) => todo!(),
//             PhysicsType::Disturbed(_) => todo!(),
//         }

//         if matches!(neighbor.material.physics_type, PhysicsType::Air | PhysicsType::Gas) {
//             continue;
//         }
//         else {
//             *velocity /= 2.0;
//         }

//     }
// }

pub fn update_sand(api: &mut ChunkApi) {
    let dx = api.rand_dir();

    if matches!(api.get_physics_type(0, -1), PhysicsType::Air | PhysicsType::Gas { .. }) {
        if
            api.once_in(5) &&
            matches!(api.get_physics_type(dx, -1), PhysicsType::Air | PhysicsType::Gas { .. })
        {
            api.swap(dx, -1);
        } else {
            api.swap(0, -1);
        }
    } else if matches!(api.get_physics_type(dx, -1), PhysicsType::Air | PhysicsType::Gas { .. }) {
        api.swap(dx, -1);
    } else if matches!(api.get_physics_type(-dx, -1), PhysicsType::Air | PhysicsType::Gas { .. }) {
        api.swap(-dx, -1);
    } else if matches!(api.get_physics_type(0, -1), PhysicsType::Liquid { .. }) {
        if
            api.once_in(30) &&
            matches!(
                api.get_physics_type(dx, -1),
                PhysicsType::Air | PhysicsType::Gas { .. } | PhysicsType::Liquid { .. }
            )
        {
            api.swap(dx, -1);
        } else {
            api.swap(0, -1);
        }
    }
}

pub fn update_liquid(api: &mut ChunkApi) {
    let mut pixel = api.get(0, 0);
    let PhysicsType::Liquid(parameters) = &mut pixel.physics_type.clone() else {
        panic!();
    };

    if
        [IVec2::new(0, -1), IVec2::new(0, 1), IVec2::new(-1, 0), IVec2::new(1, 0)]
            .into_iter()
            .any(|offset| api.is_empty(offset.x, offset.y))
    {
        api.keep_alive(0, 0);
    }

    if matches!(api.get(0, -1).physics_type, PhysicsType::Air) {
        api.swap(0, -1);
        if api.once_in(20) {
            pixel.ra = api.rand_int(20) as u8;
        }

        api.update(pixel);
        return;
    }

    let direction = if pixel.ra % 2 == 0 { 1 } else { -1 };

    for _ in 0..(parameters.flow_rate) {
        if api.is_empty(0, -1) {
            break;
        }

        if !api.is_empty(direction, 0) {
            pixel.rb = pixel.rb.saturating_sub(1);
            if pixel.rb == 0 {
                pixel.ra = api.rand_int(20) as u8;
                pixel.rb = 4;
            }
            break;
        }

        api.swap(direction, 0);

        for _ in 0..((parameters.flow_rate as f32).sqrt().max(1.0) as i32) {
            if !api.get(0, -1).is_empty() {
                break;
            }

            api.swap(0, -1);
        }

    }

    api.update(pixel);

    // let dx0 = api.get(direction, 0);
    // let dxd = api.get(direction * 2, 0);

    // if
    //     dx0.material.physics_type == PhysicsType::Air &&
    //     dxd.material.physics_type == PhysicsType::Air
    // {
    //     // scoot double
    //     pixel.rb = 6;
    //     api.swap(direction * 2, 0);

    //     let (dx, dy) = api.rand_vec_8();
    //     let nbr = api.get(dx, dy);

    //     // spread opinion
    //     if
    //         matches!(nbr.material.physics_type, PhysicsType::Liquid { .. }) &&
    //         nbr.ra % 2 != pixel.ra % 2
    //     {
    //         api.set(dx, dy, Pixel {
    //             ra: pixel.ra,
    //             ..pixel.clone()
    //         });
    //     }
    // } else if dx0.material.physics_type == PhysicsType::Air {
    //     pixel.rb = 3;
    //     api.swap(direction, 0);

    //     let (dx, dy) = api.rand_vec_8();
    //     let nbr = api.get(dx, dy);

    //     if
    //         matches!(nbr.material.physics_type, PhysicsType::Liquid { .. }) &&
    //         nbr.ra % 2 != pixel.ra % 2
    //     {
    //         api.set(dx, dy, Pixel {
    //             ra: pixel.ra,
    //             ..pixel.clone()
    //         });
    //     }
    // } else if pixel.rb == 0 {
    //     if matches!(api.get(-direction, 0).material.physics_type, PhysicsType::Air) {
    //         // bump
    //         pixel.ra = ((pixel.ra as i32) + direction) as u8;
    //     }
    // } else {
    //     // become less certain (more bumpable)
    //     pixel.rb -= 1;
    // }

    // let mut pixel = api.get(0, 0);
    // let PhysicsType::Liquid(parameters) = &mut pixel.material.physics_type.clone() else {
    //     panic!();
    // };

    // let mut bottom_cell = api.get(0, -1);
    // match &mut bottom_cell.material.physics_type {
    //     PhysicsType::Air | PhysicsType::Gas => {
    //         if api.once_in(5) {
    //             pixel.ra = api.rand_int(20) as u8;
    //             api.update(pixel.clone());
    //         }

    //         // let dx = ((pixel.ra % 2) as i32) * 2 - 1;

    //         // if api.once_in(10) && matches!(api.get(dx, -1).matter_type, PhysicsType::Empty | PhysicsType::Gas{..}) {
    //         //     api.swap(dx, -1);
    //         // }
    //         // else {
    //         api.swap(0, -1);
    //         // }

    //         return;
    //     }
    //     PhysicsType::Liquid(bottom_parameters) => {
    //         if parameters.density > bottom_parameters.density {
    //             api.swap(0, -1);

    //             return;
    //         }

    //         if pixel.material.id == bottom_cell.material.id && bottom_parameters.volume < 1.0 {
    //             let required_flow = 1.0 - bottom_parameters.volume;

    //             if required_flow > parameters.volume {
    //                 api.update(Pixel::default());
    //                 api.set(0, -1, Pixel {
    //                     material: MaterialInstance {
    //                         physics_type: PhysicsType::Liquid(Liquid {
    //                             volume: bottom_parameters.volume + parameters.volume,
    //                             ..*bottom_parameters
    //                         }),
    //                         ..bottom_cell.material.clone()
    //                     },
    //                     ..bottom_cell.clone()
    //                 });

    //                 return;
    //             }

    //             parameters.volume -= required_flow;
    //             bottom_parameters.volume += required_flow;

    //             api.update(Pixel {
    //                 material: MaterialInstance {
    //                     physics_type: PhysicsType::Liquid(Liquid { ..*parameters }),
    //                     ..pixel.material.clone()
    //                 },
    //                 ..pixel.clone()
    //             });

    //             api.set(0, -1, Pixel {
    //                 material: MaterialInstance {
    //                     physics_type: PhysicsType::Liquid(Liquid {
    //                         ..*bottom_parameters
    //                     }),
    //                     ..bottom_cell.material.clone()
    //                 },
    //                 ..bottom_cell.clone()
    //             });
    //         }
    //     }
    //     _ => {}
    // }

    // if parameters.volume > 1.0 {
    //     let top_cell = api.get(0, 1);

    //     match top_cell.material.physics_type {
    //         PhysicsType::Air => {
    //             let flow = parameters.volume / 2.0 - parameters.dry_threshold;
    //             parameters.volume -= flow;

    //             api.set(0, 1, Pixel {
    //                 material: MaterialInstance {
    //                     physics_type: PhysicsType::Liquid(Liquid {
    //                         volume: flow,
    //                         ..*parameters
    //                     }),
    //                     ..pixel.material.clone()
    //                 },
    //                 ..pixel.clone()
    //             });
    //         }
    //         PhysicsType::Liquid(top_parameters) => {
    //             if
    //                 pixel.material.id == top_cell.material.id &&
    //                 top_parameters.volume < parameters.volume
    //             {
    //                 let flow =
    //                     ((top_parameters.volume + parameters.volume) / 2.0 -
    //                         top_parameters.volume) /
    //                     2.0;
    //                 parameters.volume -= flow;

    //                 api.set(0, 1, Pixel {
    //                     material: MaterialInstance {
    //                         physics_type: PhysicsType::Liquid(Liquid {
    //                             volume: top_parameters.volume + flow,
    //                             ..top_parameters
    //                         }),
    //                         ..top_cell.material.clone()
    //                     },
    //                     ..top_cell.clone()
    //                 });
    //             }
    //         }
    //         // PhysicsType::Static => {
    //         //     let dx = api.rand_dir();

    //         //     for dx in [dx, -dx] {
    //         //         let dx_cell = api.get(dx, 1);

    //         //         match dx_cell.matter_type {
    //         //             PhysicsType::Empty | PhysicsType::Gas => {
    //         //                 api.swap(dx, 1);
    //         //                 break;
    //         //             },
    //         //             PhysicsType::Liquid(..) => {
    //         //                 if pixel.material_id != dx_cell.material_id {
    //         //                     api.swap(dx, 1);
    //         //                     break;
    //         //                 }
    //         //             },
    //         //             _ => {},
    //         //         }
    //         //     }
    //         // }
    //         _ => {}
    //     }
    // }

    // let dx = api.rand_dir();
    // let mut dy_s = [0, 0];
    // let mut flows = 0;

    // let modifier = 1.0;
    // let max_offset = i32::clamp(10 - (parameters.viscosity as i32) + 1, 1, 10);

    // 'offset_loop: for offset in 1..=((max_offset as f32) * modifier) as i32 {
    //     if parameters.volume < parameters.dry_threshold {
    //         api.keep_alive(0, 0);
    //         api.update(Pixel::default());
    //         return;
    //     }

    //     let max_flow = f32::min(parameters.volume / 3.0, 1.0);
    //     let mut exit_flag = false;

    //     for (dx, dy) in vec![dx, -dx].into_iter().zip(dy_s.iter_mut()) {
    //         let mut dx_cell = api.get(dx * offset, *dy);

    //         match &mut dx_cell.material.physics_type {
    //             PhysicsType::Air => {
    //                 let flow = max_flow / 2.0;
    //                 flows += 1;
    //                 parameters.volume -= flow;

    //                 if api.get(dx * offset, *dy - 1).material.physics_type == PhysicsType::Air {
    //                     api.set(dx * offset, *dy - 1, Pixel {
    //                         updated_at: pixel.updated_at.saturating_sub(1),
    //                         material: MaterialInstance {
    //                             physics_type: PhysicsType::Liquid(Liquid {
    //                                 volume: flow,
    //                                 ..*parameters
    //                             }),
    //                             ..pixel.material.clone()
    //                         },
    //                         ..pixel.clone()
    //                     });
    //                     *dy -= 1;
    //                 } else {
    //                     api.set(dx * offset, *dy, Pixel {
    //                         updated_at: pixel.updated_at.saturating_sub(1),
    //                         material: MaterialInstance {
    //                             physics_type: PhysicsType::Liquid(Liquid {
    //                                 volume: flow,
    //                                 ..*parameters
    //                             }),
    //                             ..pixel.material.clone()
    //                         },
    //                         ..pixel.clone()
    //                     });
    //                 }
    //             }
    //             PhysicsType::Liquid(dx_parameters) => {
    //                 if pixel.material.id == dx_cell.material.id {
    //                     let flow = [parameters.volume - dx_parameters.volume, max_flow / 2.0, 1.0]
    //                         .into_iter()
    //                         .reduce(f32::min)
    //                         .unwrap();

    //                     if dx_parameters.volume < parameters.volume {
    //                         parameters.volume -= flow;

    //                         flows += 1;
    //                         api.set(dx * offset, *dy, Pixel {
    //                             material: MaterialInstance {
    //                                 physics_type: PhysicsType::Liquid(Liquid {
    //                                     volume: dx_parameters.volume + flow,
    //                                     ..*dx_parameters
    //                                 }),
    //                                 ..dx_cell.material.clone()
    //                             },
    //                             ..dx_cell.clone()
    //                         });
    //                     }
    //                 } else if parameters.density > dx_parameters.density {
    //                     let offset_directions = [
    //                         (dx * (offset + 1), *dy),
    //                         (dx * offset, *dy + 1),
    //                     ];
    //                     let mut succesfully_moved = false;

    //                     for (dx_next, dy_next) in offset_directions {
    //                         let offset_cell = api.get(dx_next, dy_next);

    //                         if offset_cell.material.physics_type == PhysicsType::Air {
    //                             api.set(dx_next, dy_next, Pixel {
    //                                 updated_at: dx_cell.updated_at.saturating_sub(1),
    //                                 ..dx_cell.clone()
    //                             });
    //                         } else if offset_cell.material.id == dx_cell.material.id {
    //                             let PhysicsType::Liquid(offset_parameters) =
    //                                 offset_cell.material.physics_type else {
    //                                 panic!();
    //                             };

    //                             let maximum_receive_flow = f32::clamp(
    //                                 offset_parameters.max_compression - offset_parameters.volume,
    //                                 0.0,
    //                                 1.0
    //                             );
    //                             if maximum_receive_flow > dx_parameters.volume {
    //                                 api.set(dx_next, dy_next, Pixel {
    //                                     material: MaterialInstance {
    //                                         physics_type: PhysicsType::Liquid(Liquid {
    //                                             volume: offset_parameters.volume +
    //                                             dx_parameters.volume,
    //                                             ..offset_parameters
    //                                         }),
    //                                         ..offset_cell.material.clone()
    //                                     },
    //                                     ..offset_cell.clone()
    //                                 });
    //                             } else {
    //                                 let flow_to_avg =
    //                                     (dx_parameters.volume + offset_parameters.volume) / 2.0 -
    //                                     offset_parameters.volume;

    //                                 dx_parameters.volume -= flow_to_avg;

    //                                 api.set(dx_next, dy_next, Pixel {
    //                                     material: MaterialInstance {
    //                                         physics_type: PhysicsType::Liquid(Liquid {
    //                                             volume: offset_parameters.volume + flow_to_avg,
    //                                             ..offset_parameters
    //                                         }),
    //                                         ..offset_cell.material.clone()
    //                                     },
    //                                     ..offset_cell.clone()
    //                                 });

    //                                 continue;
    //                             }
    //                         } else {
    //                             continue;
    //                         }

    //                         let flow = max_flow / 2.0;
    //                         parameters.volume -= flow;

    //                         flows += 1;
    //                         api.set(dx * offset, *dy, Pixel {
    //                             updated_at: pixel.updated_at.saturating_sub(1),
    //                             material: MaterialInstance {
    //                                 physics_type: PhysicsType::Liquid(Liquid {
    //                                     volume: flow,
    //                                     ..*parameters
    //                                 }),
    //                                 ..pixel.material.clone()
    //                             },
    //                             ..pixel.clone()
    //                         });

    //                         succesfully_moved = true;
    //                         break;
    //                     }

    //                     if !succesfully_moved {
    //                         exit_flag = true;

    //                         api.set(dx * offset, *dy, Pixel { ..dx_cell.clone() });
    //                     }
    //                 }
    //             }
    //             _ => {
    //                 exit_flag = true;
    //             }
    //         }
    //     }
    //     if exit_flag {
    //         break 'offset_loop;
    //     }
    // }

    // if flows > 0 {
    //     api.keep_alive(0, 0);
    // }

    // api.update(Pixel {
    //     material: MaterialInstance {
    //         physics_type: PhysicsType::Liquid(Liquid { ..*parameters }),
    //         ..pixel.material.clone()
    //     },
    //     ..pixel
    // });
}

pub fn update_gas(api: &mut ChunkApi) {
    let mut pixel = api.get(0, 0);
    let mut dx = api.rand_dir();

    if
        matches!(api.get(dx, 0).physics_type, PhysicsType::Air) &&
        matches!(api.get(dx, 1).physics_type, PhysicsType::Air)
    {
        api.swap(dx, 0);
    } else if
        matches!(api.get(-dx, 0).physics_type, PhysicsType::Air) &&
        matches!(api.get(-dx, 1).physics_type, PhysicsType::Air)
    {
        api.swap(-dx, 0);
    }

    if matches!(api.get(0, 1).physics_type, PhysicsType::Air) {
        api.swap(0, 1);
        if api.once_in(20) {
            //randomize direction when falling sometimes
            pixel.ra = api.rand_int(20) as u8;
        }

        api.update(pixel);
        return;
    }

    dx = if pixel.ra % 2 == 0 { 1 } else { -1 };

    let dx0 = api.get(dx, 0);
    let dxd = api.get(dx * 2, 0);

    if
        dx0.physics_type == PhysicsType::Air &&
        dxd.physics_type == PhysicsType::Air
    {
        // scoot double
        pixel.rb = 6;
        api.swap(dx * 2, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        // spread opinion
        if
            matches!(nbr.physics_type, PhysicsType::Gas { .. }) &&
            nbr.ra % 2 != pixel.ra % 2
        {
            api.set(dx, dy, Pixel {
                ra: pixel.ra,
                ..pixel.clone()
            });
        }
    } else if dx0.physics_type == PhysicsType::Air {
        pixel.rb = 3;
        api.swap(dx, 0);

        let (dx, dy) = api.rand_vec_8();
        let nbr = api.get(dx, dy);

        if
            matches!(nbr.physics_type, PhysicsType::Gas { .. }) &&
            nbr.ra % 2 != pixel.ra % 2
        {
            api.set(dx, dy, Pixel {
                ra: pixel.ra,
                ..pixel.clone()
            });
        }
    } else if pixel.rb == 0 {
        if matches!(api.get(-dx, 0).physics_type, PhysicsType::Air) {
            // bump
            pixel.ra = ((pixel.ra as i32) + dx) as u8;
        }
    } else {
        // become less certain (more bumpable)
        pixel.rb -= 1;
    }

    api.update(pixel);
}
