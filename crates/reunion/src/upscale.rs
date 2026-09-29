//! Upscaling on the GPU. The game camera draws into a small canvas with one
//! texel per game pixel (e.g. 384x200 on the Steam Deck), and a second camera
//! draws that canvas onto the screen through a shader, which scales it with
//! the chosen [`UpscaleMode`] and applies the 1.2 pixel aspect.
//!
//! F8 or the left stick button switches between them.

use bevy::asset::embedded_asset;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::sprite::Anchor;
use bevy::sprite_render::{Material2d, Material2dPlugin};
use bevy_enhanced_input::prelude::*;

use crate::input::{CycleUpscale, ToggleWidescreen};
use crate::screen::{GAME_HEIGHT, GAME_WIDTH, PIXEL_ASPECT, Widescreen};

pub struct UpscalePlugin;

impl Plugin for UpscalePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "upscale.wgsl");
        app.add_plugins(Material2dPlugin::<UpscaleMaterial>::default())
            .init_resource::<UpscaleMode>()
            .init_resource::<Canvas>()
            .add_systems(Startup, spawn_cameras)
            .add_systems(PreUpdate, fit_canvas.in_set(FitCanvas))
            .add_systems(Update, fade_label)
            .add_systems(PostUpdate, place_overlays)
            .add_observer(cycle)
            .add_observer(toggle_widescreen);
    }
}

/// How the canvas is scaled to the screen.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum UpscaleMode {
    /// Plain nearest-neighbour scaling, like the original.
    #[default]
    Off,
    /// Scanlines and an aperture grille.
    Crt,
}

impl UpscaleMode {
    pub const ALL: [UpscaleMode; 2] = [UpscaleMode::Off, UpscaleMode::Crt];

    pub fn name(self) -> &'static str {
        match self {
            UpscaleMode::Off => "Off",
            UpscaleMode::Crt => "CRT",
        }
    }

    /// The filter in upscale.wgsl.
    fn shader_mode(self) -> u32 {
        match self {
            UpscaleMode::Off => 0,
            UpscaleMode::Crt => 1,
        }
    }

    fn next(self) -> Self {
        match self {
            UpscaleMode::Off => UpscaleMode::Crt,
            UpscaleMode::Crt => UpscaleMode::Off,
        }
    }
}

/// A sprite drawn at the screen's resolution over the part of the canvas
/// showing this rectangle of game coordinates, rather than through the
/// canvas: for pictures with more detail than the game's pixels (the intro's
/// 640 x 480 stills).
/// Spawn it with `RenderLayers::layer(1)`.
#[derive(Component)]
pub struct CanvasOverlay(pub Rect);

fn place_overlays(
    canvas: Res<Canvas>,
    display: Single<&Camera, With<DisplayCamera>>,
    mut overlays: Query<(&CanvasOverlay, &mut Transform, &mut Sprite)>,
) {
    let Some(logical) = display.logical_viewport_size() else { return };
    let width = canvas.width as f32;
    let scale = canvas.rect.size() / Vec2::new(width, GAME_HEIGHT);
    for (overlay, mut transform, mut sprite) in &mut overlays {
        // Game x to canvas texels: the canvas is centred on the game's middle.
        let min = Vec2::new(overlay.0.min.x - GAME_WIDTH / 2.0 + width / 2.0, overlay.0.min.y);
        let at = canvas.rect.min + min * scale;
        transform.translation = Vec3::new(at.x - logical.x / 2.0, logical.y / 2.0 - at.y, 1.0);
        transform.scale = Vec3::ONE;
        sprite.custom_size = Some(overlay.0.size() * scale);
    }
}

/// Sizes the canvas and the game camera's view to the screen.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FitCanvas;

/// The camera that draws the game, into the canvas.
#[derive(Component)]
pub struct GameCamera;

/// The camera that draws the canvas onto the screen.
#[derive(Component)]
pub struct DisplayCamera;

#[derive(Component)]
struct CanvasQuad;

