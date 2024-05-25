use bevy::{prelude::*, utils::HashMap};
use fast_poisson::Poisson2D;

#[derive(Resource, Deref, DerefMut)]
pub struct Poisson(Poisson2D);

#[derive(Resource, Deref, DerefMut)]
pub struct PoissonEnemyPosition(pub HashMap<IVec2, Vec<Vec2>>);

impl Poisson {
    pub fn from_seed(seed: u32, frequency: f32) -> Self {
        Self(
            Poisson2D::new()
                .with_seed(seed as u64)
                .with_dimensions([20.0, 20.0], (1.0 / frequency) as f64
            )
        )
    }
}

impl PoissonEnemyPosition {
    pub fn from_distibution(poisson: &Poisson) -> Self {
        let mut map = HashMap::new();

        for point in poisson.iter() {
            let point = Vec2::new(point[0] as f32, point[1] as f32) - 10.0;

            map.entry(point.floor().as_ivec2()).or_insert(Vec::new()).push(point);
        }

        Self(map)
    }
}