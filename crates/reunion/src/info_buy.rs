//! INFO-BUY (screen 5, with its list and amount icon sets 14 and 15): what
//! you've invented, and ordering it from New Earth's factories.
//!
//! From REUNION.PRG FUN_32b4_0057 (logic) and the rest of segment 32b4:
//!
//! - GRAFIKA/INFO at row 49: the spinning model on the left (see
//!   `model_view`), on the right either the item's picture (INFO/INFO<n>,
//!   180x115 from (6, 6)) or its description (TEXT/SZ_TALAL.RAW: a title
//!   and six lines), which the right half toggles;
//! - under the description the ore one piece needs; at the bottom what's
//!   ordered, how long it takes, the stores and the total price, or why it
//!   can't be bought;
//! - SELECT swaps the model for a list of all inventions (GRAFIKA/SELECT);
//!   PROJECT UP / DOWN step through them;
//! - BUY ITEM (for what New Earth can make) sets how many to order: the
//!   price and ore of the current order are refunded, then ADD / MINUS
//!   change the amount within your money and ore, and OK,BUY charges the
//!   new order (CANCEL BUY the old one again).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::state::state::StateTransitionEvent;

use crate::focus::{Activated, DefaultFocus, Hover, hotspot};
use crate::game::Game;
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, HoverLabel, IconSetOverride, RED_TEXT, YELLOW_TEXT};
use crate::model_view::{ModelView, VecModel};
use crate::screen::{GameScreen, picture, place};
use crate::text::{Label, label};

pub struct InfoBuyPlugin;

impl Plugin for InfoBuyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::InfoBuy), enter.after(crate::hud::spawn_hud))
            .add_systems(
                Update,
                spawn_view.run_if(in_state(GameScreen::InfoBuy)),
            )
            .add_systems(OnExit(GameScreen::InfoBuy), |mut commands: Commands| {
                commands.remove_resource::<View>();
            })
            .add_observer(activate)
            .add_observer(use_action);
    }
}

const BUY_ITEM: u8 = 18;
const SELECT: u8 = 31;
const SELECT_OFF: u8 = 35;
const PROJECT_UP: u8 = 22;
const PROJECT_DOWN: u8 = 21;
const ADD_ONE: u8 = 26;
const MINUS_ONE: u8 = 27;
const ADD_TEN: u8 = 28;
const MINUS_TEN: u8 = 29;
const OK_BUY: u8 = 30;
const CANCEL_BUY: u8 = 32;
/// Icon sets of the list and of the amount.
const LIST_SET: u8 = 14;
const AMOUNT_SET: u8 = 15;

const INVENTIONS: u8 = 35;
/// Research's selected invention (DS:0x95a0), preselected coming from there.
const RESEARCH_SELECTED: u16 = 0x95a0;
/// Builders at work, who share the production (DS:0x95b4).
const BUILDERS: u16 = 0x95b4;
/// Ore stock: six longs from DS:0x95c6.
const ORES: u16 = 0x95c6;
const MONEY: u16 = 0x95be;
const LIST_ROWS: usize = 13;
/// Ores in the order FUN_32b4_10bd lays them out: left, right, left, ...
const ORE_ORDER: [usize; 6] = [1, 3, 2, 6, 5, 4];

/// Invention record fields (DS:0x5d77 + 0x35 * n).
mod field {
    pub const STATUS: u16 = 0x11;
    /// u32.
    pub const PRICE: u16 = 0x17;
    pub const STORES: u16 = 0x1b;
    pub const ORDERED: u16 = 0x1d;
    /// Builder-hours per piece, and left on the piece in production.
    pub const HOURS: u16 = 0x1f;
    pub const HOURS_LEFT: u16 = 0x21;
    /// Bytes: New Earth can make it; colonies can build it.
    pub const MADE_ON_NEW_EARTH: u16 = 0x23;
    pub const BUILT_ON_COLONIES: u16 = 0x24;
    /// Six u16s: ore per piece.
    pub const ORE: u16 = 0x25;
}

#[derive(Resource, Clone, PartialEq)]
struct View {
    /// Inventions you have (status 5), in order.
    products: Vec<u8>,
    /// Index into `products`, and the first row of the list.
    current: usize,
    top: usize,
    list: bool,
    picture: bool,
    buying: Option<Purchase>,
}

/// An order being set: its amount, and the money and ore to spend it from
/// (with the current order refunded).
#[derive(Clone, PartialEq)]
struct Purchase {
    amount: u32,
    money: u32,
    ores: [u32; 6],
}

#[derive(Component, Clone)]
struct ViewPart;

