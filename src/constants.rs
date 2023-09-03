pub const CHUNK_SIZE: i64 = 64;
pub const CHUNK_ELEMENTS: i64 = CHUNK_SIZE.pow(2);
pub const WORLD_WIDTH: i64 = 2;
pub const WORLD_HEIGHT: i64 = 1;
pub const SCALE: f64 = 5.0;

pub const GRAVITY: f32 = 9.0;
pub const DIRTY_CHUNK_OFFSET: i64 = 4;
pub const IDLE_FRAME_THRESHOLD: u8 = 2;

pub const DELAY_MS: u128 = 10;
pub const RENDER_DIRTY_CHUNKS: bool = true;
pub const INFO_MENU_OPEN: bool = false;
pub const IS_BENCHMARK: bool = false;