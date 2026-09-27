//! One hotspot is focused at a time, driven by mouse hover or directional input.
//! The focused hotspot shows its hover sprite; confirming it triggers [`Activated`].

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;
use bevy_enhanced_input::prelude::*;

use crate::input::{Click, Confirm, Navigate};
use crate::pic::rgba_image;
use crate::screen::{place, world_to_game};

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Focus>()
            .init_resource::<Cursor>()
            .add_observer(navigate)
            .add_observer(confirm)
            .add_observer(click)
            .add_systems(
                Update,
                (
                    track_cursor,
                    focus_default,
                    drop_stale_focus,
                    build_hover_sprites,
                    show_focus,
                )
                    .chain(),
            );
    }
}

/// A clickable area in game coordinates. The entity doubles as its hover sprite.
#[derive(Component)]
pub struct Hotspot {
    pub rect: Rect,
    pub hover: Hover,
}

pub enum Hover {
    /// A brightened crop of a full-screen picture drawn at the game origin.
    Brighten(Handle<Image>),
}

/// Focused when the hotspot appears, so directional input has a starting point.
#[derive(Component)]
pub struct DefaultFocus;

#[derive(Resource, Default)]
pub struct Focus(pub Option<Entity>);

/// Mouse position in game coordinates.
#[derive(Resource, Default)]
struct Cursor(Option<Vec2>);

#[derive(Component)]
struct HoverReady;

/// A hotspot was activated by click, Enter or the gamepad.
#[derive(Event)]
pub struct Activated(pub Entity);

pub fn hotspot(rect: Rect, hover: Hover) -> impl Bundle {
    (Hotspot { rect, hover }, place(rect.min, 1.0), Visibility::Hidden)
}

fn hotspot_at<'a>(pos: Vec2, hotspots: impl IntoIterator<Item = (Entity, &'a Hotspot)>) -> Option<Entity> {
    hotspots
        .into_iter()
        .find(|(_, h)| h.rect.contains(pos))
        .map(|(e, _)| e)
}

fn track_cursor(
    mut moved: MessageReader<CursorMoved>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    hotspots: Query<(Entity, &Hotspot)>,
    mut cursor: ResMut<Cursor>,
    mut focus: ResMut<Focus>,
) {
    if moved.read().last().is_none() {
        return;
    }
    let (camera, camera_transform) = *camera;
    cursor.0 = window
        .cursor_position()
        .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())
        .map(world_to_game);
    if let Some(pos) = cursor.0 {
        // The mouse takes over focus whenever it moves.
        focus.0 = hotspot_at(pos, hotspots);
    }
}

fn navigate(nav: On<Fire<Navigate>>, hotspots: Query<(Entity, &Hotspot)>, mut focus: ResMut<Focus>) {
    // Snap to the dominant axis; input y is up, game y is down.
    let v = nav.value;
    let dir = if v.x.abs() > v.y.abs() {
        Vec2::new(v.x.signum(), 0.0)
    } else {
        Vec2::new(0.0, -v.y.signum())
    };
    let centers = || hotspots.iter().map(|(e, h)| (e, h.rect.center()));
    let from = focus.0.and_then(|e| hotspots.get(e).ok()).map(|(_, h)| h.rect.center());
    focus.0 = match from {
        Some(from) => next_in_direction(from, dir, centers()).or(focus.0),
        None => centers()
            .min_by(|a, b| (a.1.y, a.1.x).partial_cmp(&(b.1.y, b.1.x)).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(e, _)| e),
    };
}