/// Shows the mode's name for a moment after switching.
#[derive(Component)]
struct ModeLabel(Timer);

/// The canvas, and where it's shown in the display camera's viewport.
#[derive(Resource)]
pub struct Canvas {
    pub image: Handle<Image>,
    material: Handle<UpscaleMaterial>,
    /// Width in texels (= game pixels); the height is always 200.
    pub width: u32,
    /// In logical viewport coordinates (origin top-left).
    pub rect: Rect,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            image: Handle::default(),
            material: Handle::default(),
            width: GAME_WIDTH as u32,
            rect: Rect::new(0.0, 0.0, GAME_WIDTH, GAME_HEIGHT * PIXEL_ASPECT),
        }
    }
}

impl Canvas {
    /// A position in the display viewport (like the window's cursor) in game coordinates.
    pub fn to_game(&self, pos: Vec2) -> Vec2 {
        let rel = (pos - self.rect.min) / self.rect.size();
        let width = self.width as f32;
        Vec2::new(
            rel.x * width - width / 2.0 + GAME_WIDTH / 2.0,
            rel.y * GAME_HEIGHT,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Debug, ShaderType)]
struct Params {
    source_size: Vec2,
    scale: Vec2,
    mode: u32,
    /// WebGL2 wants uniform blocks in multiples of 16 bytes: 20 -> 32
    /// (separate fields: arrays in uniforms get a 16-byte stride).
    padding0: u32,
    padding1: u32,
    padding2: u32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct UpscaleMaterial {
    #[uniform(0)]
    params: Params,
    #[texture(1)]
    #[sampler(2)]
    canvas: Handle<Image>,
}

impl Material2d for UpscaleMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://reunion/upscale.wgsl".into()
    }
}

fn canvas_image(width: u32) -> Image {
    let mut image = Image::new_target_texture(
        width,
        GAME_HEIGHT as u32,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    // The shader picks texels itself where it wants hard edges.
    image.sampler = ImageSampler::linear();
    image
}

fn game_projection(width: u32) -> Projection {
    Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::Fixed {
            width: width as f32,
            height: GAME_HEIGHT * PIXEL_ASPECT,
        },
        ..OrthographicProjection::default_2d()
    })
}