#[derive(Component, Clone, Copy, PartialEq)]
enum Button {
    TogglePicture,
    Select,
    Row(usize),
    ScrollUp,
    ScrollDown,
}

fn address(invention: u8, field: u16) -> u16 {
    0x5d77 + 0x35 * u16::from(invention) + field
}

fn word(game: &Game, invention: u8, field: u16) -> u16 {
    game.0.word(address(invention, field)).unwrap_or(0)
}

fn long(game: &Game, at: u16) -> u32 {
    let lo = game.0.word(at).unwrap_or(0) as u32;
    let hi = game.0.word(at + 2).unwrap_or(0) as u32;
    hi << 16 | lo
}

fn set_long(game: &mut Game, at: u16, value: u32) {
    game.0.set_word(at, value as u16);
    game.0.set_word(at + 2, (value >> 16) as u16);
}

fn ore_need(game: &Game, invention: u8, ore: usize) -> u32 {
    u32::from(word(game, invention, field::ORE + 2 * (ore as u16 - 1)))
}

fn can_buy(game: &Game, invention: u8) -> bool {
    game.0.byte(address(invention, field::MADE_ON_NEW_EARTH)).unwrap_or(0) != 0
        || (game.0.byte(address(invention, field::BUILT_ON_COLONIES)).unwrap_or(0) != 0
            && game.0.word(0x6280) == Some(5))
}

fn enter(
    mut commands: Commands,
    mut transitions: MessageReader<StateTransitionEvent<GameScreen>>,
    game: Option<Res<Game>>,
) {
    let Some(game) = game else { return };
    // FUN_32b4_0c50: the finished inventions; coming from research, its choice.
    let products: Vec<u8> = (1..=INVENTIONS)
        .filter(|&i| word(&game, i, field::STATUS) == 5)
        .collect();
    let from_research = transitions.read().last().and_then(|t| t.exited) == Some(GameScreen::Research);
    let wanted = game.0.word(RESEARCH_SELECTED).unwrap_or(0) as u8;
    let current = if from_research {
        products.iter().position(|&p| p == wanted).unwrap_or(0)
    } else {
        0
    };
    commands.insert_resource(View {
        products,
        current,
        top: 0,
        list: false,
        picture: true,
        buying: None,
    });
}

