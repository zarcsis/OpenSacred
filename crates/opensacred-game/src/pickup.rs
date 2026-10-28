//! The hero picks up the items on the ground (#911; docs/re/items.md, "Picking up"). A press of
//! the left button that goes to no attack, on an item lying under the cursor, sends the hero to
//! the item's point and 25 along both axes, as the AI's state 1 walks a creature to an item
//! (`0x53A170`); standing, within a hero's reach of 100 and with a straight line of walkable
//! cells to it (`0x4FCC80`), the hero turns to it and plays its pick-up, taking it once half the
//! clip has played (#1219), else it gives up. Gold goes into the hero's purse (`0x57FD80`), every
//! other item into its pack (docs/decisions/2026-10-26-pickup.md).

use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use opensacred_data::heroes::Carried;
use opensacred_rules::drops::{Dropped, Kind};
use opensacred_rules::inventory::{Grid, Put};

use crate::attack::{self, Aim};
use crate::doors::Opens;
use crate::drops::Lying;
use crate::fight;
use crate::frame::{Control, Step, Unpaused};
use crate::game::AppState;
use crate::hero::{self, Body};
use crate::hud::Pointer;
use crate::npcs::QuestItem;
use crate::paths;
use crate::sfx;
use crate::terrain::Heights;

/// How far along both axes from the item's point the hero walks to (`+0x158`, `+0x15C`).
const BESIDE: f32 = 25.0;
/// A hero's reach of an item it takes (`0x53A170`: 100 for an id up to 16).
const REACH: f32 = 100.0;
/// The sounds of the items taken (docs/re/sound.md, "Items, gold and levels"): gold
/// (`GOLD01`), a weapon into the hand (`UI_INV_PUTMETAL01`), an item into the pack
/// (`PICKUP01`) and a quest's item (`UI_QUESTBOOK_WRITING`).
const GOLD: u32 = 7100;
const PUT_METAL: u32 = 7005;
pub const INTO_THE_PACK: u32 = 7103;
pub const QUESTS_ITEM: u32 = 7117;

/// The item that the hero was sent to by a click, which it takes as it stands near.
#[derive(Component)]
pub struct Picks(Entity);

/// The hero playing its pick-up of `item` (the motion 172, `0x53A170`'s sub-state 3): the clip's
/// node and length, and whether it took the item, which it does at the clip's half
/// (`cCreature::pickupItem`, `0x558080`); the order ends with the clip (#1219).
#[derive(Component)]
pub struct Picking {
    item: Entity,
    node: AnimationNodeIndex,
    length: f32,
    taken: bool,
}

/// The pick-up's clip's rate over the attack speed's, which the other motions of the standing
/// action's order play at alone (`0x544BD2`, #1255).
const RATE: f32 = 1.8;
/// The blend into the pick-up's clip, over its rate, and the longest blend (`0x593320`'s 0.25,
/// seen in the original, #1255; **hypothesis**: seconds, as for the arts).
const BLEND: f32 = 0.25;
const LONGEST_BLEND: f32 = 0.2;
/// The longest clip that the pick-up waits for: a clip of times longer, or negative, takes the
/// item at once (`0x558080`).
const SANE: f32 = 10.0;

/// What the hero carries: its gold (the stats' word `+0x46`), and the items it picked up, in the
/// order taken.
#[derive(Resource, Default, Debug)]
pub struct Pack {
    pub gold: u32,
    /// The items in the backpack, each by its key in the grid, in the order taken, and the grid
    /// (#1268), and the next key.
    pub items: Vec<(u32, Dropped)>,
    pub grid: Grid,
    next: u32,
    /// The main quest's items, which the scripts' collecting keeps (`SetOnCollect`, #1021), the
    /// quests' items that the scripts laid and the hero took, and those they gave it (#1256).
    pub quest: Vec<Dropped>,
    /// The item on the cursor (`cMouse +0x68`, #1269).
    pub held: Option<Held>,
}

