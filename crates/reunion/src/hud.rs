//! The in-game interface shared by every game screen: the icon bar (rows
//! 0-31), the text strip (rows 32-48) with the hover label, money and date.
//!
//! Icon bar (FUN_3a1b_00bf): 48x32 frames from ICONMAIN.PIC with a 40x24 icon
//! at (4, 4) in each, and a 32x32 corner button that pages through the set.
//! Each screen has its own set (the screen number), and each action opens the
//! screen whose trigger it is (the loop at entry's icon handling).
//!
//! Widescreen: the icon bar fits as many frames as the view allows (6 in 4:3,
//! 7 on 16:10) and the text strip stretches its boxes.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::focus::{Activated, DefaultFocus, Focus, Hotspot, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle, SharedPictures};
use crate::pic::rgba_image;
use crate::screen::{GAME_WIDTH, GameScreen, ViewBounds, in_game, place};
use crate::transition::GoTo;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IconPage>();
        for screen in GameScreen::IN_GAME {
            app.add_systems(OnEnter(screen), spawn_hud);
        }
        app.add_systems(
            Update,
            (
                layout_icon_bar,
                layout_text_strip,
                update_hover_label,
                update_status,
            )
                .chain()
                .run_if(in_game),
        )
        .add_observer(on_activated);
    }
}

pub const TEXT_Y: f32 = 32.0;
/// First row below the text strip.
pub const CONTENT_Y: f32 = 49.0;

const MIN_SLOTS: usize = 6;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
const CORNER_WIDTH: f32 = 32.0;
/// Pieces of ICONMAIN.PIC: empty frame, up arrow, down arrow, disabled.
const FRAME: Rect = Rect::new(0.0, 0.0, 48.0, 32.0);
const PAGE_UP: Rect = Rect::new(48.0, 0.0, 80.0, 32.0);
const PAGE_DOWN: Rect = Rect::new(80.0, 0.0, 112.0, 32.0);
const NO_PAGES: Rect = Rect::new(112.0, 0.0, 144.0, 32.0);

/// TEXT.PIC is three boxes whose middles are uniform columns; everything else
/// (caps and connectors) is kept as-is. Extra width goes to boxes 2 and 3.
const STRIP_BOX_MIDDLES: [(u32, u32); 3] = [(13, 127), (141, 207), (221, 307)];

/// Where the hover label goes relative to the strip, and its width (FUN_405f_11e8).
const LABEL_OFFSET: Vec2 = Vec2::new(16.0, 37.0 - TEXT_Y);
const LABEL_COLUMNS: usize = 18;
/// Palette entries 52-54 set by FUN_321d_0866, 6-bit VGA values scaled to 8 bits.
pub const TEXT_COLORS: [[u8; 4]; 3] = [[0, 0, 0, 255], [60, 137, 226, 255], [28, 105, 186, 255]];

/// Money (box 2, 10 characters) and date (box 3, 13 characters), drawn by
/// FUN_405f_140a and FUN_405f_134c at these x positions in the strip.
const MONEY_X: f32 = 144.0;
const MONEY_COLUMNS: usize = 10;
const DATE_X: f32 = 224.0;
const DATE_COLUMNS: usize = 13;

/// Room hotspot names that aren't in the label table, mapped to the action
/// with the matching icon. Inferred from the names, not from the code.
const LABEL_ALIASES: [(&str, &str); 2] = [("STARMAP", "GALACTIC MAP"), ("MESSAGE", "MESSAGES")];
const BACK_TO_MAIN: u8 = 24;

/// First action of the icon set shown in the bar.
#[derive(Resource, Default)]
struct IconPage(usize);

/// Everything in the icon bar; rebuilt when the view width or page changes.
#[derive(Component)]
struct BarPart;

/// What a bar hotspot is, so focus can survive a rebuild.
#[derive(Component, Clone, Copy, PartialEq)]
enum BarRole {
    Slot(usize),
    PageToggle,
}

/// Clicking this hotspot performs an action from the label table.
#[derive(Component)]
pub struct IconAction(pub u8);

/// Clicking this hotspot performs the action whose label matches its hover label.
#[derive(Component)]
pub struct ActionByLabel;