fn spawn_view(
    mut commands: Commands,
    view: Option<Res<View>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    parts: Query<Entity, With<ViewPart>>,
    mut images: ResMut<Assets<Image>>,
    mut set: ResMut<IconSetOverride>,
    mut shown: Local<Option<(u8, bool, bool, Option<Purchase>, Vec<u8>)>>,
) {
    let (Some(view), Some(game), Some(data)) = (view, game, data.get(&handle.0)) else {
        return;
    };
    let invention = view.products.get(view.current).copied().unwrap_or(0);
    // Redraw when the view or the invention's record changes.
    let record = game.0.block(0x5dac).map(|b| b.to_vec()).unwrap_or_default();
    let key = (invention, view.list, view.picture, view.buying.clone(), record);
    if !view.is_changed() && shown.as_ref() == Some(&key) {
        return;
    }
    *shown = Some(key);

    let wanted_set = if view.buying.is_some() {
        Some(AMOUNT_SET)
    } else if view.list {
        Some(LIST_SET)
    } else {
        None
    };
    if set.0 != wanted_set {
        set.0 = wanted_set;
    }
    for part in &parts {
        commands.entity(part).despawn();
    }
    let scoped = (ViewPart, DespawnOnExit(GameScreen::InfoBuy));
    commands.spawn((
        picture(asset_server.load("GRAFIKA/INFO.PIC"), Vec2::new(0.0, CONTENT_Y)),
        scoped.clone(),
    ));
    let text = |commands: &mut Commands, s: String, columns: usize, colors, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, colors), pos), scoped.clone()));
    };
    let button = |commands: &mut Commands, b: Button, rect: Rect, name: &str| {
        commands.spawn((b, HoverLabel(name.into()), hotspot(rect, Hover::Outline), scoped.clone()));
    };
    if invention == 0 {
        return;
    }

    // Left: the model, or the list.
    if view.list {
        commands.spawn((
            Sprite {
                image: asset_server.load("GRAFIKA/SELECT.PIC"),
                rect: Some(Rect::new(0.0, 0.0, 126.0, 126.0)),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(Vec2::new(0.0, CONTENT_Y), 0.1),
            scoped.clone(),
        ));
        for row in 0..LIST_ROWS {
            let i = view.top + row;
            let Some(&product) = view.products.get(i) else { break };
            let name = data.exe.ds_string(address(product, 0)).unwrap_or_default();
            let y = 55.0 + 9.0 * row as f32;
            let colors = if i == view.current { RED_TEXT } else { YELLOW_TEXT };
            text(&mut commands, name.clone(), 16, colors, Vec2::new(16.0, y));
            let mut entity = commands.spawn((
                Button::Row(i),
                HoverLabel(name),
                hotspot(Rect::new(14.0, y - 1.0, 112.0, y + 8.0), Hover::Outline),
                scoped.clone(),
            ));
            if i == view.current {
                entity.insert(DefaultFocus);
            }
        }
        button(&mut commands, Button::ScrollUp, Rect::new(114.0, 49.0, 126.0, 67.0), "Scroll up");
        button(&mut commands, Button::ScrollDown, Rect::new(114.0, 158.0, 126.0, 176.0), "Scroll down");
    } else {
        let spin = data.exe.ds_bytes(0x5713 + 3 * u16::from(invention), 3).unwrap_or(&[5, 0, 0]);
        let zoom = data.exe.ds_bytes(0x56cd + u16::from(invention), 1).map_or(60, |b| b[0]);
        let mode = data.exe.ds_bytes(0x56f1 + u16::from(invention), 1).map_or(1, |b| b[0]);
        let model = asset_server.load::<VecModel>(format!("VECTORS/V{invention}.VEC"));
        let view_model = ModelView::new(model, &mut images, [spin[0], spin[1], spin[2]], zoom, mode, invention);
        commands.spawn((
            Sprite::from_image(view_model.image.clone()),
            view_model,
            Anchor::TOP_LEFT,
            place(Vec2::new(6.0, 55.0), 0.2),
            scoped.clone(),
        ));
        button(&mut commands, Button::Select, Rect::new(0.0, 49.0, 128.0, 176.0), "Select");
    }

    // Right: the picture or the description.
    let toggle = if view.picture { "See info" } else { "See picture" };
    button(&mut commands, Button::TogglePicture, Rect::new(129.0, 49.0, 319.0, 176.0), toggle);
    if view.picture {
        commands.spawn((
            Sprite {
                image: asset_server.load(format!("INFO/INFO{invention}.PIC")),
                rect: Some(Rect::new(6.0, 6.0, 186.0, 121.0)),
                ..default()
            },
            Anchor::TOP_LEFT,
            place(Vec2::new(134.0, 55.0), 0.2),
            scoped.clone(),
        ));
    } else {
        commands.spawn((
            Sprite::from_color(Color::BLACK, Vec2::new(180.0, 115.0)),
            Anchor::TOP_LEFT,
            place(Vec2::new(134.0, 55.0), 0.2),
            scoped.clone(),
        ));
        if let Some(lines) = data.descriptions.get(invention as usize - 1)
            && view.buying.is_none() {
                text(&mut commands, lines[0].clone(), 29, RED_TEXT, Vec2::new(136.0, 59.0));
                for (k, line) in lines[1..].iter().enumerate() {
                    text(&mut commands, line.clone(), 29, YELLOW_TEXT, Vec2::new(136.0, 71.0 + 10.0 * k as f32));
                }
            }
        ore_lines(&mut commands, &scoped, &game, data, invention, view.buying.as_ref());
    }

    bottom_lines(&mut commands, &scoped, &game, invention, view.buying.as_ref());
}

