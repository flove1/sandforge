use std::{ sync::Arc, time::{ SystemTime, UNIX_EPOCH } };

use bevy::prelude::*;
use noise::{
    Billow,
    Fbm,
    HybridMulti,
    MultiFractal,
    NoiseFn,
    Perlin,
    RidgedMulti,
    Simplex,
};
use serde::Deserialize;

#[derive(Resource, Deref, DerefMut)]
pub struct Seed(pub u32);

impl Seed {
    pub fn new() -> Self {
        Self(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_millis())
    }
}

impl FromWorld for Seed {
    fn from_world(_: &mut World) -> Self {
        Self::new()
    }
}

#[derive(Deserialize, Clone, Copy)]
pub enum NoiseType {
    Fractal,
    Billow,
    HybridMulti,
}

pub type NoiseValue = Arc<Box<dyn (Fn(Vec2) -> f32) + Send + Sync>>;

#[derive(Resource, Clone)]
pub struct Noise {
    pub terrain_noise: NoiseValue,
    pub sand_noise: NoiseValue,
    pub liquid_noise: NoiseValue,
}

impl Noise {
    pub fn from_seed(seed: u32, terrain_noise_type: NoiseType) -> Self {
        let terrain_noise = {
            let seed = seed;

            let ridged_1 = RidgedMulti::<Perlin>::new(seed).set_octaves(1).set_frequency(2.0);
            let ridged_2 = RidgedMulti::<Perlin>
                ::new(seed + 1)
                .set_octaves(1)
                .set_frequency(2.0);

            let offset_fn = Arc::new(match terrain_noise_type {
                NoiseType::Fractal => {
                    let noise = Fbm::<Perlin>
                        ::new(seed + 2)
                        .set_octaves(6)
                        .set_frequency(3.0);

                    Box::new(move |point| { noise.get(point) / 4.0 }) as Box<dyn Fn([f64; 2]) -> f64 + Send + Sync>
                }
                NoiseType::Billow => {
                    let noise_1 = Fbm::<Perlin>
                        ::new(seed + 2)
                        .set_octaves(6)
                        .set_frequency(3.0);

                    let noise_2 = Billow::<Perlin>
                        ::new(seed + 2)
                        .set_octaves(6)
                        .set_frequency(4.5);

                    Box::new(move |point| { noise_1.get(point) / 4.0 + noise_2.get(point) / 16.0 }) as Box<dyn Fn([f64; 2]) -> f64 + Send + Sync>
                }
                NoiseType::HybridMulti => {
                    let noise = HybridMulti::<Perlin>
                        ::new(seed + 2)
                        .set_octaves(6)
                        .set_frequency(1.5);

                    Box::new(move |point| { noise.get(point) / 2.0 }) as Box<dyn Fn([f64; 2]) -> f64 + Send + Sync>
                }
            });

            move |point: Vec2| {
                let mut point = [(point.x as f64) / 48.0, (point.y as f64) / 48.0];

                point[1] += offset_fn(point);
                let value = ridged_1.get(point) + ridged_2.get(point) / 4.0;

                value as f32
            }
        };

        let sand_noise = {
            let seed = seed * 2;

            let noise_1 = HybridMulti::<Perlin>::new(seed);
            let noise_2 = Simplex::new(seed + 1);

            move |point: Vec2| {
                let point = [(point.x as f64) / 2.0, (point.y as f64) / 2.0];

                if noise_1.get(point) * 0.75 + noise_2.get(point) / 2.0 > 0.6 {
                    1.0
                } else {
                    0.0
                }
            }
        };

        let liquid_noise = {
            let seed = seed * 3;

            let ridged = RidgedMulti::<Perlin>::new(seed).set_octaves(1).set_frequency(2.0);
            let noise = Perlin::new(seed + 1);
            let fbm = Fbm::<Perlin>
                ::new(seed + 2)
                .set_octaves(6)
                .set_frequency(3.0);

            move |point: Vec2| {
                let mut point = [(point.x as f64) / 48.0, (point.y as f64) / 48.0];

                point[0] += fbm.get(point) / 2.0;
                let value = ridged.get(point);

                (value * (if noise.get(point) > 0.0 { 1.0 } else { 0.0 })) as f32
            }
        };

        Self {
            terrain_noise: Arc::new(Box::new(terrain_noise)),
            sand_noise: Arc::new(Box::new(sand_noise)),
            liquid_noise: Arc::new(Box::new(liquid_noise)),
        }
    }
}