/// An item on the cursor: its key in the grid, the item and its size in cells.
#[derive(Clone, Copy, Debug)]
pub struct Held {
    pub key: u32,
    pub dropped: Dropped,
    pub size: (u8, u8),
}

/// An item that the hero took into its pack, its type, which the scripts' collecting counts
/// (`cCreature::pickupItem` at `0x558175`, `scripts`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Taken(pub u32);

/// A quest's item that the scripts laid in the world and the hero took (`npcs::QuestItem`,
/// #1256): its type, the function its taking runs, which `scripts` runs, and its placement, which
/// `npcs` places no more.
#[derive(Message, Clone, Debug)]
pub struct TakenForQuest {
    pub item: u32,
    pub take: Option<String>,
    pub index: usize,
    /// Its script name, by whose key it enters the hero's list of the main quest's items
    /// (#1257).
    pub name: Option<String>,
}

/// An item that the scripts give the hero straight among the main quest's items, its type
/// (`CreateObj` of `0x61` or `0x62` without `0x8E`, `0x4868F6`; #1256).
#[derive(Message, Clone, Copy, Debug)]
pub struct Given(pub u32);

impl Pack {
    /// Takes `dropped`: gold into the purse, held below `0x7FFFFFFF` (`0x57FD80`), any other item
    /// of `size` into the backpack's grid at its first fit by columns (`0x5DABA0`): whether it
    /// did, none without room (`0x5592B0`).
    pub fn take(&mut self, dropped: Dropped, size: (u8, u8)) -> bool {
        if dropped.kind == Kind::Gold {
            self.gold = self.gold.saturating_add(dropped.amount).min(0x7FFF_FFFE);
            return true;
        }
        let key = self.next;
        if self.grid.take(key, size).is_none() {
            return false;
        }
        self.next += 1;
        self.items.push((key, dropped));
        true
    }

    /// Takes `dropped` of `size` onto the cursor where it holds nothing, as an item picked up
    /// while the items' page shows goes (`0x5D9060`, `0x655580`): whether it did.
    pub fn hold(&mut self, dropped: Dropped, size: (u8, u8)) -> bool {
        if self.held.is_some() {
            return false;
        }
        self.held = Some(Held {
            key: self.next,
            dropped,
            size,
        });
        self.next += 1;
        true
    }

    /// Takes the item over the cell `cell` out of the grid onto the cursor where it holds nothing
    /// (`0x5DA3D0`): whether one went.
    pub fn lift(&mut self, cell: (u8, u8)) -> bool {
        if self.held.is_some() {
            return false;
        }
        let Some(placed) = self.grid.at(cell).copied() else {
            return false;
        };
        let Some(index) = self.items.iter().position(|(key, _)| *key == placed.key) else {
            return false;
        };
        self.grid.remove(placed.key);
        let (key, dropped) = self.items.remove(index);
        self.held = Some(Held {
            key,
            dropped,
            size: placed.size,
        });
        true
    }

    /// Puts the item on the cursor at the cell `at` of the grid (`0x6C0150`): where it fits, the
    /// cursor emptied; over exactly one item, that one onto the cursor in its place; else
    /// nothing.
    pub fn put_held(&mut self, at: (i32, i32)) -> Put {
        let Some(held) = self.held else {
            return Put::Refused;
        };
        // The sizes of the items in the grid, before the put takes one out.
        let sizes: Vec<(u32, (u8, u8))> = self
            .grid
            .items
            .iter()
            .map(|placed| (placed.key, placed.size))
            .collect();
        let put = self.grid.put(held.key, at, held.size);
        match put {
            Put::Placed => {
                self.items.push((held.key, held.dropped));
                self.held = None;
            }
            Put::Swapped(other) => {
                self.items.push((held.key, held.dropped));
                self.held = None;
                if let Some(index) = self.items.iter().position(|(key, _)| *key == other) {
                    let (key, dropped) = self.items.remove(index);
                    let size = sizes
                        .iter()
                        .find(|(key, _)| *key == other)
                        .map_or((1, 1), |&(_, size)| size);
                    self.held = Some(Held { key, dropped, size });
                }
            }
            Put::Refused => {}
        }
        put
    }

