//! INFO-BUY's monitor: a spinning 3D model (VECTORS/V<n>.VEC), drawn in
//! software like the original's FUN_3d38_01aa / FUN_3e17_*:
//!
//! - every step the model turns by its spin (DS:0x5713 + 3 * invention,
//!   in thirds of a degree), starting at (0, 0, 100) degrees;
//! - vertices are rotated and projected without perspective at
//!   `zoom` (DS:0x56cd + invention) * 500 / 65536 pixels per unit, around
//!   (58, 57) of a 114x115 window;
//! - objects are drawn far to near, and within an object only the faces
//!   turned to the viewer, flat-shaded in 47 blues (palette 0xc0 +
//!   ((nz - nx) + 24) * 4 / 5, where entry 0xc0 + i is 6-bit blue
//!   i * 40 / 48 + 10).
//!
//! The original also nudges a few models' parts in sorting and draws some
//! inside faces (FUN_3e17_04d2, FUN_3e17_00cc); that isn't copied.

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext, RenderAssetUsages};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use reunion_formats::vec::{self, Model, VecError};

pub struct ModelViewPlugin;

impl Plugin for ModelViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<VecModel>()
            .register_asset_loader(VecLoader)
            .add_systems(Update, spin_and_draw);
    }
}

pub const WIDTH: u32 = 114;
pub const HEIGHT: u32 = 115;
const CENTER: Vec2 = Vec2::new(58.0, 57.0);
/// The original redraws once per main loop pass; this looks about as fast.
const STEPS_PER_SECOND: f32 = 30.0;
/// Angles are kept in thirds of a degree, like the original.
const FULL_TURN: u16 = 1080;

#[derive(Asset, TypePath)]
pub struct VecModel(pub Model);

#[derive(TypePath)]
struct VecLoader;

#[derive(Debug, thiserror::Error)]
enum VecLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("vec: {0}")]
    Vec(#[from] VecError),
}

impl AssetLoader for VecLoader {
    type Asset = VecModel;
    type Settings = ();
    type Error = VecLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<VecModel, VecLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(VecModel(vec::decode(&bytes)?))
    }

    fn extensions(&self) -> &[&str] {
        &["VEC", "vec"]
    }
}

/// A spinning model drawn into `image` (the entity's sprite).
#[derive(Component)]
pub struct ModelView {
    pub model: Handle<VecModel>,
    pub image: Handle<Image>,
    /// Thirds of a degree about x, y and z.
    pub angles: [u16; 3],
    pub spin: [u16; 3],
    pub zoom: f32,
    /// Mode 2 swaps the screen axes (DS:0x56f1 + invention).
    pub swap_axes: bool,
    steps: f32,
}

impl ModelView {
    pub fn new(
        model: Handle<VecModel>,
        images: &mut Assets<Image>,
        spin: [u8; 3],
        zoom: u8,
        mode: u8,
        invention: u8,
    ) -> Self {
        // FUN_3d38_084a: most models start at (0, 0, 100) degrees.
        let angles = match invention {
            14 => [0xd7, 0x2d, 0x36],
            24 => [0, 0x15, 0x20],
            _ => [0, 0, 300],
        };
        Self {
            model,
            image: images.add(blank()),
            angles,
            spin: spin.map(u16::from),
            zoom: f32::from(zoom),
            swap_axes: mode == 2,
            steps: 0.0,
        }
    }
}

fn blank() -> Image {
    let mut image = Image::new(
        Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; (WIDTH * HEIGHT * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn spin_and_draw(
    time: Res<Time>,
    mut views: Query<&mut ModelView>,
    models: Res<Assets<VecModel>>,
    mut images: ResMut<Assets<Image>>,
) {
    for mut view in &mut views {
        view.steps += time.delta_secs() * STEPS_PER_SECOND;
        if view.steps < 1.0 {
            continue;
        }
        let n = view.steps.floor();
        view.steps -= n;
        for _ in 0..n as u32 {
            for axis in 0..3 {
                view.angles[axis] = (view.angles[axis] + view.spin[axis]) % FULL_TURN;
            }
        }
        let Some(model) = models.get(&view.model) else {
            continue;
        };
        let pixels = render(&model.0, &view);
        if let Some(mut image) = images.get_mut(&view.image) {
            image.data = Some(pixels);
        }
    }
}

/// Rotation matrix from the three angles (thirds of a degree): the first
/// turns the model about its up axis (z), the last tilts it toward the viewer.
fn rotation(angles: [u16; 3]) -> Mat3 {
    let [a, b, c] = angles.map(|t| (f32::from(t) / 3.0).to_radians());
    Mat3::from_rotation_x(c) * Mat3::from_rotation_y(b) * Mat3::from_rotation_z(a)
}

/// The shade of a face turned toward the viewer, as a color.
fn shade(normal: Vec3) -> [u8; 4] {
    // The original's normals are about 20 long after its fixed-point
    // rotation (that spreads its 47 shades over the faces); toward the
    // viewer is -z here.
    let n = normal.normalize_or_zero() * 20.0;
    let level = (((-n.z - n.x) + 24.0) * 4.0 / 5.0).clamp(0.0, 46.0) as u32;
    let blue = (level * 40 / 48 + 10) * 255 / 63;
    [0, 0, blue as u8, 255]
}

pub fn render(model: &Model, view: &ModelView) -> Vec<u8> {
    let rot = rotation(view.angles);
    let scale = view.zoom * 500.0 / 65536.0;
    let project = |v: Vec3| {
        let p = rot * v * scale;
        let (x, y) = if view.swap_axes { (p.y, p.x) } else { (p.x, p.y) };
        // Like the original: screen y is the rotated y, not flipped.
        (Vec2::new(CENTER.x + x, CENTER.y + y), p.z)
    };
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    for px in pixels.chunks_exact_mut(4) {
        px[3] = 255;
    }
    // Objects far to near, by their centers.
    let mut order: Vec<(f32, usize)> = model
        .objects
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let sum: Vec3 = o.vertices.iter().map(|v| as_vec3(*v)).sum();
            let center = sum / o.vertices.len().max(1) as f32;
            (project(center).1, i)
        })
        .collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, i) in order {
        let object = &model.objects[i];
        let points: Vec<(Vec2, f32)> = object.vertices.iter().map(|v| project(as_vec3(*v))).collect();
        let rotated: Vec<Vec3> = object.vertices.iter().map(|v| rot * as_vec3(*v)).collect();
        for face in &object.faces {
            if face.len() < 3 {
                continue;
            }
            let (a, b, c) = (rotated[face[0]], rotated[face[1]], rotated[face[2]]);
            let mut normal = (b - a).cross(c - a);
            if view.swap_axes {
                normal = Vec3::new(normal.y, normal.x, -normal.z);
            }
            // Only faces turned toward the viewer.
            if normal.z >= 0.0 {
                continue;
            }
            let polygon: Vec<Vec2> = face.iter().map(|&v| points[v].0).collect();
            fill(&mut pixels, &polygon, shade(normal));
        }
    }
    pixels
}

