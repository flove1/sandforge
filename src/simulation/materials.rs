use bevy::{ prelude::Entity, reflect::Reflect, utils::{ HashMap, HashSet } };
use bevy_math::IVec2;
use serde::{ Deserialize, Serialize };

use super::{ chunk::ChunkApi, pixel::Pixel };

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Material {
    pub id: String,
    pub ui_name: String,

    pub physics_type: PhysicsType,

    pub color: [u8; 4],
    pub color_offset: u8,

    #[serde(default)]
    pub durability: Option<f32>,

    #[serde(default)]
    pub lighting: Option<[u8; 4]>,

    #[serde(default)]
    pub fire: Option<Fire>,

    #[serde(default)]
    pub reactions: Option<HashMap<String, Reaction>>,

    #[serde(default)]
    pub contact: Option<ContactEffect>,

    #[serde(default)]
    pub tags: HashSet<String>,
}

#[derive(Reflect, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum ContactEffect {
    Heal(f32),
    Damage(f32),
    Explode{
        radius: f32,
        damage: f32,
        force: f32,
    },
    Transistion(f32, String),
}

#[derive(Reflect, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Fire {
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
    Gas(Gas),
    Rigidbody(Entity),
}

#[derive(Reflect, Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub struct Gas {
    #[serde(default = "default_dissipation")]
    pub dissipate: i32,

    pub density: u8,
}

#[derive(Reflect, Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub struct Liquid {
    #[serde(default)]
    pub inertion: u8,

    #[serde(default = "default_dir")]
    pub direction: i32,

    pub flow_rate: u8,
    pub density: u8,
}

fn default_dissipation() -> i32 {
    -1
}

fn default_dir() -> i32 {
    if fastrand::bool() { 1 } else { -1 }
}

impl ToString for PhysicsType {
    fn to_string(&self) -> String {
        match self {
            PhysicsType::Air => "Air".to_string(),
            PhysicsType::Static { .. } => "Static".to_string(),
            PhysicsType::Powder { .. } => "Powder".to_string(),
            PhysicsType::Liquid { .. } => "Liquid".to_string(),
            PhysicsType::Gas { .. } => "Gas".to_string(),
            PhysicsType::Rigidbody { .. }  => "Rigidbody".to_string(),
        }
    }
}

impl Default for PhysicsType {
    fn default() -> Self {
        Self::Air
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            id: "air".into(),
            ui_name: "Air".into(),
            physics_type: PhysicsType::Air,
            color: [0; 4],
            color_offset: 0,
            reactions: None,
            lighting: None,
            fire: None,
            contact: None,
            durability: None,
            tags: HashSet::new(),
        }
    }
}

const FOUR_DIRECTIONS: [IVec2; 4] = [
    IVec2::new(-1, 0),
    IVec2::new(0, -1),
    IVec2::new(1, 0),
    IVec2::new(0, 1),
];

const EIGHT_DIRECTIONS: [IVec2; 8] = [
    IVec2::new(-1, -1),
    IVec2::new(0, -1),
    IVec2::new(1, -1),
    IVec2::new(-1, 0),
    IVec2::new(1, 0),
    IVec2::new(-1, 1),
    IVec2::new(0, 1),
    IVec2::new(1, 1),
];

pub fn update_fire(api: &mut ChunkApi) -> bool {
    let mut pixel = api.get(0, 0);

    let Some(fire_parameters) = pixel.fire_parameters.as_mut() else {
        return false;
    };

    if fire_parameters.try_to_ignite {
        if
            !fire_parameters.requires_oxygen ||
            EIGHT_DIRECTIONS.iter().any(|direction| {
                let neighbour = api.get(direction.x, direction.y);

                neighbour.is_empty()
            })
        {
            if fire_parameters.probability > fastrand::f32() {
                pixel.on_fire = true;
            }
        }
    }

    if pixel.on_fire {
        api.keep_alive(0, 0);

        let Some(fire_parameters) = pixel.fire_parameters.as_mut() else { panic!() };

        let mut has_access_to_air = false;

        for direction in EIGHT_DIRECTIONS.iter() {
            let mut pixel = api.get(direction.x, direction.y);

            if pixel.is_empty() {
                has_access_to_air = true;
            } else if let Some(fire_parameters) = &mut pixel.fire_parameters {
                fire_parameters.try_to_ignite = true;
                api.set(direction.x, direction.y, pixel);
            }
        }

        if fire_parameters.requires_oxygen && !has_access_to_air {
            pixel.on_fire = false;
        } else if fire_parameters.fire_hp <= 0.0 {
            api.update(Pixel::default());
            if pixel.physics_type == PhysicsType::Static {
                api.collider_changed(0, 0);
            }

            return true;
        } else if fastrand::f32() > 0.75 {
            fire_parameters.fire_hp -= 1.0;
        }

        api.update(pixel);
    }

    false
}

