use bevy::{
    prelude::*,
    utils::{hashbrown::hash_map::Values, HashMap, HashSet},
};
use serde_yaml::Value;

use crate::{generation::biome::Biome, simulation::materials::{FireParameters, Material, PhysicsType}};

// TODO: load as asset
#[derive(Resource)]
pub struct Registries {
    pub reactive_materials: HashSet<String>,
    pub materials: Registry<Material>, 
    pub biomes: Registry<Biome>
}

impl FromWorld for Registries {
    fn from_world(_world: &mut World) -> Self {
        let mut materials = Registry::default();

        materials.register("air", Material::default());
        materials.register(
            "grass",
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
                ..Default::default()
            },
        );
        materials.register(
            "dirt",
            Material {
                id: "dirt".to_string(),
                matter_type: PhysicsType::Static,
                color: [0x6d, 0x5f, 0x3d, 0xff],
                color_offset: 10,
                fire_parameters: None,
                ..Default::default()
            },
        );
        materials.register(
            "stone",
            Material {
                id: "stone".to_string(),
                matter_type: PhysicsType::Static,
                color: [0x71, 0x77, 0x77, 0xff],
                color_offset: 25,
                fire_parameters: None,
                ..Default::default()
            },
        );
        materials.register(
            "actor",
            Material {
                id: "actor".to_string(),
                matter_type: PhysicsType::Static,
                color: [0xff, 0x00, 0x00, 0x50],
                color_offset: 0,
                fire_parameters: None,
                ..Default::default()
            },
        );

        let parsed_yaml: Value =
            serde_yaml::from_str(&std::fs::read_to_string("materials.yaml").unwrap()).unwrap();

        parsed_yaml
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|item| serde_yaml::from_value(item.clone()).ok())
            .for_each(|material: Material| {
                materials.register(material.id.clone(), material);
            });

        let reactive_materials = materials
            .map
            .values()
            .filter(|material| material.reactions.is_some())
            .map(|material| material.id.clone())
            .collect();

        Self {
            materials,
            reactive_materials,
            biomes: Registry::default()
        }
    }
}

#[derive(Default)]
pub struct Registry<V> {
    map: HashMap<RegistryID, V>,
}

impl<V> Registry<V> {
    pub fn register(&mut self, key: impl Into<String>, value: V) {
        self.map.insert(RegistryID::from(key), value);
    }

    pub fn get(&self, key: impl Into<String>) -> Option<&V> {
        self.map.get(&RegistryID::from(key))
    }

    pub fn iter(&self) -> Values<'_, RegistryID, V> {
        self.map.values()
    }
}

#[derive(Default, Hash, PartialEq, Eq)]
pub struct RegistryID {
    pub id: String,
}

impl<S: Into<String>> From<S> for RegistryID {
    fn from(value: S) -> Self {
        Self {
            id: value.into().to_lowercase(),
        }
    }
}
