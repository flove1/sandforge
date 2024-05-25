use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::noise::NoiseType;

#[derive(Deserialize, Clone)]
pub struct NoiseLayer {
    pub material_id: String,
    pub value: f32,
}

#[derive(Deserialize, Clone)]
pub struct Level {
    pub terrain_layers: Vec<NoiseLayer>,
    pub background_layers: Vec<NoiseLayer>,
    pub texture_path: String,
    pub noise_type: NoiseType,
    pub powder_id: String,
    pub liquid_id: String,
    pub enemy_frequency: f32,
}