fn spawn_cameras(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<UpscaleMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mode: Res<UpscaleMode>,
    mut canvas: ResMut<Canvas>,
) {
    let width = GAME_WIDTH as u32;
    let image = images.add(canvas_image(width));
    commands.spawn((
        Camera2d,
        GameCamera,
        Camera {
            order: -1,
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Msaa::Off,
        game_projection(width),
    ));
    let material = materials.add(UpscaleMaterial {
        params: Params {
            source_size: Vec2::new(width as f32, GAME_HEIGHT),
            scale: Vec2::ONE,
            mode: mode.shader_mode(),
            padding0: 0,
            padding1: 0,
            padding2: 0,
        },
        canvas: image.clone(),
    });
    commands.spawn((Camera2d, DisplayCamera, Msaa::Off, RenderLayers::layer(1)));
    commands.spawn((
        CanvasQuad,
        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial2d(material.clone()),
        RenderLayers::layer(1),
    ));
    commands.spawn((
        ModeLabel(Timer::from_seconds(0.0, TimerMode::Once)),
        Text2d::default(),
        TextColor(Color::srgb(1.0, 0.85, 0.35)),
        Anchor::TOP_LEFT,
        Transform::from_xyz(0.0, 0.0, 10.0),
        Visibility::Hidden,
        RenderLayers::layer(1),
    ));
    canvas.image = image;
    canvas.material = material;
    canvas.width = width;
}

/// Sizes the canvas to the screen's shape and fits it into the screen.
fn fit_canvas(
    display: Single<&Camera, With<DisplayCamera>>,
    mut game_camera: Single<&mut Projection, With<GameCamera>>,
    mut quad: Single<&mut Transform, (With<CanvasQuad>, Without<ModeLabel>)>,
    mut label: Single<&mut Transform, (With<ModeLabel>, Without<CanvasQuad>)>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<UpscaleMaterial>>,
    mode: Res<UpscaleMode>,
    widescreen: Res<Widescreen>,
    mut canvas: ResMut<Canvas>,
) {
    let (Some(logical), Some(physical)) =
        (display.logical_viewport_size(), display.physical_viewport_size())
    else {
        return;
    };
    // Placed in physical pixels, so integer scales are exact.
    let screen = physical.as_vec2();
    let pixels_per_unit = screen.x / logical.x;
    let tall = GAME_HEIGHT * PIXEL_ASPECT;
    // As wide as the screen allows (in whole, even game pixels), at least 320.
    // 4:3 keeps the original's 320 columns, with black beside them.
    let width = if widescreen.0 {
        ((tall * screen.x / screen.y / 2.0).floor() as u32 * 2).max(GAME_WIDTH as u32)
    } else {
        GAME_WIDTH as u32
    };
    let fit = (screen.x / width as f32).min(screen.y / tall);
    let scale = Vec2::new(fit, fit * PIXEL_ASPECT);
    if width != canvas.width {
        canvas.width = width;
        if let Some(mut image) = images.get_mut(&canvas.image) {
            image.resize(Extent3d {
                width,
                height: GAME_HEIGHT as u32,
                depth_or_array_layers: 1,
            });
        }
        **game_camera = game_projection(width);
    }
    let size = Vec2::new(width as f32, GAME_HEIGHT) * scale;
    // Whole pixels from the top-left, so texel edges land on pixel edges.
    let min = ((screen - size) / 2.0).floor();
    let rect = Rect::from_corners(min, min + size);
    let rect = Rect::from_corners(rect.min / pixels_per_unit, rect.max / pixels_per_unit);
    canvas.rect = rect;
    let center = rect.center() - logical / 2.0;
    quad.translation = Vec3::new(center.x, -center.y, 0.0);
    quad.scale = rect.size().extend(1.0);
    label.translation = Vec3::new(rect.min.x - logical.x / 2.0 + 12.0, logical.y / 2.0 - rect.min.y - 8.0, 10.0);

    let params = Params {
        source_size: Vec2::new(width as f32, GAME_HEIGHT),
        scale,
        mode: mode.shader_mode(),
        padding0: 0,
        padding1: 0,
        padding2: 0,
    };
    // Only touch the material on changes: that re-uploads it.
    if materials.get(&canvas.material).is_some_and(|m| m.params != params)
        && let Some(mut material) = materials.get_mut(&canvas.material)
    {
        material.params = params;
    }
}

fn cycle(
    _: On<Start<CycleUpscale>>,
    mut mode: ResMut<UpscaleMode>,
    mut label: Single<(&mut ModeLabel, &mut Text2d, &mut Visibility)>,
) {
    *mode = mode.next();
    info!("upscale: {}", mode.name());
    let (timer, text, visibility) = &mut *label;
    timer.0 = Timer::from_seconds(2.0, TimerMode::Once);
    text.0 = format!("Upscale: {}", mode.name());
    **visibility = Visibility::Inherited;
}

fn toggle_widescreen(
    _: On<Start<ToggleWidescreen>>,
    mut widescreen: ResMut<Widescreen>,
    mut label: Single<(&mut ModeLabel, &mut Text2d, &mut Visibility)>,
) {
    widescreen.0 = !widescreen.0;
    let (timer, text, visibility) = &mut *label;
    timer.0 = Timer::from_seconds(2.0, TimerMode::Once);
    text.0 = if widescreen.0 { "Widescreen (experimental)" } else { "Original 4:3" }.into();
    **visibility = Visibility::Inherited;
}

fn fade_label(time: Res<Time>, mut label: Single<(&mut ModeLabel, &mut Visibility)>) {
    let (timer, visibility) = &mut *label;
    if timer.0.tick(time.delta()).just_finished() {
        **visibility = Visibility::Hidden;
    }
}
