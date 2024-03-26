pub const CHUNK_SIZE: i32 = 64;
pub const CHUNK_CELLS: i32 = CHUNK_SIZE.pow(2);

pub const WORLD_WIDTH: i32 = 8;
pub const WORLD_HEIGHT: i32 = 8;

// Bigger == faster
pub const COLLIDER_PRECISION: f32 = 2.0;

//Layers
pub const PLAYER_LAYER: f32 = 1.;
pub const PARTICLE_LAYER: f32 = 10.;
pub const AUTOMATA_LAYER: f32 = 100.;