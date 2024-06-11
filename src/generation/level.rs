use serde::Deserialize;

use super::noise::NoiseType;

#[derive(Deserialize, Clone)]
pub struct NoiseLayer {
    pub material_id: String,
    pub value: f32,
}

#[derive(Deserialize, Clone)]
pub struct EnemyOnLevel {
    pub enemy_id: String,
    pub frequency: f32,
    pub spawn_chance: f32,
}

#[derive(Deserialize, Clone)]
pub struct Level {
    pub terrain_layers: Vec<NoiseLayer>,
    pub background_layers: Vec<NoiseLayer>,
    pub texture_path: String,
    pub noise_type: NoiseType,
    pub powder_id: String,
    pub liquid_id: String,
    pub enemies: Vec<EnemyOnLevel>,
    pub lighting: [f32; 3],
    pub background: [f32; 3],
    pub shadow: [f32; 3],
    pub ambient: String,
}