    /// Puts `dropped` of `size` into the grid at the cell `at` where it fits: whether it did.
    pub fn put(&mut self, dropped: Dropped, at: (u8, u8), size: (u8, u8)) -> bool {
        let key = self.next;
        if !self.grid.place(key, at, size) {
            return false;
        }
        self.next += 1;
        self.items.push((key, dropped));
        true
    }

    /// Moves the last item of the type `item` of the pack among the main quest's items, out of
    /// the grid.
    pub fn keep_for_quest(&mut self, item: u32) {
        if let Some(at) = self
            .items
            .iter()
            .rposition(|(_, dropped)| dropped.item == item)
        {
            let (key, kept) = self.items.remove(at);
            self.grid.remove(key);
            self.quest.push(kept);
        }
    }
}

/// On a press of the left button that went to no attack, the item lying nearest the cursor's
/// line of sight, if one is near it, becomes the hero's to take, before a door (the pick's
/// classes, `0x626C50`: the items 2, the doors 3), and the hero walks to it; any press ends the
/// last such order.
pub(crate) fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<Pointer>,
    aim: Res<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    items: Query<(Entity, &GlobalTransform, &Lying)>,
    mut heroes: Query<(Entity, &mut Body)>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    // Any press ends the order, a pick-up under way with it: its clip given up, the item left
    // lying where the clip had not reached its half.
    for (hero, _) in &heroes {
        commands.entity(hero).remove::<(Picks, Picking)>();
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let (false, Ok(ray)) = (aim.0, camera.viewport_to_world(view, cursor)) else {
        return;
    };
    let Some((item, at)) = items
        .iter()
        .filter(|(_, _, lying)| lying.resting)
        .map(|(item, at, _)| {
            let at = at.translation();
            (item, at, attack::from_line(ray, at))
        })
        .filter(|&(_, _, distance)| distance <= attack::PICKED_WITHIN)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(item, at, _)| (item, at))
    else {
        return;
    };
    for (hero, mut body) in &mut heroes {
        debug!("pickup: the hero goes to take {item}");
        body.send(at + Vec3::new(BESIDE, 0.0, BESIDE));
        commands.entity(hero).remove::<Opens>().insert(Picks(item));
    }
}

/// Sends the hero to take the item lying nearest it (the command `take`), as a click on it does:
/// a line of which item.
pub fn take_nearest(world: &mut World) -> Result<String, String> {
    let mut heroes = world.query_filtered::<(Entity, &Transform), With<Body>>();
    let (hero, here) = heroes
        .iter(world)
        .next()
        .map(|(hero, transform)| (hero, transform.translation))
        .ok_or("the hero is not in the world yet")?;
    let mut items = world.query::<(Entity, &GlobalTransform, &Lying)>();
    let (item, at, lying) = items
        .iter(world)
        .filter(|(_, _, lying)| lying.resting)
        .map(|(item, at, lying)| (item, at.translation(), *lying))
        .min_by(|a, b| {
            let d = |at: Vec3| (at - here).xz().length_squared();
            d(a.1).total_cmp(&d(b.1))
        })
        .ok_or("no item lies on the ground")?;
    let mut body = world
        .get_mut::<Body>(hero)
        .ok_or("the hero is not in the world yet")?;
    body.send(at + Vec3::new(BESIDE, 0.0, BESIDE));
    world.entity_mut(hero).remove::<Opens>().insert(Picks(item));
    Ok(format!(
        "the hero goes to take {:?} {} x{}, {:.0} away",
        lying.dropped.kind,
        lying.dropped.item,
        lying.dropped.amount,
        (at - here).xz().length()
    ))
}

