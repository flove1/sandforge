use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

#[derive(Default)]
pub struct Biome {
    pub color: [u8; 4],
    // pub tiles: Vec<Tile>,
    // pub tile_size: u8
    // pub tile_atlas: Handle<TextureAtlasLayout>
}

#[derive(AssetCollection, Resource)]
pub struct BiomeMap {
    #[asset(path = "biomes.png")]
    texture: Handle<Image>,
}