use bevy::{ prelude::*, utils::{ HashMap, HashSet } };
use ron::Value;
use crate::{ generation::level::Level, simulation::materials::{ Material, Reaction } };

#[derive(Resource)]
pub struct Registries {
    pub reactive_materials: HashSet<String>,
    pub materials: HashMap<String, Material>,
    pub levels: Vec<Level>,
}

impl FromWorld for Registries {
    fn from_world(_world: &mut World) -> Self {
        let mut materials = HashMap::new();
        let mut reactive_materials = HashSet::new();

        materials.insert("air".to_string(), Material::default());

        ron::de
            ::from_str::<Vec<Material>>(&std::fs::read_to_string("materials.ron").unwrap())
            .unwrap()
            .into_iter()
            .for_each(|material| {
                materials.insert(material.id.clone(), material);
            });

        ron::de
            ::from_str::<Vec<Reaction>>(&std::fs::read_to_string("reactions.ron").unwrap())
            .unwrap()
            .into_iter()
            .for_each(|reaction| {
                reactive_materials.insert(reaction.input_material_1.clone());

                materials.entry(reaction.input_material_1.clone()).and_modify(|material| {
                    material.reactions
                        .get_or_insert(HashMap::default())
                        .insert(reaction.input_material_2.clone(), reaction);
                });
            });

        let levels = ron::de
            ::from_str::<Vec<Level>>(&std::fs::read_to_string("levels.ron").unwrap())
            .unwrap();

        Self {
            materials,
            reactive_materials,
            levels,
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