/// What takes an item off the ground into the hero's purse, pack or hand.
#[derive(SystemParam)]
pub(crate) struct Taking<'w, 's> {
    commands: Commands<'w, 's>,
    pack: ResMut<'w, Pack>,
    taken: MessageWriter<'w, Taken>,
    sounds: MessageWriter<'w, sfx::Play>,
    arsenal: Option<Res<'w, crate::arsenal::Arsenal>>,
    throws: MessageWriter<'w, crate::arsenal::Throws>,
    items: Query<'w, 's, (&'static Lying, Option<&'static QuestItem>)>,
    for_quest: MessageWriter<'w, TakenForQuest>,
    tables: Option<Res<'w, crate::drops::Tables>>,
    shown: Option<Res<'w, crate::inventory::Shown>>,
}

impl Taking<'_, '_> {
    /// Takes `item` (`cCreature::pickupItem`, `0x558080`), if it still lies there: gold sounds
    /// `GOLD01` and the weapon taken back into the hero's hand `UI_INV_PUTMETAL01` (the events
    /// `(9, 0x83)` and `(9, 5)`; #1167), and the scripts' collecting sounds any other item
    /// (`scripts`). A quest's item that the scripts laid goes among the main quest's items with
    /// `UI_QUESTBOOK_WRITING` (the event `(9, 0x81)`), and the scripts run its taking's function
    /// (`Take:`, `0x4A71F0`; #1256).
    fn take(&mut self, item: Entity) {
        let Ok((lying, quest)) = self.items.get(item) else {
            return;
        };
        info!(
            "pickup: the hero takes {:?} {} x{}",
            lying.dropped.kind, lying.dropped.item, lying.dropped.amount
        );
        if let Some(quest) = quest {
            self.for_quest.write(TakenForQuest {
                item: lying.dropped.item,
                take: quest.take.clone(),
                index: quest.index,
                name: quest.name.clone(),
            });
            self.sounds.write(sfx::Play {
                sound: sfx::Of::Id(QUESTS_ITEM),
                at: None,
                rate: (0.4, sfx::Roll::Share(0.2)),
            });
            self.pack.quest.push(lying.dropped);
        // The weapon the hero hurled goes back into its empty hand (`0x54ACC0`, #755).
        } else if self
            .arsenal
            .as_deref()
            .is_some_and(|arsenal| arsenal.takes_back(&lying.dropped))
        {
            self.throws.write(crate::arsenal::Throws(false));
            self.sounds.write(sfx::Play {
                sound: sfx::Of::Id(PUT_METAL),
                at: None,
                rate: (0.45, sfx::Roll::Share(0.1)),
            });
        } else {
            let size = self
                .tables
                .as_deref()
                .map_or((1, 1), |tables| tables.size(lying.dropped.item));
            // While the items' page shows, onto the empty cursor (#1269).
            let onto_the_cursor = lying.dropped.kind != Kind::Gold
                && self.shown.as_deref().is_some_and(|shown| shown.0)
                && self.pack.hold(lying.dropped, size);
            // Without room it stays where it lies (the jump of `0x5592B0` not followed).
            if !onto_the_cursor && !self.pack.take(lying.dropped, size) {
                info!(
                    "pickup: no room in the backpack for {} of {size:?}",
                    lying.dropped.item
                );
                return;
            }
            if lying.dropped.kind == Kind::Gold {
                self.sounds.write(sfx::Play {
                    sound: sfx::Of::Id(GOLD),
                    at: None,
                    rate: (0.3, sfx::Roll::Share(0.4)),
                });
            } else {
                self.taken.write(Taken(lying.dropped.item));
            }
        }
        self.commands.entity(item).despawn();
    }
}

/// The rate of the pick-up's clip of a hero of the attack speed `attack_speed` (`0x584FE0`, the
/// Dwarf's frenzy in it): its hundredth, times [`RATE`] (`0x544BCC`; the creature's `+0x2C9`,
/// 1.0 but for the effect 15, not followed). The Seraphim's 154 plays it at 2.772 (seen, #1255).
fn rate(attack_speed: f32) -> f32 {
    attack_speed / 100.0 * RATE
}

