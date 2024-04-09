use bevy::{asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext}, prelude::*, utils::BoxedFuture};
use bevy_asset_loader::asset_collection::AssetCollection;
use thiserror::Error;

#[derive(Asset, TypePath, Debug)]
pub struct FontAsset {
    #[allow(dead_code)]
    bytes: Vec<u8>,
}

impl FontAsset {
    pub fn get_bytes(&self) -> &Vec<u8> {
        &self.bytes
    }
}

#[derive(Default)]
pub struct FontAssetLoader;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum FontAssetLoaderError {
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
}

impl AssetLoader for FontAssetLoader {
    type Asset = FontAsset;
    type Settings = ();
    type Error = FontAssetLoaderError;
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a (),
        _load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(FontAsset { bytes })
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ttf"]
    }
}

#[derive(AssetCollection, Resource)]
pub struct FontAssets {
    #[asset(path = "PeaberryBase.ttf")]
    pub ui: Handle<FontAsset>,
}

#[derive(AssetCollection, Resource)]
pub struct BiomeMapAssets {
    #[asset(path = "biomes.png")]
    texture: Handle<Image>
}

#[derive(AssetCollection, Resource)]
pub struct PlayerSpriteAssets {
    #[asset(path = "player/idle.png")]
    pub idle: Handle<Image>
}

#[derive(AssetCollection, Resource)]
pub struct TileAssets {
    #[asset(path = "caves.png")]
    pub caves: Handle<Image>
}