/// FUN_32b4_1003 / 10bd (and 0d21 / 1141 / 1314 while buying): the ore
/// one piece (or the order) needs, and the ore in stock.
fn ore_lines(
    commands: &mut Commands,
    scoped: &(ViewPart, DespawnOnExit<GameScreen>),
    game: &Game,
    data: &GameData,
    invention: u8,
    buying: Option<&Purchase>,
) {
    let ore_name = |i: usize| {
        data.exe
            .ds_string(0x5a73 + 9 * i as u16)
            .unwrap_or_default()
            .trim_end()
            .to_string()
    };
    // "Name:   123", the number in a 6-wide field whose first space is a colon.
    let entry = |i: usize, value: u32| {
        let mut n = format!("{value:>6}");
        if n.starts_with(' ') {
            n.replace_range(0..1, ":");
        }
        format!("{}{n}", ore_name(i))
    };
    let place_of = |k: usize, y0: f32| {
        let x = if k.is_multiple_of(2) { 142.0 } else { 225.0 };
        (Vec2::new(x, y0 + 9.0 * (k / 2) as f32), if k.is_multiple_of(2) { 13 } else { 14 })
    };
    let mut text = |s: String, columns: usize, colors, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, colors), pos), scoped.clone()));
    };
    match buying {
        None => {
            text("Ore needed: (One piece)".into(), 29, RED_TEXT, Vec2::new(136.0, 133.0));
            for (k, &ore) in ORE_ORDER.iter().enumerate() {
                let (pos, columns) = place_of(k, 142.0);
                text(entry(ore, ore_need(game, invention, ore)), columns, YELLOW_TEXT, pos);
            }
        }
        Some(p) => {
            text("Ores in stock".into(), 29, RED_TEXT, Vec2::new(136.0, 73.0));
            for (k, &ore) in ORE_ORDER.iter().enumerate() {
                let (pos, columns) = place_of(k, 82.0);
                text(entry(ore, p.ores[ore - 1]), columns, YELLOW_TEXT, pos);
            }
            let pieces = if p.amount > 1 { "pieces )" } else { "piece )" };
            text(format!("Ore needed: ({} {pieces}", p.amount), 29, RED_TEXT, Vec2::new(136.0, 133.0));
            for (k, &ore) in ORE_ORDER.iter().enumerate() {
                let (pos, columns) = place_of(k, 142.0);
                let need = ore_need(game, invention, ore);
                // Red where one more piece wouldn't fit the stock.
                let short = need * (p.amount + 1) > p.ores[ore - 1];
                let colors = if short { RED_TEXT } else { YELLOW_TEXT };
                text(entry(ore, need * p.amount), columns, colors, pos);
            }
        }
    }
}

/// FUN_32b4_18a1 / 1666 / 1474: the order, its time and price, or why it
/// can't be bought.
fn bottom_lines(
    commands: &mut Commands,
    scoped: &(ViewPart, DespawnOnExit<GameScreen>),
    game: &Game,
    invention: u8,
    buying: Option<&Purchase>,
) {
    let mut text = |s: String, columns: usize, colors, pos: Vec2| {
        commands.spawn((label(Label::new(s, columns, colors), pos), scoped.clone()));
    };
    let ordered = buying.map_or(u32::from(word(game, invention, field::ORDERED)), |p| p.amount);
    let stores = word(game, invention, field::STORES);
    if ordered == 0 && buying.is_none() {
        if can_buy(game, invention) {
            text("Stores:".into(), 19, YELLOW_TEXT, Vec2::new(14.0, 189.0));
            text(format!("{stores}"), 10, RED_TEXT, Vec2::new(100.0, 189.0));
        } else {
            let why = match invention {
                8 | 12 | 34 => "You only need one of this item.",
                14 | 18 => "This item will be fitted on the ships",
                28 => "This is fitted on the colony centres",
                23 => "This cannot be produced on New Earth",
                1 | 7 | 29 | 30 | 35 => "This can be built on your colonies",
                _ => "This cannot be produced",
            };
            text(why.into(), 40, RED_TEXT, Vec2::new(14.0, 189.0));
        }
        return;
    }
    // Hours: (builders - 1 + hours per piece * pieces still to start + hours
    // left on the one in production) / builders, rounding up.
    let builders = u32::from(game.0.word(BUILDERS).unwrap_or(1)).max(1);
    let per_piece = u32::from(word(game, invention, field::HOURS));
    let left = u32::from(word(game, invention, field::HOURS_LEFT));
    let hours = if left == 0 {
        builders + per_piece * ordered - 1
    } else {
        builders + per_piece * ordered.saturating_sub(1) + left - 1
    } / builders;
    let price = long(game, address(invention, field::PRICE));
    text("Bought items:".into(), 15, YELLOW_TEXT, Vec2::new(14.0, 180.0));
    text(format!("{ordered}"), 5, RED_TEXT, Vec2::new(100.0, 180.0));
    text("Time to go:".into(), 17, YELLOW_TEXT, Vec2::new(160.0, 180.0));
    text(format!("{hours}"), 6, RED_TEXT, Vec2::new(264.0, 180.0));
    text("Stores:".into(), 19, YELLOW_TEXT, Vec2::new(14.0, 189.0));
    text(format!("{stores}"), 10, RED_TEXT, Vec2::new(100.0, 189.0));
    text("Total price:".into(), 17, YELLOW_TEXT, Vec2::new(160.0, 189.0));
    text(format!("{}", price.saturating_mul(ordered)), 8, RED_TEXT, Vec2::new(264.0, 189.0));
}

