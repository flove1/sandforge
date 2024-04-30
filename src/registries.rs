use std::sync::{ Arc, Mutex };

use bevy::{ prelude::*, utils::{ HashMap, HashSet } };
use bevy_egui::egui::mutex::RwLock;
use serde_yaml::Value;

use crate::{
    generation::biome::Biome,
    simulation::materials::{ FireParameters, Material, PhysicsType, Reaction },
};

#[derive(Resource)]
pub struct Registries {
    pub reactive_materials: Arc<RwLock<HashSet<String>>>,
    pub materials: Arc<RwLock<HashMap<String, Material>>>,
    pub biomes: Arc<RwLock<HashMap<String, Biome>>>,
}

impl FromWorld for Registries {
    fn from_world(_world: &mut World) -> Self {
        let mut materials = HashMap::new();
        let mut reactive_materials = HashSet::new();

        materials.insert("air".to_string(), Material::default());
        materials.insert("grass".to_string(), Material {
            id: "grass".to_string(),
            matter_type: PhysicsType::Static,
            color: [0x7d, 0xaa, 0x4d, 0xff],
            color_offset: 10,
            fire_parameters: Some(FireParameters {
                fire_temperature: 125,
                ignition_temperature: 75,
                fire_hp: 25,
            }),
            ..Default::default()
        });
        materials.insert("dirt".to_string(), Material {
            id: "dirt".to_string(),
            matter_type: PhysicsType::Static,
            color: [0x6d, 0x5f, 0x3d, 0xff],
            color_offset: 10,
            fire_parameters: None,
            ..Default::default()
        });
        materials.insert("stone".to_string(), Material {
            id: "stone".to_string(),
            matter_type: PhysicsType::Static,
            color: [0x71, 0x77, 0x77, 0xff],
            color_offset: 25,
            fire_parameters: None,
            ..Default::default()
        });

        let parsed_yaml: Value = serde_yaml
            ::from_str(&std::fs::read_to_string("materials.yaml").unwrap())
            .unwrap();

        parsed_yaml
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|item| serde_yaml::from_value(item.clone()).ok())
            .for_each(|material: Material| {
                materials.insert(material.id.clone(), material);
            });

        parsed_yaml
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|item| serde_yaml::from_value(item.clone()).ok())
            .for_each(|reaction: Reaction| {
                reactive_materials.insert(reaction.input_material_1.clone());

                materials.entry(reaction.input_material_1.clone()).and_modify(|material| {
                    material.reactions
                        .get_or_insert(HashMap::default())
                        .insert(reaction.input_material_2.clone(), reaction);
                });
            });

        Self {
            materials: Arc::new(RwLock::new(materials)),
            reactive_materials: Arc::new(RwLock::new(reactive_materials)),
            biomes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// #[derive(Default)]
// pub type Registry<V> = HashMap<RegistryID, V>;

// #[derive(Default)]
// pub struct Registry<V> {
//     map: HashMap<RegistryID, V>,
// }

// impl<V> Registry<V> {
//     pub fn register(&mut self, key: impl Into<String>, value: V) {
//         self.map.insert(RegistryID::from(key), value);
//     }

//     pub fn get(&self, key: impl Into<String>) -> Option<&V> {
//         self.map.get(&RegistryID::from(key))
//     }

//     pub fn iter(&self) -> Values<'_, RegistryID, V> {
//         self.map.values()
//     }
// }

// #[derive(Default, Hash, PartialEq, Eq)]
// pub struct RegistryID {
//     pub id: String,
// }

// impl<S: Into<String>> From<S> for RegistryID {
//     fn from(value: S) -> Self {
//         Self {
//             id: value.into().to_lowercase(),
//         }
//     }
// }
