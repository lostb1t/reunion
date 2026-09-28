//! Loads original `.PIC` files straight into Bevy [`Image`]s.
//!
//! The original (FUN_405f_0bef) loads a picture's pixels and palette moved
//! up by 0x40, below the colors of the icon bar; so a game color c >= 0x40
//! used for fills is the picture's color c - 0x40.

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext, RenderAssetUsages};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use reunion_formats::ani::{Ani, AniError};
use reunion_formats::pic::{self, PicError};

pub struct PicPlugin;

/// Label of a picture's sub-asset with palette index 0 transparent.
pub const MASKED: &str = "masked";
/// Label of a picture's [`Palette`] sub-asset.
pub const PALETTE: &str = "palette";

/// A picture's palette, for animations that play over it.
#[derive(Asset, TypePath)]
pub struct Palette(pub [[u8; 3]; 256]);

/// Label of a picture's [`IndexedPic`] sub-asset.
pub const INDEXED: &str = "indexed";

/// A picture as palette indices, for effects on its palette (cutscenes).
#[derive(Asset, TypePath)]
pub struct IndexedPic(pub reunion_formats::pic::Pic);

/// A "SpidyAnim" `.ANI` animation.
#[derive(Asset, TypePath)]
pub struct Animation(pub Ani);

impl Plugin for PicPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Palette>()
            .init_asset::<IndexedPic>()
            .init_asset::<Animation>()
            .register_asset_loader(PicLoader)
            .register_asset_loader(AniLoader);
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
        load_context: &mut LoadContext<'_>,
    ) -> Result<Image, PicLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        // A few are only a palette (CREDITS/PAL3, PAL4): 256 x RGB.
        let pic = if bytes.len() == 768 && !bytes.starts_with(b"SpidyGfx") {
            pic::Pic {
                width: 1,
                height: 1,
                pixels: vec![0],
                palette: std::array::from_fn(|i| [bytes[3 * i], bytes[3 * i + 1], bytes[3 * i + 2]]),
            }
        } else {
            pic::decode(&bytes)?
        };
        // `<file>#masked`: palette index 0 transparent, for sprite sheets.
        load_context.add_labeled_asset(PALETTE.to_string(), Palette(pic.palette));
        load_context.add_labeled_asset(INDEXED.to_string(), IndexedPic(pic.clone()));
        load_context.add_labeled_asset(
            MASKED.to_string(),
            rgba_image(pic.width.into(), pic.height.into(), pic.to_rgba_masked()),
        );
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

#[derive(TypePath)]
struct AniLoader;

#[derive(Debug, thiserror::Error)]
enum AniLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("ani: {0}")]
    Ani(#[from] AniError),
}

impl AssetLoader for AniLoader {
    type Asset = Animation;
    type Settings = ();
    type Error = AniLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Animation, AniLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(Animation(Ani::parse(&bytes)?))
    }

    fn extensions(&self) -> &[&str] {
        &["ANI", "ani"]
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
