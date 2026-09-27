//! SPACE BATTLE (screen 29): the fight in orbit around a planet.
//!
//! From REUNION.PRG segment 0x20d4 (the battle itself is
//! [`reunion_formats::battle`]): WAR/SPWAR at row 49, with the battle seen
//! through its round screen (the red): WAR/SPHATTER's stars and the ships,
//! 8x8 sprites from WAR/SPANTS (row 0 yours, row 1 the enemy's, rows 2-5
//! the explosions of the four kinds), and in front the back of your
//! fighter's head (WAR/SPFACE<hired fighter>, at (42, 144)). The icon bar
//! has RETREAT.
//!
//! When it's over (FUN_20d4_1de8): WAR/SPVICT or WAR/SPLOST with the losses
//! of both sides (hunters, fighters, destroyers, cruisers; at (87, 88 + 9k)
//! and (87, 143 + 9k)), and END BATTLE, after which the losers leave and the
//! game goes on at the galactic map.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use reunion_formats::battle::{EXPLOSION_FRAMES, HEIGHT, SpaceBattle, WIDTH};

use crate::game::{Game, random};
use crate::game_data::{GameData, GameDataHandle};
use crate::hud::{ActionUsed, CONTENT_Y, IconSetOverride, YELLOW_TEXT};
use crate::pic::{MASKED, rgba_image};
use crate::screen::{GameScreen, place};
use crate::text::{Label, label};
use crate::transition::GoTo;

pub struct SpaceBattlePlugin;

impl Plugin for SpaceBattlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameScreen::SpaceBattle), enter)
            .add_systems(
                Update,
                (key_screen, fight, draw).chain().run_if(in_state(GameScreen::SpaceBattle)),
            )
            .add_systems(OnExit(GameScreen::SpaceBattle), |mut commands: Commands| {
                commands.remove_resource::<Fight>();
            })
            .add_observer(act);
    }
}

const RETREAT: u8 = 62;
const END_BATTLE: u8 = 65;
/// Icon bar set with END BATTLE.
const BATTLE_OVER_SET: u8 = 30;
const VGA_REFRESH_HZ: f64 = 70.086;
/// SPWAR's round screen.
const SCREEN_RED: [u8; 3] = [224, 0, 0];

/// A battle to fight when SPACE BATTLE opens: where, and who started it.
#[derive(Resource, Clone, Copy, Debug)]
pub struct BattleStart {
    pub place: (u8, u8, u8),
    pub you_attack: bool,
    /// A fight for the ground follows a won attack (or a lost defence).
    pub ground: bool,
}

#[derive(Resource)]
struct Fight {
    battle: SpaceBattle,
    start: BattleStart,
    frames: f64,
    retreated: bool,
    /// Losses (yours, the enemy's) once it's over.
    result: Option<[[u32; 4]; 2]>,
    /// The battle area, redrawn every step.
    image: Handle<Image>,
    stars: Handle<Image>,
    sprites: Handle<Image>,
    changed: bool,
}

/// SPWAR, keyed once loaded.
#[derive(Component)]
struct Frame(Handle<Image>);

#[derive(Component, Clone)]
struct ResultPart;

fn enter(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    start: Option<Res<BattleStart>>,
    game: Option<Res<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(start), Some(game), Some(data)) = (start, game, data.get(&handle.0)) else {
        commands.trigger(GoTo(GameScreen::GalacticMap));
        return;
    };
    let Some(battle) = SpaceBattle::new(&game.0, &data.exe, start.place, &mut random) else {
        commands.trigger(GoTo(GameScreen::GalacticMap));
        return;
    };
    let scoped = DespawnOnExit(GameScreen::SpaceBattle);
    let image = images.add(rgba_image(u32::from(WIDTH), u32::from(HEIGHT), vec![0; usize::from(WIDTH) * usize::from(HEIGHT) * 4]));
    commands.spawn((
        Sprite::from_image(image.clone()),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.2),
        scoped.clone(),
    ));
    commands.spawn((
        Frame(asset_server.load("WAR/SPWAR.PIC")),
        Sprite::default(),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 0.5),
        Visibility::Hidden,
        scoped.clone(),
    ));
    let fighter = game.0.word(0x95ba).unwrap_or(0).min(3);
    commands.spawn((
        Sprite::from_image(asset_server.load(format!("WAR/SPFACE{fighter}.PIC#{MASKED}"))),
        Anchor::TOP_LEFT,
        place(Vec2::new(42.0, 144.0), 0.6),
        scoped,
    ));
    commands.insert_resource(Fight {
        battle,
        start: *start,
        frames: 0.0,
        retreated: false,
        result: None,
        image,
        stars: asset_server.load("WAR/SPHATTER.PIC"),
        sprites: asset_server.load("WAR/SPANTS.PIC"),
        changed: true,
    });
    commands.remove_resource::<BattleStart>();
}

/// Makes SPWAR's round screen see-through once the picture is loaded.
fn key_screen(mut frames: Query<(&Frame, &mut Sprite, &mut Visibility)>, mut images: ResMut<Assets<Image>>) {
    for (frame, mut sprite, mut visibility) in &mut frames {
        if *visibility != Visibility::Hidden {
            continue;
        }
        let Some(source) = images.get(&frame.0) else { continue };
        let Some(mut keyed) = source.data.clone() else { continue };
        for px in keyed.chunks_exact_mut(4) {
            if px[..3] == SCREEN_RED {
                px[3] = 0;
            }
        }
        let size = source.texture_descriptor.size;
        sprite.image = images.add(rgba_image(size.width, size.height, keyed));
        *visibility = Visibility::Inherited;
    }
}

