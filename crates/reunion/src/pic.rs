//! Loads original `.PIC` files straight into Bevy [`Image`]s.

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext, RenderAssetUsages};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use reunion_formats::pic::{self, PicError};

pub struct PicPlugin;

impl Plugin for PicPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(PicLoader);
    }
}

#[derive(TypePath)]
struct PicLoader;

#[derive(Debug, thiserror::Error)]
enum PicLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("pic: {0}")]
    Pic(#[from] PicError),
}

impl AssetLoader for PicLoader {
    type Asset = Image;
    type Settings = ();
    type Error = PicLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Image, PicLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let pic = pic::decode(&bytes)?;
        Ok(rgba_image(
            pic.width.into(),
            pic.height.into(),
            pic.to_rgba(),
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["PIC", "pic"]
    }
}

/// A pixel-art image; the CPU copy is kept so hover variants can be derived from it.
pub fn rgba_image(width: u32, height: u32, rgba: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    image
}
