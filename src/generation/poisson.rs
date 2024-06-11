use bevy::{prelude::*, utils::HashMap};
use fast_poisson::Poisson2D;
use rand::{Rng, SeedableRng};

use super::level::EnemyOnLevel;

#[derive(Resource, Deref, DerefMut)]
pub struct EnemyPositions(pub HashMap<IVec2, Vec<(String, Vec2)>>);

impl EnemyPositions {
    pub fn new(seed: u32, size: IVec2, enemies: Vec<EnemyOnLevel>) -> Self {
        let mut map = HashMap::new();
        let mut seed = seed;

        for enemy_type in enemies {
            seed += 1;
            let poisson = Poisson2D::new()
                .with_seed(seed as u64)
                .with_dimensions([size.x as f64, size.y as f64], (1.0 / enemy_type.frequency) as f64
            );
            
            let mut probability_rng = rand::rngs::SmallRng::seed_from_u64(seed as u64);

            for point in poisson.iter() {
                if !probability_rng.gen_bool(enemy_type.spawn_chance as f64) {
                    continue;
                }

                let point = Vec2::new(point[0] as f32, point[1] as f32) - size.as_vec2() / 2.0;
                map.entry(point.floor().as_ivec2()).or_insert(Vec::new()).push((enemy_type.enemy_id.clone(), point));
            }
        }

        Self(map)
    }
}