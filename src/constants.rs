pub const CHUNK_SIZE: i64 = 64;
pub const WORLD_WIDTH: i64 = 16;
pub const WORLD_HEIGHT: i64 = 9;
pub const SCALE: f64 = 1.0;
// pub const PIXEL_SIZE: f64 = 
pub const CHUNK_ELEMENTS: i64 = CHUNK_SIZE.pow(2);
pub const GRAVITY: f32 = 9.0;
pub const DIRTY_CHUNK_OFFSET: i64 = 4;
pub const IDLE_FRAME_THRESHOLD: u8 = 2;

pub const DELAY: u128 = 10;
pub const RENDER_DIRTY_CHUNKS: bool = true;
pub const IS_BENCHMARK: bool = false;