#[derive(Component)]
pub struct HoverLabel(pub String);

#[derive(Component)]
struct TextStrip;

#[derive(Component)]
struct LabelText;

#[derive(Component)]
struct MoneyText;

#[derive(Component)]
struct DateText;

fn spawn_hud(
    mut commands: Commands,
    screen: Res<State<GameScreen>>,
    mut page: ResMut<IconPage>,
    mut focus: ResMut<Focus>,
) {
    let scoped = DespawnOnExit(*screen.get());
    page.0 = 0;
    // The previous screen's hotspots are gone; the icon bar picks a new default.
    focus.0 = None;
    commands.spawn((
        TextStrip,
        Sprite::default(),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, TEXT_Y), 0.0),
        scoped.clone(),
    ));
    commands.spawn((
        LabelText,
        Sprite::default(),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, TEXT_Y) + LABEL_OFFSET, 2.0),
        scoped.clone(),
    ));
    commands.spawn((
        MoneyText,
        Sprite::default(),
        Anchor::TOP_LEFT,
        Transform::default(),
        scoped.clone(),
    ));
    commands.spawn((
        DateText,
        Sprite::default(),
        Anchor::TOP_LEFT,
        Transform::default(),
        scoped,
    ));
}

fn update_status(
    game: Option<Res<Game>>,
    bounds: Res<ViewBounds>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut money: Single<(&mut Sprite, &mut Transform), (With<MoneyText>, Without<DateText>)>,
    mut date: Single<(&mut Sprite, &mut Transform), (With<DateText>, Without<MoneyText>)>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(game), Some(data)) = (game, data.get(&handle.0)) else {
        return;
    };
    if !game.is_changed() && !bounds.is_changed() && money.0.image != Handle::default() {
        return;
    }
    let state = &game.0;
    let [year, month, day, hour] = state.date();
    let (money_sprite, money_transform) = &mut *money;
    money_sprite.image = images.add(data.font.render(
        &state.money().to_string(),
        MONEY_COLUMNS,
        TEXT_COLORS,
    ));
    **money_transform = place(Vec2::new(bounds.left + MONEY_X, 37.0), 2.0);
    // Box 3 moves right by the extra width box 2 gets (see stretch_strip).
    let box2_growth = ((bounds.width() as u32).saturating_sub(GAME_WIDTH as u32) / 2) as f32;
    let (date_sprite, date_transform) = &mut *date;
    date_sprite.image = images.add(data.font.render(
        &format!("{year}-{month}-{day}-{hour}"),
        DATE_COLUMNS,
        TEXT_COLORS,
    ));
    **date_transform = place(Vec2::new(bounds.left + DATE_X + box2_growth, 37.0), 2.0);
}

/// The 32x32 button after the frames.
#[derive(Debug, PartialEq, Clone, Copy)]
enum Corner {
    /// Wide enough to show every action; no button.
    None,
    /// Six or fewer actions: the original's crossed-out button.
    Disabled,
    Paging,
}

/// How the icon bar fits in a view: frame count, corner button and left edge.
#[derive(Debug, PartialEq)]
struct BarLayout {
    slots: usize,
    corner: Corner,
    left: f32,
}

fn bar_layout(bounds: ViewBounds, actions: usize) -> BarLayout {
    let fit_all = ((bounds.width() - 1.0) / CELL_WIDTH).floor() as usize;
    let with_corner = ((bounds.width() - 1.0 - CORNER_WIDTH) / CELL_WIDTH).floor() as usize;
    let (slots, corner) = if actions <= MIN_SLOTS {
        // As many (empty) frames as fit, so the bar spans the screen like the others.
        (with_corner.max(MIN_SLOTS), Corner::Disabled)
    } else if fit_all >= actions {
        (actions, Corner::None)
    } else {
        (with_corner.max(MIN_SLOTS), Corner::Paging)
    };
    let width = 1.0
        + slots as f32 * CELL_WIDTH
        + if corner == Corner::None {
            0.0
        } else {
            CORNER_WIDTH
        };
    // In 4:3 this gives the original positions: frames at x 1, 49, ... corner at 289.
    let left = ((GAME_WIDTH - width) / 2.0).ceil();
    BarLayout {
        slots,
        corner,
        left,
    }
}