/// The blend into the pick-up's clip at the rate `rate`: [`BLEND`] over it, at most
/// [`LONGEST_BLEND`].
fn blend(rate: f32) -> Duration {
    Duration::from_secs_f32((BLEND / rate.max(f32::EPSILON)).min(LONGEST_BLEND))
}

/// The hero that stands, once its way to an item it was sent to ends: within its reach of the
/// item, with a straight line of walkable cells to it, it turns to it and plays its pick-up
/// (the motion 172, `MOTION_PICKUP`) at the game's speed times its [`rate`] ([`picking`]), or
/// takes it at once where its model has none or the clip's time is not sane; else it gives up.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub(crate) fn take(
    mut taking: Taking,
    heights: Res<Heights>,
    (clips, speed, animations): (
        Option<Res<hero::Clips>>,
        Res<hero::GameSpeed>,
        Res<Assets<AnimationClip>>,
    ),
    places: Query<&GlobalTransform, With<Lying>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut heroes: Query<(
        Entity,
        &mut Transform,
        &Body,
        &Picks,
        Option<&fight::Combat>,
        Option<&fight::Frenzy>,
    )>,
) {
    for (hero, mut transform, body, picks, combat, frenzy) in &mut heroes {
        // Its order to the item taken at a tick of the world first (`hero::take_orders`).
        if body.target().is_some() || body.ordered() {
            continue;
        }
        taking.commands.entity(hero).remove::<Picks>();
        let Ok(at) = places.get(picks.0) else {
            continue;
        };
        let (here, at) = (transform.translation, at.translation());
        let level = body.level();
        if !takes(here, at)
            || !paths::clear(here.xz(), at.xz(), |cell| heights.walkable_on(cell, level))
        {
            debug!("pickup: the hero cannot reach {}", picks.0);
            continue;
        }
        // It turns to the item and plays its pick-up, taking the item at the clip's half
        // (`0x44DD90`, `0x53A170`'s sub-state 3), at its attack speed, quickened by the
        // Dwarf's frenzy (`0x584FE0`).
        transform.rotation = hero::facing((at - here).with_y(0.0));
        let rate = rate(combat.map_or(100.0, |combat| {
            opensacred_rules::arts::quickened(combat.speed, frenzy.map_or(0.0, |frenzy| frenzy.0))
        }));
        let clip = clips
            .as_deref()
            .and_then(|clips| Some((clips.pickup.clone()?, clips.player?)));
        if let Some(((node, clip), player)) = clip
            && let Ok((mut player, mut transitions)) = players.get_mut(player)
        {
            let length = animations.get(&clip).map_or(0.0, AnimationClip::duration);
            if length > 0.0 && length <= SANE {
                transitions
                    .play(&mut player, node, blend(rate))
                    .set_speed(speed.0 * rate);
                taking.commands.entity(hero).insert(Picking {
                    item: picks.0,
                    node,
                    length,
                    taken: false,
                });
                continue;
            }
        }
        taking.take(picks.0);
    }
}

/// Plays each hero's pick-up on: it takes the item once half the clip has played
/// (`cCreature::pickupItem`, `0x558080`), and its order ends as the clip ends, or as another
/// clip takes the pick-up's place (a strike, a fall).
pub(crate) fn picking(
    mut taking: Taking,
    clips: Option<Res<hero::Clips>>,
    players: Query<(&AnimationPlayer, &AnimationTransitions)>,
    mut heroes: Query<(Entity, &mut Picking)>,
) {
    for (hero, mut picking) in &mut heroes {
        let active = clips
            .as_deref()
            .and_then(|clips| clips.player)
            .and_then(|player| players.get(player).ok())
            .filter(|(_, transitions)| transitions.get_main_animation() == Some(picking.node))
            .and_then(|(player, _)| player.animation(picking.node));
        let Some(active) = active else {
            taking.commands.entity(hero).remove::<Picking>();
            continue;
        };
        if !picking.taken && active.seek_time() >= picking.length / 2.0 {
            picking.taken = true;
            taking.take(picking.item);
        }
        if active.is_finished() {
            taking.commands.entity(hero).remove::<Picking>();
        }
    }
}

