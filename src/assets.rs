use bevy::{asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext}, prelude::*, render::texture::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor}, utils::BoxedFuture};
use bevy_asset_loader::asset_collection::AssetCollection;
use thiserror::Error;

#[derive(Asset, TypePath, Debug)]
pub struct FontBytes {
    #[allow(dead_code)]
    bytes: Vec<u8>,
}

impl FontBytes {
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
    type Asset = FontBytes;
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
            Ok(FontBytes { bytes })
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ttf"]
    }
}

#[derive(AssetCollection, Resource)]
pub struct FontAssets {
    #[asset(path = "PeaberryBase.ttf")]
    pub ui: Handle<FontBytes>,
}

#[derive(AssetCollection, Resource)]
pub struct ChunkMapAssets {
    #[asset(path = "chunkmap.png")]
    pub texture: Handle<Image>,
    #[asset(path = "structure.png")]
    pub structure: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
pub struct SpriteSheets {
    #[asset(path = "player/alchemist.png")]
    pub player: Handle<Image>,

    #[asset(path = "bat.png")]
    pub bat: Handle<Image>,

    #[asset(path = "smoke.png")]
    pub smoke: Handle<Image>,

    #[asset(path = "rope.png")]
    pub rope: Handle<Image>,

    #[asset(path = "rope_end.png")]
    pub rope_end: Handle<Image>,
}

pub fn process_assets(
    sprites: Res<SpriteSheets>,
    mut images: ResMut<Assets<Image>>,
    font_handles: Res<FontAssets>,
    font_data: ResMut<Assets<FontBytes>>,
    mut fonts: ResMut<Assets<Font>>,
) {
    let image = images.get_mut(sprites.rope.clone()).unwrap();

    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::nearest()
    });

    let font_bytes = font_data.get(font_handles.ui.clone()).unwrap();
    fonts.insert(TextStyle::default().font, Font::try_from_bytes(font_bytes.bytes.clone()).expect("Can't create font"));
}