pub fn update_reactions(api: &mut ChunkApi, materials: &HashMap<String, Material>) {
    let pixel = api.get(0, 0);

    let Some(reactions) = pixel.material.reactions else {
        return;
    };

    let non_empty_reaction = reactions.get("any");

    for offset in EIGHT_DIRECTIONS.iter() {
        let neighbour = api.get(offset.x, offset.y);

        if neighbour.is_empty() || neighbour.material.id == pixel.material.id {
            continue;
        }

        let reaction = match reactions.get(&api.get(offset.x, offset.y).material.id) {
            Some(reaction) => reaction,
            None => {
                if non_empty_reaction.is_none() {
                    continue;
                }

                non_empty_reaction.unwrap()
            }
        };

        if fastrand::f32() < reaction.probability {
            let result_1 = materials.get(&reaction.output_material_1).unwrap();
            let result_2 = materials.get(&reaction.output_material_2).unwrap();

            if
                pixel.physics_type == PhysicsType::Static &&
                result_1.physics_type != PhysicsType::Static
            {
                api.collider_changed(0, 0);
            }

            if
                neighbour.physics_type == PhysicsType::Static &&
                result_2.physics_type != PhysicsType::Static
            {
                api.collider_changed(offset.x, offset.y);
            }

            api.set(0, 0, Pixel::from(result_1).with_clock(api.clock));
            api.set(offset.x, offset.y, Pixel::from(result_2).with_clock(api.clock));

            return;
        }
    }
}

pub fn update_powder(api: &mut ChunkApi) {
    let dx = api.rand_dir();
    let is_empty = |physics_type|
        matches!(physics_type, PhysicsType::Air | PhysicsType::Gas { .. });

    if is_empty(api.get_physics_type(0, -1)) {
        api.swap(0, -1);

        if api.once_in(5) && is_empty(api.get_physics_type(dx, 0)) {
            api.swap(dx, 0);
        }
    } else if is_empty(api.get_physics_type(dx, 0)) && is_empty(api.get_physics_type(dx, -1)) {
        api.swap(dx, -1);
    } else if is_empty(api.get_physics_type(-dx, 0)) && is_empty(api.get_physics_type(-dx, -1)) {
        api.swap(-dx, -1);
    } else if matches!(api.get_physics_type(0, -1), PhysicsType::Liquid { .. }) {
        api.swap(0, -1);

        if api.once_in(15) && matches!(api.get_physics_type(dx, 0), PhysicsType::Liquid { .. }) {
            api.swap(dx, 0);
        }
    }
}

pub fn update_liquid(api: &mut ChunkApi) {
    let mut pixel = api.get(0, 0);
    let PhysicsType::Liquid(parameters) = &mut pixel.physics_type else {
        panic!();
    };

    let check_if_empty = |parameters: &Liquid, pixel: Pixel| -> bool {
        match pixel.physics_type {
            PhysicsType::Air | PhysicsType::Gas(..) => true,
            PhysicsType::Liquid(other_parameters) => parameters.density > other_parameters.density,
            _ => false,
        }
    };

    if
        FOUR_DIRECTIONS.into_iter().any(|offset|
            check_if_empty(parameters, api.get(offset.x, offset.y))
        )
    {
        api.keep_alive(0, 0);
    }

    if check_if_empty(parameters, api.get(0, -1)) {
        api.swap(0, -1);
        if api.once_in(20) {
            parameters.direction = api.rand_dir();
        }

        if fastrand::f32() < 0.75 {
            api.update(pixel);
            return;
        }
    }

    for _ in 0..parameters.flow_rate {
        if check_if_empty(parameters, api.get(0, -1)) {
            break;
        }

        if !check_if_empty(parameters, api.get(parameters.direction, 0)) {
            parameters.inertion = parameters.inertion.saturating_sub(1);
            if parameters.inertion == 0 {
                parameters.direction = -parameters.direction;
                parameters.inertion = 3;
            }
            break;
        }

        api.swap(parameters.direction, 0);

        for _ in 0..(parameters.flow_rate as f32).sqrt().max(1.0) as i32 {
            if !check_if_empty(parameters, api.get(0, -1)) {
                break;
            }

            api.swap(0, -1);
        }
    }

    api.update(pixel);
}

pub fn update_gas(api: &mut ChunkApi) {
    let mut pixel = api.get(0, 0);
    let PhysicsType::Gas(parameters) = &mut pixel.physics_type else {
        panic!();
    };

    match parameters.dissipate {
        -1 => {}
        0 => {
            api.update(Pixel::default());
            return;
        }
        _ => {
            if fastrand::bool() {
                parameters.dissipate -= 1;
            }
        }
    }

    let check_if_empty = |parameters: &Gas, pixel: Pixel| -> bool {
        match pixel.physics_type {
            PhysicsType::Air => true,
            PhysicsType::Gas(other_parameters) => parameters.density > other_parameters.density,
            _ => false,
        }
    };

    if
        FOUR_DIRECTIONS.into_iter().any(|offset|
            check_if_empty(parameters, api.get(offset.x, offset.y))
        )
    {
        api.keep_alive(0, 0);
    }

    let direction = api.rand_dir();

    if
        check_if_empty(parameters, api.get(direction, 0)) &&
        check_if_empty(parameters, api.get(direction, 1))
    {
        api.swap(direction, 0);
    } else if
        check_if_empty(parameters, api.get(-direction, 0)) &&
        check_if_empty(parameters, api.get(-direction, 1))
    {
        api.swap(-direction, 0);
    }

    if check_if_empty(parameters, api.get(0, 1)) {
        api.swap(0, 1);
        api.update(pixel);
        return;
    }

    for _ in 0..3 {
        if !check_if_empty(parameters, api.get(direction, 0)) {
            break;
        }

        api.swap(direction, 0);
    }

    api.update(pixel);
}