/// Nearest candidate ahead of `from` in `dir`, preferring ones in line with it.
/// Wraps around to the far side when nothing is ahead.
fn next_in_direction<T: Copy>(from: Vec2, dir: Vec2, candidates: impl Iterator<Item = (T, Vec2)> + Clone) -> Option<T> {
    let scored = |ahead: bool| {
        candidates
            .clone()
            .filter_map(move |(item, center)| {
                let d = center - from;
                let along = d.dot(dir);
                let across = (d - dir * along).length();
                let in_direction = if ahead { along > 0.5 } else { along < -0.5 };
                in_direction.then_some((item, along + 2.0 * across))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(item, _)| item)
    };
    scored(true).or_else(|| scored(false))
}

fn confirm(_: On<Start<Confirm>>, focus: Res<Focus>, mut commands: Commands) {
    if let Some(entity) = focus.0 {
        commands.trigger(Activated(entity));
    }
}

fn click(
    _: On<Start<Click>>,
    cursor: Res<Cursor>,
    hotspots: Query<(Entity, &Hotspot)>,
    mut focus: ResMut<Focus>,
    mut commands: Commands,
) {
    if let Some(pos) = cursor.0
        && let Some(entity) = hotspot_at(pos, hotspots)
    {
        focus.0 = Some(entity);
        commands.trigger(Activated(entity));
    }
}

fn focus_default(added: Query<Entity, Added<DefaultFocus>>, mut focus: ResMut<Focus>) {
    if let Some(entity) = added.iter().next() {
        focus.0 = Some(entity);
    }
}

fn drop_stale_focus(hotspots: Query<(), With<Hotspot>>, mut focus: ResMut<Focus>) {
    if focus.0.is_some_and(|e| !hotspots.contains(e)) {
        focus.0 = None;
    }
}

fn build_hover_sprites(
    pending: Query<(Entity, &Hotspot), Without<HoverReady>>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    for (entity, hotspot) in &pending {
        let Hover::Brighten(source) = &hotspot.hover;
        let Some(crop) = images.get(source).and_then(|image| brightened_crop(image, hotspot.rect)) else {
            continue; // source not loaded yet
        };
        let handle = images.add(crop);
        commands
            .entity(entity)
            .insert((Sprite::from_image(handle), Anchor::TOP_LEFT, HoverReady));
    }
}

/// Crops `rect` out of `image` and brightens it towards warm white.
fn brightened_crop(image: &Image, rect: Rect) -> Option<Image> {
    let data = image.data.as_ref()?;
    let (width, height) = (image.width(), image.height());
    let x0 = (rect.min.x.max(0.0) as u32).min(width);
    let y0 = (rect.min.y.max(0.0) as u32).min(height);
    let x1 = (rect.max.x as u32).clamp(x0, width);
    let y1 = (rect.max.y as u32).clamp(y0, height);
    if x0 == x1 || y0 == y1 {
        return None;
    }
    let rows = || (y0..y1).map(|y| &data[((y * width + x0) * 4) as usize..((y * width + x1) * 4) as usize]);
    // Normalize to the crop's brightest channel so dim items light up as much as bright ones.
    let peak = rows()
        .flat_map(|row| row.chunks_exact(4).flat_map(|px| px[..3].to_vec()))
        .max()
        .unwrap_or(0)
        .max(1);
    let gain = 255.0 / peak as f32;
    let boost = |c: u8, tint: f32| (c as f32 * gain * tint).min(255.0) as u8;
    let mut out = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
    for row in rows() {
        for px in row.chunks_exact(4) {
            out.extend([boost(px[0], 1.0), boost(px[1], 0.9), boost(px[2], 0.6), px[3]]);
        }
    }
    Some(rgba_image(x1 - x0, y1 - y0, out))
}

fn show_focus(
    focus: Res<Focus>,
    time: Res<Time>,
    mut hotspots: Query<(Entity, &mut Visibility, Option<&mut Sprite>), With<Hotspot>>,
) {
    let pulse = 0.8 + 0.2 * (time.elapsed_secs() * 5.0).sin();
    for (entity, mut visibility, sprite) in &mut hotspots {
        let focused = focus.0 == Some(entity);
        visibility.set_if_neq(if focused { Visibility::Inherited } else { Visibility::Hidden });
        if focused && let Some(mut sprite) = sprite {
            sprite.color = Color::WHITE.with_alpha(pulse);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The three main menu items, stacked vertically.
    const ITEMS: [(u8, Vec2); 3] = [(0, Vec2::new(163.0, 62.0)), (1, Vec2::new(163.0, 90.0)), (2, Vec2::new(163.0, 119.0))];

    #[test]
    fn moves_to_nearest_in_direction() {
        let down = Vec2::new(0.0, 1.0);
        assert_eq!(next_in_direction(ITEMS[0].1, down, ITEMS.into_iter()), Some(1));
        assert_eq!(next_in_direction(ITEMS[1].1, -down, ITEMS.into_iter()), Some(0));
    }

    #[test]
    fn wraps_around() {
        let down = Vec2::new(0.0, 1.0);
        assert_eq!(next_in_direction(ITEMS[2].1, down, ITEMS.into_iter()), Some(0));
        assert_eq!(next_in_direction(ITEMS[0].1, -down, ITEMS.into_iter()), Some(2));
    }

    #[test]
    fn stays_put_without_candidates_sideways() {
        let right = Vec2::new(1.0, 0.0);
        assert_eq!(next_in_direction(ITEMS[1].1, right, ITEMS.into_iter()), None);
    }
}