fn as_vec3(v: [i16; 3]) -> Vec3 {
    Vec3::new(f32::from(v[0]), f32::from(v[1]), f32::from(v[2]))
}

/// Scanline polygon fill (even-odd), pixel centers.
fn fill(pixels: &mut [u8], polygon: &[Vec2], color: [u8; 4]) {
    let min_y = polygon.iter().map(|p| p.y).fold(f32::MAX, f32::min).max(0.0) as i32;
    let max_y = polygon.iter().map(|p| p.y).fold(f32::MIN, f32::max).min(HEIGHT as f32 - 1.0) as i32;
    let mut crossings = Vec::new();
    for y in min_y..=max_y {
        let sy = y as f32 + 0.5;
        crossings.clear();
        for i in 0..polygon.len() {
            let (p, q) = (polygon[i], polygon[(i + 1) % polygon.len()]);
            if (p.y <= sy) != (q.y <= sy) {
                crossings.push(p.x + (sy - p.y) / (q.y - p.y) * (q.x - p.x));
            }
        }
        crossings.sort_by(f32::total_cmp);
        for pair in crossings.chunks_exact(2) {
            let x0 = (pair[0] - 0.5).ceil().max(0.0) as i32;
            let x1 = (pair[1] - 0.5).floor().min(WIDTH as f32 - 1.0) as i32;
            for x in x0..=x1 {
                let at = ((y as u32 * WIDTH + x as u32) * 4) as usize;
                pixels[at..at + 4].copy_from_slice(&color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `REUNION_MODEL_SHOTS=<dir> cargo test render_models` writes a few
    /// frames of every model as PPM images, for looking at the renderer.
    #[test]
    fn render_models() {
        let Ok(dir) = std::env::var("REUNION_MODEL_SHOTS") else {
            return;
        };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../game");
        let exe = std::fs::read(root.join("GRWAR/REUNION.PRG")).unwrap();
        let exe = reunion_formats::exe::GameExe::parse(exe).unwrap();
        for n in [1u8, 3, 6, 10, 16, 23, 33] {
            let model = vec::decode(&std::fs::read(root.join(format!("VECTORS/V{n}.VEC"))).unwrap()).unwrap();
            let spin = exe.ds_bytes(0x5713 + 3 * u16::from(n), 3).unwrap();
            let zoom = exe.ds_bytes(0x56cd + u16::from(n), 1).unwrap()[0];
            let mode = exe.ds_bytes(0x56f1 + u16::from(n), 1).unwrap()[0];
            let mut images = Assets::<Image>::default();
            let mut view = ModelView::new(Handle::default(), &mut images, [spin[0], spin[1], spin[2]], zoom, mode, n);
            let mut sheet = vec![0u8; (WIDTH * 4 * HEIGHT * 3) as usize];
            for frame in 0..4u32 {
                let pixels = render(&model, &view);
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        let src = ((y * WIDTH + x) * 4) as usize;
                        let dst = ((y * WIDTH * 4 + frame * WIDTH + x) * 3) as usize;
                        sheet[dst..dst + 3].copy_from_slice(&pixels[src..src + 3]);
                    }
                }
                for axis in 0..3 {
                    view.angles[axis] = (view.angles[axis] + 30 * view.spin[axis]) % FULL_TURN;
                }
            }
            let mut ppm = format!("P6 {} {} 255\n", WIDTH * 4, HEIGHT).into_bytes();
            ppm.extend(sheet);
            std::fs::write(format!("{dir}/model{n}.ppm"), ppm).unwrap();
        }
    }
}
