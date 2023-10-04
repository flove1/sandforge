pub const CHUNK_SIZE: i32 = 64;

pub const CHUNK_ELEMENTS: i32 = CHUNK_SIZE.pow(2);

pub const WORLD_WIDTH: i32 = 8;
pub const WORLD_HEIGHT: i32 = 8;

pub const SCREEN_WIDTH: f32 = (WORLD_WIDTH * CHUNK_SIZE) as f32 * SCALE;
pub const SCREEN_HEIGHT: f32 = (WORLD_HEIGHT * CHUNK_SIZE) as f32 * SCALE;

pub const SCALE: f32 = 1.5;

pub const PHYSICS_SCALE: f32 = 1.0;
pub const PHYSICS_TO_WORLD: f32 = CHUNK_SIZE as f32 * PHYSICS_SCALE;

// Bigger == faster
pub const COLLIDER_PRECISION: f32 = 2.0;

pub const DIRTY_CHUNK_OFFSET: i32 = 2;

pub const TARGET_FPS: u128 = 120;
pub const CA_DELAY_MS: u128 = 5;