pub const CHUNK_SIZE: i32 = 64;
pub const CHUNK_CELLS: i32 = CHUNK_SIZE.pow(2);

// Bigger == faster
pub const COLLIDER_PRECISION: f32 = 2.0;

pub const BACKGROUND_Z: f32 = -1.;
pub const TERRAIN_Z: f32 = 1.;
pub const ENEMY_Z: f32 = 2.;
pub const PLAYER_Z: f32 = 3.;
pub const PARTICLE_Z: f32 = 4.;