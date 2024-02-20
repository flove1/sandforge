// use std::{default, hash::Hash};

// use crate::materials::{FireParameters, Material, PhysicsType, Reaction};
// use bevy::{prelude::*, utils::HashMap};
// use noise::core::value;
// use serde_yaml::Value;

// pub struct Registries {
//     pub reactive_materials: Vec<String>,
//     pub materials: Registry<RegistryID, Material>,
//     pub reactions: Registry<ReactionID, Reaction>
//     // pub material_placers: MaterialPlacerRegistry,
//     // pub structure_pieces: StructurePieceRegistry,
//     // pub structure_pools: StructurePoolRegistry,
//     // pub configured_structures: ConfiguredStructureRegistry,
//     // pub structure_sets: StructureSetRegistry,
//     // pub biomes: BiomeRegistry,
// }

// impl FromWorld for Registries {
//     fn from_world(world: &mut World) -> Self {
//         let mut materials = Registry::default();
//         let mut reactions = Registry::default();

//         materials.register("air", Material::default());
//         materials.register("grass", Material {
//             id: "grass".to_string(),
//             matter_type: PhysicsType::Static,
//             color: [0x7d, 0xaa, 0x4d, 0xff],
//             // color_offset: 10,
//             fire_parameters: Some(FireParameters {
//                 fire_temperature: 125,
//                 ignition_temperature: 75,
//                 fire_hp: 25,
//             }),
//         });
//         materials.register("dirt", Material {
//             id: "dirt".to_string(),
//             matter_type: PhysicsType::Static,
//             color: [0x6d, 0x5f, 0x3d, 0xff],
//             // color_offset: 10,
//             fire_parameters: None,
//         });
//         materials.register("stone", Material {
//             id: "stone".to_string(),
//             matter_type: PhysicsType::Static,
//             color: [0x71, 0x77, 0x77, 0xff],
//             // color_offset: 25,
//             fire_parameters: None,
//         });
//         materials.register("actor", Material {
//             id: "actor".to_string(),
//             matter_type: PhysicsType::Static,
//             color: [0xff, 0x00, 0x00, 0x50],
//             // color_offset: 0,
//             fire_parameters: None,
//         });

//         let parsed_yaml: Value = serde_yaml::from_str(&std::fs::read_to_string("materials.yaml").unwrap()).unwrap();

//         parsed_yaml
//             .as_sequence()
//             .unwrap()
//             .iter()
//             .filter_map(|item| serde_yaml::from_value(item.clone()).ok())
//             .for_each(|material: Material| {
//                 materials.register(material.id, material);
//             });

//         parsed_yaml
//             .as_sequence()
//             .unwrap()
//             .iter()
//             .filter_map(|item| serde_yaml::from_value(item.clone()).ok())
//             .for_each(|reaction: Reaction| {
//                 reaction


//                 if !reactions.contains_key(&reaction.input_element_1) {
//                     REACTIONS.insert(
//                         reaction.input_element_1.clone(),
//                         DashMap::with_hasher(ahash::RandomState::new()),
//                     );
//                 }

//                 REACTIONS
//                     .get_mut(reaction.input_element_1.as_str())
//                     .unwrap()
//                     .insert(reaction.input_element_2.clone(), reaction);
//             });

//         // REACTIONS.clear();

//         // reactions.into_iter().for_each(|reaction| {
//         //     if !REACTIONS.contains_key(&reaction.input_element_1) {
//         //         REACTIONS.insert(
//         //             reaction.input_element_1.clone(),
//         //             DashMap::with_hasher(ahash::RandomState::new()),
//         //         );
//         //     }

//         //     REACTIONS
//         //         .get_mut(reaction.input_element_1.as_str())
//         //         .unwrap()
//         //         .insert(reaction.input_element_2.clone(), reaction);
//         // });

//         let q = materials.get("fs");

//         Self {
//             materials,
//             reactions,
//             reactive_materials: vec![],
//             // reactions
//         }
//     }
// }

// #[derive(Default)]
// pub struct Registry<K: Hash+Eq, V> {
//     map: HashMap<K, V>,
// }

// impl<K: Hash+Eq, V> Registry<K, V> {
//     pub fn register(&mut self, key: impl Into<K>, value: V) {
//         self.map.insert(key.into(), value);
//     }

//     pub fn get(&self, key: impl Into<K>) -> Option<&V> {
//         self.map.get(&key.into())
//     }
// }

// #[derive(Default, Hash, PartialEq, Eq)]
// pub struct RegistryID {
//     pub id: String,
// }

// impl<S: Into<String>> From<S> for RegistryID{
//     fn from(value: S) -> Self {
//         Self {
//             id: value.into().to_lowercase()
//         }
//     }
// }

// #[derive(Default, Hash, PartialEq, Eq)]
// pub struct ReactionID {
//     pub element_1: String,
//     pub element_2: String,
// }


// impl<S: Into<String>> From<(S, S)> for ReactionID{
//     fn from(value: (S, S)) -> Self {
//         let value = (value.0.into().to_lowercase(), value.1.into().to_lowercase());

//         //To keep order consistent
//         Self {
//             element_1: String::min(value.0, value.1),
//             element_2: String::max(value.0, value.1),
//         }
//     }
// }

// pub type ReactionRegistry = Registry<ReactionID, Reaction>;

// impl ReactionRegistry {
    
// }