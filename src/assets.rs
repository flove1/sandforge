use bevy::{
    asset::{ io::Reader, AssetLoader, AsyncReadExt, LoadContext },
    prelude::*,
    render::texture::{ ImageAddressMode, ImageSampler, ImageSamplerDescriptor },
    utils::{ BoxedFuture, HashMap },
};
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
    #[error("Could not load asset: {0}")] Io(#[from] std::io::Error),
}

impl AssetLoader for FontAssetLoader {
    type Asset = FontBytes;
    type Settings = ();
    type Error = FontAssetLoaderError;
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a (),
        _load_context: &'a mut LoadContext
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
pub struct FontAssetCollection {
    #[asset(path = "PeaberryBase.ttf")]
    pub ui: Handle<FontBytes>,
}

#[derive(AssetCollection, Resource)]
pub struct LayoutAssetCollection {
    #[asset(path = "layouts", collection(typed, mapped))]
    pub folder: HashMap<String, Handle<Image>>,
}

#[derive(AssetCollection, Resource, Clone)]
pub struct SpriteAssetCollection {
    #[asset(path = "player/alchemist.png")]
    #[asset(image(sampler = nearest))]
    pub player: Handle<Image>,

    #[asset(path = "player/heal.png")]
    #[asset(image(sampler = nearest))]
    pub heal: Handle<Image>,

    #[asset(path = "player/attack.png")]
    #[asset(image(sampler = nearest))]
    pub attack: Handle<Image>,

    #[asset(path = "portal.png")]
    #[asset(image(sampler = nearest))]
    pub portal: Handle<Image>,

    #[asset(path = "enemy/bat.png")]
    #[asset(image(sampler = nearest))]
    pub bat: Handle<Image>,

    #[asset(path = "enemy/fungus_tiny.png")]
    #[asset(image(sampler = nearest))]
    pub fungus_tiny: Handle<Image>,

    #[asset(path = "enemy/fungus_big.png")]
    #[asset(image(sampler = nearest))]
    pub fungus_big: Handle<Image>,

    #[asset(path = "smoke.png")]
    #[asset(image(sampler = nearest))]
    pub smoke: Handle<Image>,

    #[asset(path = "rope.png")]
    #[asset(image(sampler = nearest))]
    pub rope: Handle<Image>,

    #[asset(path = "rope_end.png")]
    #[asset(image(sampler = nearest))]
    pub rope_end: Handle<Image>,

    #[asset(path = "enemy/plant.png")]
    #[asset(image(sampler = nearest))]
    pub plant: Handle<Image>,

    #[asset(path = "ui/cursor.png")]
    #[asset(image(sampler = linear))]
    pub cursor: Handle<Image>,

    #[asset(path = "enemy/frog.png")]
    #[asset(image(sampler = nearest))]
    pub frog: Handle<Image>,

    #[asset(path = "enemy/wolf.png")]
    #[asset(image(sampler = nearest))]
    pub wolf: Handle<Image>,

    #[asset(path = "enemy/rat.png")]
    #[asset(image(sampler = nearest))]
    pub rat: Handle<Image>,

    #[asset(path = "ui/help_border.png")]
    #[asset(image(sampler = linear))]
    pub border: Handle<Image>,

    #[asset(path = "ui/help_divider.png")]
    #[asset(image(sampler = linear))]
    pub help_divider: Handle<Image>,

    #[asset(path = "ui/help_divider_horizontal.png")]
    #[asset(image(sampler = linear))]
    pub help_divider_horizontal: Handle<Image>,

    #[asset(path = "ui/health_border.png")]
    pub in_game_border: Handle<Image>,
}

#[derive(AssetCollection, Resource, Clone)]
pub struct AudioAssetCollection {
    #[asset(path = "audio/menu.ogg")]
    pub menu: Handle<AudioSource>,

    #[asset(path = "audio/slash.ogg")]
    pub slash: Handle<AudioSource>,

    #[asset(path = "audio/hit.wav")]
    pub hit: Handle<AudioSource>,

    #[asset(path = "audio/button_select.ogg")]
    pub button_select: Handle<AudioSource>,

    #[asset(path = "audio/button_click.ogg")]
    pub button_click: Handle<AudioSource>,

    #[asset(path = "audio/death.ogg")]
    pub death: Handle<AudioSource>,

    #[asset(path = "audio/perk.ogg")]
    pub perk: Handle<AudioSource>,

    #[asset(path = "audio/powder", collection(typed, mapped))]
    pub powder: HashMap<String, Handle<AudioSource>>,

    #[asset(path = "audio/liquid", collection(typed, mapped))]
    pub liquid: HashMap<String, Handle<AudioSource>>,
}

pub fn process_assets(
    sprites: Res<SpriteAssetCollection>,
    mut images: ResMut<Assets<Image>>,
    font_handles: Res<FontAssetCollection>,
    font_data: ResMut<Assets<FontBytes>>,
    mut fonts: ResMut<Assets<Font>>
) {
    if let Some(image) = images.get_mut(sprites.rope.clone()) {
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            address_mode_w: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::nearest()
        });
    }

    let font_bytes = font_data.get(font_handles.ui.clone()).unwrap();
    fonts.insert(
        TextStyle::default().font,
        Font::try_from_bytes(font_bytes.bytes.clone()).expect("Can't create font")
    );
}