/// Runs the battle at the original's frame rate; when it's over, the
/// survivors go home and the result shows.
fn fight(
    time: Res<Time>,
    fight: Option<ResMut<Fight>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    asset_server: Res<AssetServer>,
    mut set: ResMut<IconSetOverride>,
    mut commands: Commands,
) {
    let (Some(mut fight), Some(mut game), Some(data)) = (fight, game, data.get(&handle.0)) else {
        return;
    };
    if fight.result.is_some() {
        return;
    }
    fight.frames += time.delta_secs_f64() * VGA_REFRESH_HZ;
    while fight.frames >= 1.0 && !fight.battle.over {
        fight.frames -= 1.0;
        let cheat = game.0.byte(0x91e8).unwrap_or(0) != 0;
        if fight.battle.frame(cheat, &mut random) {
            fight.changed = true;
        }
    }
    if !fight.battle.over && !fight.retreated {
        return;
    }
    let losses = fight.battle.finish(&mut game.0, &data.exe);
    fight.result = Some(losses);
    set.0 = Some(BATTLE_OVER_SET);
    // FUN_20d4_1de8: the result, over the battle.
    let won = fight.battle.won() && !fight.retreated;
    let scoped = (ResultPart, DespawnOnExit(GameScreen::SpaceBattle));
    let name = if won { "SPVICT" } else { "SPLOST" };
    commands.spawn((
        Sprite::from_image(asset_server.load(format!("WAR/{name}.PIC"))),
        Anchor::TOP_LEFT,
        place(Vec2::new(0.0, CONTENT_Y), 1.0),
        scoped.clone(),
    ));
    for (side, top) in [(0, 0x4f), (1, 0x86)] {
        for k in 1..=4 {
            let text = format!("{:>4}", losses[side][k - 1]);
            let at = Vec2::new(87.0, (top + 9 * k) as f32);
            commands.spawn((label(Label::new(text, 4, YELLOW_TEXT), at), scoped.clone())).insert(place(at, 1.1));
        }
    }
}

/// Draws the stars and the ships into the battle area.
fn draw(fight: Option<ResMut<Fight>>, mut images: ResMut<Assets<Image>>) {
    let Some(mut fight) = fight else { return };
    if !fight.changed {
        return;
    }
    let (Some(stars), Some(sprites)) = (
        images.get(&fight.stars).and_then(|i| i.data.clone()),
        images.get(&fight.sprites).and_then(|i| i.data.clone()),
    ) else {
        return;
    };
    fight.changed = false;
    let (w, h) = (usize::from(WIDTH), usize::from(HEIGHT));
    let mut pixels = stars;
    pixels.resize(w * h * 4, 0);
    // An 8x8 cell of SPANTS, black left out, with its corner at (x, y).
    let mut blit = |cell: (usize, usize), x: i32, y: i32| {
        for dy in 0..8 {
            for dx in 0..8 {
                let (sx, sy) = (cell.0 * 8 + dx, cell.1 * 8 + dy);
                let s = (sy * 80 + sx) * 4;
                let Some(px) = sprites.get(s..s + 4) else { continue };
                if px[..3] == [0, 0, 0] || sx % 8 == 0 || sy % 8 == 0 {
                    continue;
                }
                let (tx, ty) = (x + dx as i32, y + dy as i32);
                if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
                    continue;
                }
                let t = (ty as usize * w + tx as usize) * 4;
                pixels[t..t + 4].copy_from_slice(px);
            }
        }
    };
    for (side, ships) in fight.battle.sides.iter().enumerate() {
        let flying = fight.battle.flying[side];
        for (i, ship) in ships.iter().enumerate() {
            let kind = usize::from(ship.kind.clamp(1, 4));
            let (x, y) = (i32::from(ship.x), i32::from(ship.y));
            if i < flying {
                blit((kind - 1, side), x - 1, y - 1);
            } else if (2..=EXPLOSION_FRAMES + 1).contains(&ship.step) {
                blit((usize::from(ship.step) - 2, kind + 1), x, y - 1);
            }
        }
    }
    if let Some(mut image) = images.get_mut(&fight.image) {
        image.data = Some(pixels);
    }
}

fn act(
    action: On<ActionUsed>,
    screen: Res<State<GameScreen>>,
    fight: Option<ResMut<Fight>>,
    game: Option<ResMut<Game>>,
    handle: Res<GameDataHandle>,
    data: Res<Assets<GameData>>,
    mut commands: Commands,
) {
    if *screen.get() != GameScreen::SpaceBattle {
        return;
    }
    let (Some(mut fight), Some(mut game), Some(data)) = (fight, game, data.get(&handle.0)) else {
        return;
    };
    match action.0 {
        RETREAT if fight.result.is_none() => fight.retreated = true,
        END_BATTLE if fight.result.is_some() => {
            let won = fight.battle.won() && !fight.retreated;
            let place = fight.start.place;
            game.0.after_battle(&data.exe, place, won, &mut random);
            game.select(place.0.into(), place.1.into(), place.2.into());
            // The first victory: the developers start on new designs.
            if won && game.0.byte(0x5d67) == Some(0) {
                game.0.set_byte(0x5d67, 1);
                game.0.start_invention_timer(13, 20, 20, &mut random);
                game.0.start_invention_timer(17, 50, 90, &mut random);
            }
            // The fight for the ground (screens 19 and 32) comes after a
            // won attack or a lost defence against an invasion; it isn't
            // there yet, so the game goes on at the map.
            let ground_next = fight.start.ground && (won == fight.start.you_attack);
            if ground_next {
                info!("ground battle at {place:?} skipped");
            }
            commands.trigger(GoTo(GameScreen::GalacticMap));
        }
        _ => {}
    }
}