fn icon_set<'a>(data: &'a GameData, screen: &State<GameScreen>) -> &'a [u8] {
    screen
        .get()
        .number()
        .and_then(|n| data.icon_sets.get(n as usize))
        .map_or(&[], Vec::as_slice)
}

fn layout_icon_bar(
    mut commands: Commands,
    pictures: Res<SharedPictures>,
    screen: Res<State<GameScreen>>,
    bounds: Res<ViewBounds>,
    mut page: ResMut<IconPage>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    parts: Query<Entity, With<BarPart>>,
    roles: Query<&BarRole>,
    mut focus: ResMut<Focus>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(data) = data.get(&handle.0) else {
        return;
    };
    if !parts.is_empty() && !bounds.is_changed() && !page.is_changed() {
        return;
    }
    // The edge fillers are cut from ICONMAIN.PIC, so wait for its pixels.
    if images.get(&pictures.icon_frames).is_none() {
        return;
    }
    let actions = icon_set(data, &screen);
    let layout = bar_layout(*bounds, actions.len());
    if page.0 >= actions.len() || layout.corner != Corner::Paging {
        page.0 = 0;
    }
    let focused_role = focus.0.and_then(|e| roles.get(e).ok()).copied();
    for part in &parts {
        commands.entity(part).try_despawn();
    }

    let scoped = DespawnOnExit(*screen.get());
    let icon_main = pictures.icon_frames.clone();
    let mut restored = None;
    for slot in 0..layout.slots {
        let x = layout.left + 1.0 + slot as f32 * CELL_WIDTH;
        commands.spawn((
            BarPart,
            Sprite {
                image: icon_main.clone(),
                rect: Some(FRAME),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(Vec2::new(x, 0.0), 0.0),
            scoped.clone(),
        ));
        let Some(&action) = actions.get(page.0 + slot) else {
            continue; // an empty frame
        };
        if let Some(icon) = data.action_icons[action as usize].clone() {
            commands.spawn((
                BarPart,
                Sprite::from_image(icon),
                Anchor::TOP_LEFT,
                place(Vec2::new(x + 4.0, 4.0), 0.5),
                scoped.clone(),
            ));
        }
        let role = BarRole::Slot(slot);
        let cell = Rect::new(x, 0.0, x + CELL_WIDTH, CELL_HEIGHT);
        let entity = commands
            .spawn((
                BarPart,
                role,
                IconAction(action),
                HoverLabel(data.labels[action as usize].clone()),
                hotspot(cell, Hover::Outline),
                scoped.clone(),
            ))
            .id();
        if focused_role == Some(role) || (focus.0.is_none() && slot == 0 && restored.is_none()) {
            restored = Some(entity);
        }
    }
    let corner_x = layout.left + 1.0 + layout.slots as f32 * CELL_WIDTH;
    let corner_piece = match layout.corner {
        Corner::None => None,
        Corner::Disabled => Some(NO_PAGES),
        Corner::Paging if page.0 + layout.slots >= actions.len() => Some(PAGE_UP),
        Corner::Paging => Some(PAGE_DOWN),
    };
    if let Some(piece) = corner_piece {
        commands.spawn((
            BarPart,
            Sprite {
                image: icon_main.clone(),
                rect: Some(piece),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(Vec2::new(corner_x, 0.0), 0.0),
            scoped.clone(),
        ));
    }
    // Frames are 48 wide, so a gap is left at the screen edges; fill it with
    // frame edge caps and the frame's top and bottom border.
    let bar_start = layout.left + 1.0;
    let bar_end = corner_x
        + if corner_piece.is_some() {
            CORNER_WIDTH
        } else {
            0.0
        };
    let gaps = [
        (bounds.left, bar_start - bounds.left, Edge::Left),
        (bar_end, bounds.right - bar_end, Edge::Right),
    ];
    for (x, width, edge) in gaps {
        let Some(filler) = images
            .get(&icon_main)
            .and_then(|frames| frame_filler(frames, width as u32, edge))
        else {
            continue;
        };
        commands.spawn((
            BarPart,
            Sprite::from_image(images.add(filler)),
            Anchor::TOP_LEFT,
            place(Vec2::new(x, 0.0), 0.0),
            scoped.clone(),
        ));
    }
    if layout.corner == Corner::Paging {
        let x = corner_x;
        let corner = Rect::new(x, 0.0, x + CORNER_WIDTH, CELL_HEIGHT);
        let entity = commands
            .spawn((
                BarPart,
                BarRole::PageToggle,
                hotspot(corner, Hover::Outline),
                scoped,
            ))
            .id();
        if focused_role == Some(BarRole::PageToggle) {
            restored = Some(entity);
        }
    }
    if let Some(entity) = restored {
        if focus.0.is_none() {
            commands.entity(entity).try_insert(DefaultFocus);
        } else {
            focus.0 = Some(entity);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Edge {
    Left,
    Right,
}

/// Columns of the empty frame used as the cap at a screen edge.
const FILLER_CAP: u32 = 3;
/// A column of the empty frame with only its top and bottom border.
const FILLER_MIDDLE: u32 = 24;

/// A `width`-wide piece of empty frame: the frame's outer edge at the screen
/// edge, its top and bottom border repeated towards the bar.
fn frame_filler(frames: &Image, width: u32, edge: Edge) -> Option<Image> {
    if width == 0 {
        return None;
    }
    let data = frames.data.as_ref()?;
    let (sheet_width, height) = (frames.width(), CELL_HEIGHT as u32);
    let frame_width = FRAME.width() as u32;
    let cap = FILLER_CAP.min(width);
    let columns: Vec<u32> = (0..width)
        .map(|i| match edge {
            Edge::Left if i < cap => i,
            Edge::Right if i >= width - cap => frame_width - (width - i),
            _ => FILLER_MIDDLE,
        })
        .collect();
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for &x in &columns {
            let i = ((y * sheet_width + x) * 4) as usize;
            rgba.extend_from_slice(data.get(i..i + 4)?);
        }
    }
    Some(rgba_image(width, height, rgba))
}

fn layout_text_strip(
    pictures: Res<SharedPictures>,
    bounds: Res<ViewBounds>,
    mut strip: Single<(&mut Sprite, &mut Transform), (With<TextStrip>, Without<LabelText>)>,
    mut label: Single<&mut Transform, (With<LabelText>, Without<TextStrip>)>,
    mut images: ResMut<Assets<Image>>,
) {
    let (sprite, transform) = &mut *strip;
    if sprite.image != Handle::default() && !bounds.is_changed() {
        return;
    }
    let Some(wide) = images
        .get(&pictures.text_strip)
        .and_then(|image| stretch_strip(image, bounds.width() as u32))
    else {
        return; // TEXT.PIC still loading
    };
    sprite.image = images.add(wide);
    **transform = place(Vec2::new(bounds.left, TEXT_Y), 0.0);
    **label = place(Vec2::new(bounds.left, TEXT_Y) + LABEL_OFFSET, 2.0);
}

/// Widens the text strip by repeating the middle column of boxes 2 and 3.
fn stretch_strip(image: &Image, width: u32) -> Option<Image> {
    let data = image.data.as_ref()?;
    let (src_width, height) = (image.width(), image.height());
    let extra = width.saturating_sub(src_width);
    let grow = [0, extra / 2, extra - extra / 2];

    // Source column for every output column.
    let mut columns = Vec::with_capacity(width as usize);
    let mut from = 0;
    for ((start, end), grow) in STRIP_BOX_MIDDLES.into_iter().zip(grow) {
        columns.extend(from..end);
        columns.extend(std::iter::repeat_n(start, grow as usize));
        from = end;
    }
    columns.extend(from..src_width);

    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for &x in &columns {
            let i = ((y * src_width + x) * 4) as usize;
            rgba.extend_from_slice(&data[i..i + 4]);
        }
    }
    Some(rgba_image(columns.len() as u32, height, rgba))
}

fn update_hover_label(
    focus: Res<Focus>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    labels: Query<&HoverLabel>,
    mut text: Single<&mut Sprite, With<LabelText>>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Local<Option<String>>,
) {
    let Some(data) = data.get(&handle.0) else {
        return;
    };
    let label = focus
        .0
        .and_then(|e| labels.get(e).ok())
        .map(|l| l.0.clone())
        .unwrap_or_default();
    if shown.as_ref() == Some(&label) && text.image != Handle::default() {
        return;
    }
    text.image = images.add(data.font.render(&label, LABEL_COLUMNS, TEXT_COLORS));
    *shown = Some(label);
}

fn on_activated(
    activated: On<Activated>,
    hotspots: Query<
        (
            Option<&IconAction>,
            Option<&HoverLabel>,
            Option<&BarRole>,
            Has<ActionByLabel>,
        ),
        With<Hotspot>,
    >,
    screen: Res<State<GameScreen>>,
    bounds: Res<ViewBounds>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut page: ResMut<IconPage>,
    mut game: Option<ResMut<Game>>,
    mut commands: Commands,
) {
    let Ok((icon, label, role, by_label)) = hotspots.get(activated.0) else {
        return;
    };
    let Some(data) = data.get(&handle.0) else {
        return;
    };
    if role == Some(&BarRole::PageToggle) {
        let actions = icon_set(data, &screen).len();
        let slots = bar_layout(*bounds, actions).slots;
        page.0 = if page.0 + slots >= actions {
            0
        } else {
            page.0 + slots
        };
        return;
    }
    let label = label.map_or("", |l| l.0.as_str());
    let action = match icon {
        Some(IconAction(action)) => Some(*action),
        None if by_label => action_for_label(data, label),
        None => return,
    };
    let target = action.and_then(|a| screen_for_action(data, a));
    match target {
        Some(target) => {
            // Opening PLANET MAIN from the main screen looks at New Earth: system 1,
            // planet 5, no moon (entry, screen 0x14 set-up when coming from screen 1).
            if *screen.get() == GameScreen::MainScreen
                && target == GameScreen::PlanetMain
                && let Some(game) = game.as_mut()
            {
                game.select(1, 5, 0);
            }
            commands.trigger(GoTo(target));
        }
        None => info!("{label} is not implemented yet"),
    }
}

fn action_for_label(data: &GameData, label: &str) -> Option<u8> {
    let label = LABEL_ALIASES
        .iter()
        .find(|(from, _)| *from == label)
        .map_or(label, |(_, to)| to);
    data.labels.iter().position(|l| l == label).map(|a| a as u8)
}

/// The screen an action opens, if it's one we have.
fn screen_for_action(data: &GameData, action: u8) -> Option<GameScreen> {
    if action == BACK_TO_MAIN {
        return Some(GameScreen::MainScreen);
    }
    let number = data
        .screen_triggers
        .iter()
        .position(|t| *t == Some(action))?;
    GameScreen::from_number(number as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(width: f32) -> ViewBounds {
        let half = width / 2.0;
        ViewBounds {
            left: GAME_WIDTH / 2.0 - half,
            right: GAME_WIDTH / 2.0 + half,
        }
    }

    #[test]
    fn four_by_three_matches_the_original_bar() {
        // Frames at x 1, 49, ... 241 and the corner at 289.
        assert_eq!(
            bar_layout(bounds(320.0), 10),
            BarLayout {
                slots: 6,
                corner: Corner::Paging,
                left: 0.0
            }
        );
    }

    #[test]
    fn sixteen_by_ten_fits_seven() {
        let layout = bar_layout(bounds(384.0), 10);
        assert_eq!((layout.slots, layout.corner), (7, Corner::Paging));
        assert!(layout.left >= -32.0);
    }

    #[test]
    fn wide_enough_drops_paging() {
        assert_eq!(
            bar_layout(bounds(500.0), 10),
            BarLayout {
                slots: 10,
                corner: Corner::None,
                left: -80.0
            }
        );
    }

    #[test]
    fn short_sets_fill_the_width_too() {
        let layout = bar_layout(bounds(384.0), 4);
        assert_eq!((layout.slots, layout.corner), (7, Corner::Disabled));
    }

    #[test]
    fn short_sets_keep_six_frames() {
        // PLANET MAIN has 4 actions; the original draws six frames and a disabled corner.
        assert_eq!(
            bar_layout(bounds(320.0), 4),
            BarLayout {
                slots: 6,
                corner: Corner::Disabled,
                left: 0.0
            }
        );
    }
}