/// Whether a hero standing at `hero` takes the item at `item`: within [`REACH`] of it on the
/// ground, the integer root rounded up (`0x53A170`).
fn takes(hero: Vec3, item: Vec3) -> bool {
    (item - hero).xz().length().ceil() <= REACH
}

/// The items that the hero's template carries in its backpack at the start (`heroes.ron`,
/// #1285), which `game` reads with the template.
#[derive(Resource, Default, Clone, Debug)]
pub struct Starting(pub Vec<Carried>);

/// The hero's pack at each game's start: no gold, and its template's items in their cells, each
/// of its type's kind, as the scripts give an item (`receive`).
fn start(
    mut commands: Commands,
    starting: Option<Res<Starting>>,
    tables: Option<Res<crate::drops::Tables>>,
) {
    let mut pack = Pack::default();
    for carried in starting.iter().flat_map(|starting| &starting.0) {
        let kind = tables
            .as_deref()
            .and_then(|tables| tables.catalogue().kind(carried.item))
            .unwrap_or(Kind::Other);
        let dropped = Dropped {
            kind,
            item: carried.item,
            level: 0,
            key: 16,
            quality: 0,
            amount: 1,
            values: None,
        };
        if !pack.put(dropped, carried.at, carried.size) {
            warn!(
                "pickup: the template's item {} does not fit at {:?}",
                carried.item, carried.at
            );
        }
    }
    commands.insert_resource(pack);
}

/// Keeps each item that the scripts give the hero among the main quest's items (#1256).
fn receive(mut given: MessageReader<Given>, mut pack: ResMut<Pack>) {
    for &Given(item) in given.read() {
        info!("pickup: the scripts give the hero {item} among the quest's items");
        pack.quest.push(Dropped {
            kind: Kind::Other,
            item,
            level: 0,
            key: 16,
            quality: 0,
            amount: 1,
            values: None,
        });
    }
}