fn activate(activated: On<Activated>, buttons: Query<&Button>, view: Option<ResMut<View>>) {
    let (Ok(&button), Some(mut view)) = (buttons.get(activated.0), view) else {
        return;
    };
    if view.buying.is_some() && button != Button::TogglePicture {
        return;
    }
    match button {
        Button::TogglePicture => view.picture = !view.picture,
        Button::Select => view.list = true,
        Button::Row(i) => view.current = i,
        Button::ScrollUp => {
            // FUN_32b4_1de0 / 1e00: the list and the choice move together.
            if view.top > 0 {
                view.top -= 1;
                if view.current >= view.top + LIST_ROWS {
                    view.current -= 1;
                }
            }
        }
        Button::ScrollDown => {
            if view.top + LIST_ROWS < view.products.len() {
                view.top += 1;
                if view.current < view.top {
                    view.current += 1;
                }
            }
        }
    }
}

/// The largest amount within money, ore and (for colony buildings) space
/// stations, and at least one while a piece is in production.
fn clamp(game: &Game, invention: u8, p: &mut Purchase) {
    let price = long(game, address(invention, field::PRICE));
    if let Some(affordable) = p.money.checked_div(price) {
        p.amount = p.amount.min(affordable);
    }
    for ore in 1..=6 {
        let need = ore_need(game, invention, ore);
        if let Some(enough) = p.ores[ore - 1].checked_div(need) {
            p.amount = p.amount.min(enough);
        }
    }
    if game.0.byte(address(invention, field::BUILT_ON_COLONIES)).unwrap_or(0) != 0 {
        p.amount = p.amount.min(u32::from(game.0.word(0x628a).unwrap_or(0)));
    }
    if word(game, invention, field::HOURS_LEFT) > 0 && p.amount == 0 {
        p.amount = 1;
    }
}

fn use_action(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    view: Option<ResMut<View>>,
    game: Option<ResMut<Game>>,
) {
    if *screen.get() != GameScreen::InfoBuy {
        return;
    }
    let (Some(mut view), Some(mut game)) = (view, game) else {
        return;
    };
    let Some(&invention) = view.products.get(view.current) else {
        return;
    };
    let count = view.products.len();
    match action.0 {
        SELECT => view.list = true,
        SELECT_OFF => view.list = false,
        PROJECT_UP if view.current > 0 => {
            view.current -= 1;
            view.top = view.top.min(view.current);
        }
        PROJECT_DOWN if view.current + 1 < count => {
            view.current += 1;
            if view.current >= view.top + LIST_ROWS {
                view.top = view.current + 1 - LIST_ROWS;
            }
        }
        BUY_ITEM if view.buying.is_none() && can_buy(&game, invention) => {
            // The current order's price and ore become available again.
            let ordered = u32::from(word(&game, invention, field::ORDERED));
            let price = long(&game, address(invention, field::PRICE));
            let mut purchase = Purchase {
                amount: ordered,
                money: game.0.money().saturating_add(price * ordered),
                ores: std::array::from_fn(|i| {
                    long(&game, ORES + 4 * i as u16) + ore_need(&game, invention, i + 1) * ordered
                }),
            };
            clamp(&game, invention, &mut purchase);
            view.buying = Some(purchase);
        }
        ADD_ONE | MINUS_ONE | ADD_TEN | MINUS_TEN | OK_BUY | CANCEL_BUY => {
            let Some(mut purchase) = view.buying.clone() else {
                return;
            };
            match action.0 {
                ADD_ONE => purchase.amount += 1,
                MINUS_ONE => purchase.amount = purchase.amount.saturating_sub(1),
                ADD_TEN => purchase.amount += 10,
                MINUS_TEN => purchase.amount = purchase.amount.saturating_sub(10),
                CANCEL_BUY => purchase.amount = u32::from(word(&game, invention, field::ORDERED)),
                _ => {}
            }
            clamp(&game, invention, &mut purchase);
            if matches!(action.0, OK_BUY | CANCEL_BUY) {
                if action.0 == OK_BUY {
                    let hours_left = address(invention, field::HOURS_LEFT);
                    if purchase.amount > 0 && game.0.word(hours_left) == Some(0) {
                        let per_piece = word(&game, invention, field::HOURS);
                        game.0.set_word(hours_left, per_piece);
                    }
                    game.0.set_word(address(invention, field::ORDERED), purchase.amount as u16);
                }
                let price = long(&game, address(invention, field::PRICE));
                set_long(&mut game, MONEY, purchase.money - price * purchase.amount);
                for ore in 1..=6 {
                    let left = purchase.ores[ore - 1] - ore_need(&game, invention, ore) * purchase.amount;
                    set_long(&mut game, ORES + 4 * (ore as u16 - 1), left);
                }
                view.buying = None;
            } else {
                view.buying = Some(purchase);
            }
        }
        _ => {}
    }
}