/// The items that the hero picks up.
pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pack>()
            .add_message::<Taken>()
            .add_message::<TakenForQuest>()
            .add_message::<Given>()
            .add_systems(OnEnter(AppState::InGame), start)
            .add_systems(Update, receive.run_if(in_state(AppState::InGame)))
            .add_systems(
                Update,
                (
                    pick.in_set(Control)
                        .after(attack::aim)
                        .after(hero::steer)
                        .after(crate::doors::pick),
                    take.after(hero::run),
                    picking.after(take),
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensacred_rules::drops::Dropped;

    fn dropped(kind: Kind, item: u32, amount: u32) -> Dropped {
        Dropped {
            kind,
            item,
            level: 1,
            key: 16,
            quality: 0,
            amount,
            values: None,
        }
    }

    #[test]
    fn the_hero_takes_an_item_within_its_reach() {
        let item = Vec3::new(1000.0, 0.0, 1000.0);
        assert!(takes(item + Vec3::new(BESIDE, 30.0, BESIDE), item));
        assert!(takes(item + Vec3::new(100.0, 0.0, 0.0), item));
        assert!(takes(item + Vec3::new(70.0, 0.0, 71.0), item));
        assert!(!takes(item + Vec3::new(71.0, 0.0, 71.0), item));
    }

    #[test]
    fn the_pick_up_plays_at_the_attack_speed_times_its_own_rate() {
        // The Seraphim's 154, as the original played it (#1255): 2.772, blended in over 0.09 s.
        assert!((rate(154.0) - 2.772).abs() < 1e-5);
        assert_eq!(blend(rate(154.0)).as_millis(), 90);
        // A slow hero's blend held to 0.2 s.
        assert!((rate(100.0) - 1.8).abs() < 1e-6);
        assert_eq!(blend(1.0).as_millis(), 200);
    }

    #[test]
    fn gold_goes_into_the_purse_and_the_rest_into_the_pack() {
        let mut pack = Pack::default();
        assert!(pack.take(dropped(Kind::Gold, 5133, 37), (1, 1)));
        assert!(pack.take(dropped(Kind::Weapon, 1712, 1), (1, 3)));
        assert!(pack.take(dropped(Kind::Gold, 5132, 5), (1, 1)));
        assert!(pack.take(dropped(Kind::Other, 6005, 1), (1, 1)));
        assert_eq!(pack.gold, 42);
        let items: Vec<u32> = pack.items.iter().map(|(_, d)| d.item).collect();
        assert_eq!(items, [1712, 6005]);
        // The sword down the first column, the stone under it (#1268).
        let places: Vec<(u8, u8)> = pack.grid.items.iter().map(|item| item.at).collect();
        assert_eq!(places, [(0, 0), (0, 3)]);
        pack.keep_for_quest(1712);
        assert_eq!(pack.grid.items.len(), 1);
        assert_eq!(pack.quest.len(), 1);
        pack.gold = 0x7FFF_FFF0;
        assert!(pack.take(dropped(Kind::Gold, 5135, 1000), (1, 1)));
        assert_eq!(pack.gold, 0x7FFF_FFFE);
        // A full grid takes nothing more.
        let mut full = Pack::default();
        for _ in 0..160 {
            assert!(full.take(dropped(Kind::Other, 5171, 1), (1, 1)));
        }
        assert!(!full.take(dropped(Kind::Other, 5171, 1), (1, 1)));
    }

    #[test]
    fn an_item_goes_onto_the_cursor_and_back_into_the_grid_or_in_place_of_another() {
        let mut pack = Pack::default();
        assert!(pack.put(dropped(Kind::Potion, 5171, 1), (0, 0), (1, 1)));
        assert!(pack.put(dropped(Kind::Weapon, 1712, 1), (2, 0), (1, 3)));
        // Nothing under the cell: nothing taken.
        assert!(!pack.lift((5, 5)));
        assert!(pack.lift((2, 1)));
        assert_eq!(
            pack.held.map(|held| (held.dropped.item, held.size)),
            Some((1712, (1, 3)))
        );
        assert_eq!(pack.items.len(), 1);
        // A second item does not go onto the full cursor.
        assert!(!pack.lift((0, 0)));
        assert!(!pack.hold(dropped(Kind::Other, 6005, 1), (1, 1)));
        // Out of the grid: refused; over the potion: swapped.
        assert_eq!(pack.put_held((19, 6)), Put::Refused);
        assert!(matches!(pack.put_held((0, 0)), Put::Swapped(_)));
        assert_eq!(
            pack.held.map(|held| (held.dropped.item, held.size)),
            Some((5171, (1, 1)))
        );
        assert_eq!(pack.grid.at((0, 2)).map(|placed| placed.size), Some((1, 3)));
        assert_eq!(pack.put_held((5, 5)), Put::Placed);
        assert!(pack.held.is_none());
        assert_eq!(pack.items.len(), 2);
        assert!(pack.hold(dropped(Kind::Other, 6005, 1), (1, 1)));
    }

    #[test]
    fn the_templates_items_lie_in_their_cells_and_a_pick_up_goes_below_them() {
        let mut pack = Pack::default();
        for row in 0..3 {
            assert!(pack.put(dropped(Kind::Potion, 5171, 1), (0, row), (1, 1)));
        }
        assert!(pack.put(dropped(Kind::Other, 5633, 1), (0, 3), (1, 2)));
        assert!(!pack.put(dropped(Kind::Other, 5633, 1), (0, 4), (1, 1)));
        assert!(pack.take(dropped(Kind::Potion, 5171, 1), (1, 1)));
        let places: Vec<(u8, u8)> = pack.grid.items.iter().map(|item| item.at).collect();
        assert_eq!(places, [(0, 0), (0, 1), (0, 2), (0, 3), (0, 5)]);
        assert_eq!(pack.items.len(), 5);
    }
}
