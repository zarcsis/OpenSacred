//! The hero picks up the items on the ground (#911; docs/re/items.md, "Picking up"). A press of
//! the left button that goes to no attack, on an item lying under the cursor, sends the hero to
//! the item's point and 25 along both axes, as the AI's state 1 walks a creature to an item
//! (`0x53A170`); standing, within a hero's reach of 100 and with a straight line of walkable
//! cells to it (`0x4FCC80`), the hero turns to it and plays its pick-up, taking it once half the
//! clip has played (#1219), else it gives up. Gold goes into the hero's purse (`0x57FD80`), every
//! other item into its pack, not yet into the hands or the slots (#1088).
//!
//! The bar's Collect button and the key A send the hero collecting (the creature event `0x113`,
//! the AI's state 12 without a target, `0x4EC560`; #1408, #1591): it walks to the first item about it that it may carry,
//! by square rings of cells out to 19, plays its pick-up there and takes every item on the 3 × 3
//! cells about it at the clip's half, and looks again as the clip ends, until nothing is left.

use std::collections::HashMap;
use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use opensacred_data::heroes::Carried;
use opensacred_rules::drops::{Dropped, Kind};
use opensacred_rules::inventory::{Grid, Placed, Put};

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

/// The rings of cells about the hero's cell that the collecting searches, 0 to 19 (`0x4EC560`).
const COLLECTS_WITHIN: i32 = 19;
/// How near an item's point the collecting hero stops (`+0x176`).
const COLLECT_STOP: f32 = 20.0;
/// How long before its pick-up's clip ends the collecting hero looks again, in seconds of the
/// game (`0x4EC560`'s sub-state 3; **hypothesis**: of the clip's time at its rate).
const LOOKS_AGAIN: f32 = 0.2;
/// How far an item may lie above or below the height that the hero's level gives at its point and
/// still lie on the hero's layer, which the collecting searches (**estimate**: well within a
/// floor's height; the items keep no layer of their own).
const SAME_LAYER: f32 = 20.0;

/// The hero collecting the items about it (the AI's state 12 without a target, `0x4EC560`), as the
/// bar's Collect button or the command `collect` sends it ([`collect`]).
#[derive(Component, Clone, Copy, Debug)]
pub struct Collects(Collecting);

impl Collects {
    /// Whether it plays its pick-up, whose clip the hero's walk leaves alone (`hero::run`).
    pub fn picks_up(&self) -> bool {
        matches!(self.0, Collecting::Picking { .. })
    }
}

/// Where the collecting stands.
#[derive(Clone, Copy, Debug)]
enum Collecting {
    /// It looks for the next item (the sub-state 0).
    Seeking,
    /// It walks to the item.
    Walking(Entity),
    /// It plays its pick-up (the sub-state 3): the clip's node, its length and rate, and whether
    /// it took the items about it at the clip's half, once a clip (`+0x134`).
    Picking {
        node: AnimationNodeIndex,
        length: f32,
        rate: f32,
        taken: bool,
    },
}

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
    /// The items worn in the equipment window's slots, each by its key (#1289); the classes'
    /// default items, which the window does not show, not among them.
    pub worn: [Option<(u32, Dropped)>; opensacred_rules::equipment::SLOTS as usize],
    /// The types of the books read, each once, in the order read (the inventory entry's list at
    /// `+0xC`, `0x5DD5D0`; #1358), which the quest book's window shows (#1091).
    pub books: Vec<u32>,
    /// The `Use:` functions of the items that the scripts laid, by their keys (`CreateObj`'s
    /// `0x83`, the script manager's list at `+0x358`; #1272).
    pub uses: Vec<(u32, String)>,
    /// The items used up since `sparkles` last looked, their places in the grid and their types,
    /// each of which sparkles (`0x5D8000`, #1272).
    pub used: Vec<(Placed, u32)>,
}

/// Takes the item over the cell `cell` of a grid and its items onto the cursor `held` where it
/// holds nothing (`0x5DA3D0`): whether one went. The backpack's and a town stash's alike.
pub(crate) fn lift_from(
    (items, grid): (&mut Vec<(u32, Dropped)>, &mut Grid),
    held: &mut Option<Held>,
    cell: (u8, u8),
) -> bool {
    if held.is_some() {
        return false;
    }
    let Some(placed) = grid.at(cell).copied() else {
        return false;
    };
    let Some(index) = items.iter().position(|(key, _)| *key == placed.key) else {
        return false;
    };
    grid.remove(placed.key);
    let (key, dropped) = items.remove(index);
    *held = Some(Held {
        key,
        dropped,
        size: placed.size,
    });
    true
}

/// Puts the item on the cursor `held` at the cell `at` of a grid and its items (`0x6C0150`,
/// `0x6ACFC0`): where it fits, the cursor emptied; over exactly one item, that one onto the cursor
/// in its place; else nothing.
pub(crate) fn put_into(
    (items, grid): (&mut Vec<(u32, Dropped)>, &mut Grid),
    held: &mut Option<Held>,
    at: (i32, i32),
) -> Put {
    let Some(holding) = *held else {
        return Put::Refused;
    };
    // The sizes of the items in the grid, before the put takes one out.
    let sizes: Vec<(u32, (u8, u8))> = grid
        .items
        .iter()
        .map(|placed| (placed.key, placed.size))
        .collect();
    let put = grid.put(holding.key, at, holding.size);
    match put {
        Put::Placed => {
            items.push((holding.key, holding.dropped));
            *held = None;
        }
        Put::Swapped(other) => {
            items.push((holding.key, holding.dropped));
            *held = None;
            if let Some(index) = items.iter().position(|(key, _)| *key == other) {
                let (key, dropped) = items.remove(index);
                let size = sizes
                    .iter()
                    .find(|(key, _)| *key == other)
                    .map_or((1, 1), |&(_, size)| size);
                *held = Some(Held { key, dropped, size });
            }
        }
        Put::Refused => {}
    }
    put
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

/// An item that the scripts' `CreateObj` laid and the hero took (`npcs::ScriptItem`, #1296): its
/// placement, which `npcs` places no more.
#[derive(Message, Clone, Copy, Debug)]
pub struct TakenPlaced(pub usize);

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

    /// The potions in the grid by their keys and types, in its order row by row, as the belt
    /// looks for them (`0x5DA8C0`, #1311): by their top rows, then their left columns.
    pub fn potions(&self) -> Vec<(u32, u32)> {
        let mut placed: Vec<_> = self
            .grid
            .items
            .iter()
            .filter_map(|placed| {
                let (_, dropped) = self.items.iter().find(|(key, _)| *key == placed.key)?;
                opensacred_rules::potions::kind(dropped.item)?;
                Some(((placed.at.1, placed.at.0), (placed.key, dropped.item)))
            })
            .collect();
        placed.sort_by_key(|&(at, _)| at);
        placed.into_iter().map(|(_, potion)| potion).collect()
    }

    /// Takes the item of the key `key` out of the grid for good, as a potion drunk is used up
    /// (`0x5DEBC0`, #1311), to sparkle where it was (`0x5D8000`, #1272): the item, none where the
    /// grid holds no such key.
    pub fn use_up(&mut self, key: u32) -> Option<Dropped> {
        let (placed, dropped) = self.take_out(key)?;
        self.used.push((placed, dropped.item));
        Some(dropped)
    }

    /// Takes the item of the key `key` out of the grid for good with no sparkle, as a `Use:`
    /// function's item leaves it (`0x5DA630`, #1272): its place and the item.
    pub fn take_out(&mut self, key: u32) -> Option<(Placed, Dropped)> {
        let index = self.items.iter().position(|(k, _)| *k == key)?;
        let placed = self.grid.remove(key)?;
        self.uses.retain(|(k, _)| *k != key);
        Some((placed, self.items.remove(index).1))
    }

    /// Gives the item taken last, into the grid or onto the cursor, the `Use:` function
    /// `function` (#1272).
    pub fn bind_use(&mut self, function: String) {
        if let Some(key) = self.next.checked_sub(1) {
            self.uses.push((key, function));
        }
    }

    /// The `Use:` function of the item of the key `key`, none for an item without one.
    pub fn use_of(&self, key: u32) -> Option<&str> {
        self.uses
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, function)| function.as_str())
    }

    /// Adds the book of the type `item` to the books read where it is not among them
    /// (`0x5DD5D0`): whether it was new.
    pub fn read(&mut self, item: u32) -> bool {
        if self.books.contains(&item) {
            return false;
        }
        self.books.push(item);
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
        lift_from((&mut self.items, &mut self.grid), &mut self.held, cell)
    }

    /// Puts the item on the cursor at the cell `at` of the grid (`0x6C0150`): where it fits, the
    /// cursor emptied; over exactly one item, that one onto the cursor in its place; else
    /// nothing.
    pub fn put_held(&mut self, at: (i32, i32)) -> Put {
        put_into((&mut self.items, &mut self.grid), &mut self.held, at)
    }

    /// Takes the item worn in `slot` off onto the empty cursor, of `size` in the grid (`0x556A30`):
    /// whether one went.
    pub fn take_off(&mut self, slot: u8, size: (u8, u8)) -> bool {
        if self.held.is_some() {
            return false;
        }
        let Some((key, dropped)) = self.worn.get_mut(usize::from(slot)).and_then(Option::take)
        else {
            return false;
        };
        self.held = Some(Held { key, dropped, size });
        true
    }

    /// Puts the held item on in `slot` (`sdk_EQUIP`), the item worn there before onto the cursor,
    /// of `size` in the grid: whether it went.
    pub fn put_on(&mut self, slot: u8, size: (u8, u8)) -> bool {
        let (Some(held), Some(worn)) = (self.held, self.worn.get_mut(usize::from(slot))) else {
            return false;
        };
        let old = worn.replace((held.key, held.dropped));
        self.held = old.map(|(key, dropped)| Held { key, dropped, size });
        true
    }

    /// Takes the item worn in `slot`, of `size`, into the grid at its first fit (the right
    /// button, the UI event `0x2C` of the sub-kind 2): whether it went.
    pub fn unwear(&mut self, slot: u8, size: (u8, u8)) -> bool {
        let Some(&Some((key, dropped))) = self.worn.get(usize::from(slot)) else {
            return false;
        };
        if self.grid.take(key, size).is_none() {
            return false;
        }
        self.items.push((key, dropped));
        self.worn[usize::from(slot)] = None;
        true
    }

    /// Puts the grid's item `key` on in `slot` (the UI event `0x2C` of the sub-kind 1), the item
    /// worn there before, of `old_size`, into the cell that the new one left, else at its first
    /// fit (the sub-kind 3): whether it went; nothing moves where the old one finds no room.
    pub fn wear(&mut self, key: u32, slot: u8, old_size: (u8, u8)) -> bool {
        let Some(index) = self.items.iter().position(|(held, _)| *held == key) else {
            return false;
        };
        let Some(placed) = self.grid.remove(key) else {
            return false;
        };
        if let Some(&Some((old, _))) = self.worn.get(usize::from(slot))
            && !self.grid.place(old, placed.at, old_size)
            && self.grid.take(old, old_size).is_none()
        {
            self.grid.place(key, placed.at, placed.size);
            return false;
        }
        let (_, dropped) = self.items.remove(index);
        if let Some(old) = self.worn[usize::from(slot)].replace((key, dropped)) {
            self.items.push(old);
        }
        true
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
/// classes, `0x626C50`: the items 2, the doors 3), else with Alt held the item of the label
/// under the cursor (`0x6270A0`, `labels`), and the hero walks to it; any press ends the last
/// such order.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub(crate) fn pick(
    mut commands: Commands,
    (buttons, keys): (Res<ButtonInput<MouseButton>>, crate::controls::Keys),
    pointer: Res<Pointer>,
    aim: Res<Aim>,
    labels: Res<crate::labels::Shown>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    items: Query<(Entity, &GlobalTransform, &Lying)>,
    mut heroes: Query<(Entity, &mut Body)>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    // Any press ends the order, a pick-up under way with it: its clip given up, the item left
    // lying where the clip had not reached its half; and the collecting.
    for (hero, _) in &heroes {
        commands.entity(hero).remove::<(Picks, Picking, Collects)>();
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
        .or_else(|| {
            let item = labels
                .under(cursor)
                .filter(|_| crate::labels::held(&keys))?;
            let (item, at, _) = items.get(item).ok()?;
            Some((item, at.translation()))
        })
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
    world
        .entity_mut(hero)
        .remove::<(Opens, Collects)>()
        .insert(Picks(item));
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
    items: Query<
        'w,
        's,
        (
            &'static Lying,
            Option<&'static QuestItem>,
            Option<&'static crate::npcs::ScriptItem>,
        ),
    >,
    placed: MessageWriter<'w, TakenPlaced>,
    for_quest: MessageWriter<'w, TakenForQuest>,
    tables: Option<Res<'w, crate::drops::Tables>>,
    shown: Option<Res<'w, crate::inventory::Shown>>,
    remarks: MessageWriter<'w, crate::remarks::Remark>,
}

impl Taking<'_, '_> {
    /// Whether the hero may carry `item`, as the collecting asks before it walks to one
    /// (`0x4EC560`): gold, a quest's item that the scripts laid, the weapon it hurled, or an item
    /// for which the backpack's grid has room (`0x5DABA0`); the filter `PICKUPAUTO` of the
    /// options passes every item at its default (`0x4EC330`).
    fn carries(&self, item: Entity) -> bool {
        let Ok((lying, quest, _)) = self.items.get(item) else {
            return false;
        };
        let size = self
            .tables
            .as_deref()
            .map_or((1, 1), |tables| tables.size(lying.dropped.item));
        lying.dropped.kind == Kind::Gold
            || quest.is_some()
            || self
                .arsenal
                .as_deref()
                .is_some_and(|arsenal| arsenal.takes_back(&lying.dropped))
            || self.pack.grid.first_fit(size).is_some()
    }

    /// Takes `item` (`cCreature::pickupItem`, `0x558080`), if it still lies there: gold sounds
    /// `GOLD01` and the weapon taken back into the hero's hand `UI_INV_PUTMETAL01` (the events
    /// `(9, 0x83)` and `(9, 5)`; #1167), and the scripts' collecting sounds any other item
    /// (`scripts`). A quest's item that the scripts laid goes among the main quest's items with
    /// `UI_QUESTBOOK_WRITING` (the event `(9, 0x81)`), and the scripts run its taking's function
    /// (`Take:`, `0x4A71F0`; #1256).
    fn take(&mut self, item: Entity) {
        let Ok((lying, quest, script_item)) = self.items.get(item) else {
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
            // Without room it stays where it lies (the jump of `0x5592B0` not followed), the
            // hero saying so (the event (8, 3), #1324).
            if !onto_the_cursor && !self.pack.take(lying.dropped, size) {
                info!(
                    "pickup: no room in the backpack for {} of {size:?}",
                    lying.dropped.item
                );
                self.remarks.write(crate::remarks::Remark::CantCarry);
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
                // A `CreateObj`'s item keeps its use's function in the backpack (#1272).
                if let Some(function) = script_item.and_then(|item| item.uses.clone()) {
                    self.pack.bind_use(function);
                }
            }
        }
        // A `CreateObj`'s item taken is not laid again as its sector comes back (#1296).
        if let Some(script_item) = script_item {
            self.placed.write(TakenPlaced(script_item.index));
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
            || !paths::straight(here.xz(), at.xz(), |cell| heights.walkable_on(cell, level))
        {
            debug!("pickup: the hero cannot reach {}", picks.0);
            continue;
        }
        // It turns to the item and plays its pick-up, taking the item at the clip's half
        // (`0x44DD90`, `0x53A170`'s sub-state 3), at its attack speed, quickened by the
        // Dwarf's frenzy (`0x584FE0`).
        transform.rotation = hero::facing((at - here).with_y(0.0));
        let rate = hero_rate(combat, frenzy);
        if let Some((node, length)) =
            play_pickup(clips.as_deref(), &mut players, &animations, (speed.0, rate))
        {
            taking.commands.entity(hero).insert(Picking {
                item: picks.0,
                node,
                length,
                taken: false,
            });
            continue;
        }
        taking.take(picks.0);
    }
}

/// The rate of the hero's pick-up's clip, by its attack speed quickened by the Dwarf's frenzy
/// (`0x584FE0`).
fn hero_rate(combat: Option<&fight::Combat>, frenzy: Option<&fight::Frenzy>) -> f32 {
    rate(combat.map_or(100.0, |combat| {
        opensacred_rules::arts::quickened(combat.speed, frenzy.map_or(0.0, |frenzy| frenzy.0))
    }))
}

/// Plays the hero's pick-up (the motion 172, `MOTION_PICKUP`) at the game's speed `speed` times its
/// rate `rate`: the clip's node and length, none where its model has none or the clip's time is
/// not sane, where the item is taken at once.
fn play_pickup(
    clips: Option<&hero::Clips>,
    players: &mut Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    animations: &Assets<AnimationClip>,
    (speed, rate): (f32, f32),
) -> Option<(AnimationNodeIndex, f32)> {
    let ((node, clip), player) =
        clips.and_then(|clips| Some((clips.pickup.clone()?, clips.player?)))?;
    let (mut player, mut transitions) = players.get_mut(player).ok()?;
    let length = animations.get(&clip).map_or(0.0, AnimationClip::duration);
    if length <= 0.0 || length > SANE {
        return None;
    }
    transitions
        .play(&mut player, node, blend(rate))
        .set_speed(speed * rate);
    Some((node, length))
}

/// Where the clip of `node` stands while it is the main one: its time, and whether it ended.
fn playing(
    player: &AnimationPlayer,
    transitions: &AnimationTransitions,
    node: AnimationNodeIndex,
) -> Option<(f32, bool)> {
    if transitions.get_main_animation() != Some(node) {
        return None;
    }
    let active = player.animation(node)?;
    Some((active.seek_time(), active.is_finished()))
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
            .and_then(|(player, transitions)| playing(player, transitions, picking.node));
        let Some((time, finished)) = active else {
            taking.commands.entity(hero).remove::<Picking>();
            continue;
        };
        if !picking.taken && time >= picking.length / 2.0 {
            picking.taken = true;
            taking.take(picking.item);
        }
        if finished {
            taking.commands.entity(hero).remove::<Picking>();
        }
    }
}

/// A hero that fights, shoots or makes an art, for which the collecting does not start and ends
/// (`0x530C9F`: the AI's states 4, 6, 8, 10 and 11 and the action 7), or that dies.
type Busy = Or<(
    With<attack::Attack>,
    With<attack::Striking>,
    With<crate::making::Making>,
    With<crate::making::Playing>,
    With<crate::npcs::Dies>,
)>;

/// A left click on the bar's Collect button (`UI_TB_COLLECT`), or the key A (`cEngine_receiveEvent`,
/// `0x618090`, #1591), sends the hero collecting: the creature event `0x113`, which the hero takes
/// as the AI's state 12 without a target (`0x530C9F`), unless it is [`Busy`]. The key works over
/// the interface too, so it is not of `frame::Control`; the open console takes it (#1554).
fn send_collecting(
    mut commands: Commands,
    (buttons, keys, console): (
        Res<ButtonInput<MouseButton>>,
        crate::controls::Keys,
        Option<Res<crate::console::Console>>,
    ),
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    images: Res<crate::bar::UiImages>,
    heroes: Query<Entity, With<Body>>,
    busy: Query<(), Busy>,
) {
    let clicked = buttons.just_pressed(MouseButton::Left)
        && crate::inventory::screen_point(&pointer, &ui_scale, &windows).is_some_and(|at| {
            let (index, corner) = crate::bar::COLLECT_BUTTON;
            Rect::from_corners(corner, corner + images.size(index)).contains(at)
        });
    let keyed = keys.just_pressed(crate::controls::Action::Collect)
        && !console.is_some_and(|console| console.is_open());
    if !(clicked || keyed) {
        return;
    }
    for hero in &heroes {
        if busy.contains(hero) {
            debug!("pickup: the hero fights and collects nothing");
            continue;
        }
        info!("pickup: the hero collects the items about it");
        commands
            .entity(hero)
            .remove::<(Picks, Picking, Opens)>()
            .insert(Collects(Collecting::Seeking));
    }
}

/// Sends the hero collecting (the command `collect`), as the bar's Collect button does: a line of
/// what it does.
pub fn start_collecting(world: &mut World) -> Result<String, String> {
    let mut heroes = world.query_filtered::<Entity, With<Body>>();
    let hero = heroes
        .iter(world)
        .next()
        .ok_or("the hero is not in the world yet")?;
    let mut busy = world.query_filtered::<(), Busy>();
    if busy.get(world, hero).is_ok() {
        return Err("the hero fights, makes an art or dies, and collects nothing".into());
    }
    world
        .entity_mut(hero)
        .remove::<(Picks, Picking, Opens)>()
        .insert(Collects(Collecting::Seeking));
    Ok("the hero collects the items about it".into())
}

/// Steps each collecting hero on (`0x4EC560` without a target). Looking, it finds the first item
/// on its layer that it may carry ([`Taking::carries`]) by the square rings of cells about its
/// cell ([`next_item`]) and walks to the item's point, stopping within [`COLLECT_STOP`]; nothing
/// found, it goes idle (`0x54E6D0`). Standing, it plays its pick-up, takes every item of the
/// 3 × 3 cells about it at the clip's half (`cCreature::pickupItem`, `0x558080`), or at once
/// without a clip, and looks again [`LOOKS_AGAIN`] before the clip's end; nothing about it at
/// the half, it goes idle. A fight, an art or another clip in its pick-up's place ends it.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub(crate) fn collect(
    mut taking: Taking,
    heights: Res<Heights>,
    (clips, speed, animations): (
        Option<Res<hero::Clips>>,
        Res<hero::GameSpeed>,
        Res<Assets<AnimationClip>>,
    ),
    lying: Query<(Entity, &GlobalTransform, &Lying)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    busy: Query<(), Busy>,
    mut heroes: Query<(
        Entity,
        &Transform,
        &mut Body,
        &mut Collects,
        Option<&fight::Combat>,
        Option<&fight::Frenzy>,
    )>,
) {
    for (hero, transform, mut body, mut collects, combat, frenzy) in &mut heroes {
        if busy.contains(hero) {
            taking.commands.entity(hero).remove::<Collects>();
            continue;
        }
        let here = transform.translation;
        let level = body.level();
        let on_layer = |at: Vec3| {
            heights
                .standing(at.x, at.z, level)
                .is_some_and(|height| (height - at.y).abs() <= SAME_LAYER)
        };
        let on_layer_items = || {
            lying
                .iter()
                .filter(|(_, _, item)| item.resting)
                .map(|(item, at, _)| (item, at.translation()))
                .filter(|&(_, at)| on_layer(at))
                .collect::<Vec<_>>()
        };
        let rate = hero_rate(combat, frenzy);
        match collects.0 {
            Collecting::Seeking => match next_item(
                here,
                on_layer_items()
                    .into_iter()
                    .filter(|&(item, _)| taking.carries(item)),
            ) {
                Some((item, at)) => {
                    debug!("pickup: the hero collects {item}");
                    body.send_onto(at, level);
                    collects.0 = Collecting::Walking(item);
                }
                None => {
                    info!("pickup: the hero finds nothing more to collect");
                    taking.commands.entity(hero).remove::<Collects>();
                }
            },
            Collecting::Walking(item) => {
                let Some(at) = lying
                    .get(item)
                    .ok()
                    .filter(|(_, _, item)| item.resting)
                    .map(|(_, at, _)| at.translation())
                else {
                    collects.0 = Collecting::Seeking;
                    continue;
                };
                if (at - here).xz().length() <= COLLECT_STOP {
                    body.stop();
                } else if body.target().is_some() || body.ordered() {
                    continue;
                }
                debug!(
                    "pickup: the hero stands {:.0} from {item} and picks up",
                    (at - here).xz().length()
                );
                match play_pickup(clips.as_deref(), &mut players, &animations, (speed.0, rate)) {
                    Some((node, length)) => {
                        collects.0 = Collecting::Picking {
                            node,
                            length,
                            rate,
                            taken: false,
                        };
                    }
                    None if take_about(&mut taking, here, on_layer_items()) => {
                        collects.0 = Collecting::Seeking;
                    }
                    None => {
                        taking.commands.entity(hero).remove::<Collects>();
                    }
                }
            }
            Collecting::Picking {
                node,
                length,
                rate,
                taken,
            } => {
                let active = clips
                    .as_deref()
                    .and_then(|clips| clips.player)
                    .and_then(|player| players.get(player).ok())
                    .and_then(|(player, transitions)| playing(player, transitions, node));
                let Some((time, finished)) = active else {
                    debug!("pickup: another clip takes the collecting's pick-up's place");
                    taking.commands.entity(hero).remove::<Collects>();
                    continue;
                };
                if !taken && (finished || time >= length / 2.0) {
                    collects.0 = Collecting::Picking {
                        node,
                        length,
                        rate,
                        taken: true,
                    };
                    if !take_about(&mut taking, here, on_layer_items()) {
                        info!("pickup: the hero finds nothing about it to collect");
                        taking.commands.entity(hero).remove::<Collects>();
                        continue;
                    }
                }
                if matches!(collects.0, Collecting::Picking { taken: true, .. })
                    && (finished || time >= length - LOOKS_AGAIN * speed.0 * rate)
                {
                    collects.0 = Collecting::Seeking;
                }
            }
        }
    }
}

/// The cells of the square ring `radius` about the cell `centre`, by its rows from the least y to
/// the greatest, each from the least x (`0x4EC560`).
fn ring((x, y): paths::Cell, radius: i32) -> impl Iterator<Item = paths::Cell> {
    (-radius..=radius).flat_map(move |dy| {
        let edge = dy.abs() == radius;
        (-radius..=radius)
            .filter(move |&dx| edge || dx.abs() == radius)
            .map(move |dx| (x + dx, y + dy))
    })
}

/// The item that the collecting hero at `here` goes for among `items`, each at its point: of the
/// first cell that holds one by the rings 0 to [`COLLECTS_WITHIN`] about the hero's cell
/// ([`ring`]), the one nearest the hero (**hypothesis**: the original takes the first of the
/// cell's chain).
fn next_item<T: Copy>(here: Vec3, items: impl IntoIterator<Item = (T, Vec3)>) -> Option<(T, Vec3)> {
    let centre = paths::cell_of(here.x, here.z);
    let mut by_cell: HashMap<paths::Cell, Vec<(T, Vec3)>> = HashMap::new();
    for (item, at) in items {
        let cell = paths::cell_of(at.x, at.z);
        if (cell.0 - centre.0).abs() <= COLLECTS_WITHIN
            && (cell.1 - centre.1).abs() <= COLLECTS_WITHIN
        {
            by_cell.entry(cell).or_default().push((item, at));
        }
    }
    (0..=COLLECTS_WITHIN)
        .flat_map(|radius| ring(centre, radius))
        .find_map(|cell| {
            by_cell.get(&cell)?.iter().copied().min_by(|a, b| {
                let d = |at: Vec3| (at - here).xz().length_squared();
                d(a.1).total_cmp(&d(b.1))
            })
        })
}

/// Takes every item of `items` that lies on the 3 × 3 cells about the hero at `here`
/// (`0x4EC560`'s sub-state 3): whether any lay there.
fn take_about(taking: &mut Taking, here: Vec3, items: Vec<(Entity, Vec3)>) -> bool {
    let centre = paths::cell_of(here.x, here.z);
    let about: Vec<Entity> = items
        .into_iter()
        .filter(|&(_, at)| {
            let cell = paths::cell_of(at.x, at.z);
            (cell.0 - centre.0).abs() <= 1 && (cell.1 - centre.1).abs() <= 1
        })
        .map(|(item, _)| item)
        .collect();
    for &item in &about {
        taking.take(item);
    }
    !about.is_empty()
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
            named: None,
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
            named: None,
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
            .add_message::<TakenPlaced>()
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
                    send_collecting.before(collect),
                    collect.after(picking),
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
            named: None,
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
    fn a_ring_goes_by_its_rows_from_the_least_y_each_from_the_least_x() {
        assert_eq!(ring((10, 20), 0).collect::<Vec<_>>(), [(10, 20)]);
        assert_eq!(
            ring((10, 20), 1).collect::<Vec<_>>(),
            [
                (9, 19),
                (10, 19),
                (11, 19),
                (9, 20),
                (11, 20),
                (9, 21),
                (10, 21),
                (11, 21)
            ]
        );
        assert_eq!(ring((0, 0), 19).count(), 8 * 19);
    }

    #[test]
    fn the_collecting_goes_for_the_first_ring_that_holds_an_item() {
        let cell = |x: i32, y: i32| {
            let centre = paths::centre((x, y));
            Vec3::new(centre.x, 0.0, centre.y)
        };
        let here = cell(100, 100);
        // The nearer ring wins over a nearer point of a farther one, and within the ring the
        // row of the least y; nothing beyond 19 rings.
        let items = [
            (1, cell(102, 99)),
            (2, cell(99, 101)),
            (3, cell(101, 98)),
            (4, cell(100, 120)),
        ];
        assert_eq!(next_item(here, items).map(|(item, _)| item), Some(2));
        assert_eq!(next_item(here, [(4, cell(100, 120))]), None);
        assert_eq!(
            next_item(here, [(5, cell(100, 119))]).map(|(item, _)| item),
            Some(5)
        );
        assert_eq!(
            next_item(here, items[..1].iter().copied()).map(|(item, _)| item),
            Some(1)
        );
        // In one ring, the row of the least y first, however near the other.
        assert_eq!(
            next_item(here, [items[0], items[2]]).map(|(item, _)| item),
            Some(3)
        );
        let on_its_cell = [(6, cell(101, 101)), (7, here + Vec3::new(5.0, 0.0, 5.0))];
        assert_eq!(next_item(here, on_its_cell).map(|(item, _)| item), Some(7));
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
    fn the_belt_finds_the_potions_row_by_row_and_a_drink_uses_one_up() {
        let mut pack = Pack::default();
        // Taken by columns: the sword down the first, the potions after it.
        assert!(pack.take(dropped(Kind::Weapon, 1712, 1), (1, 3)));
        assert!(pack.put(dropped(Kind::Potion, 5171, 1), (2, 0), (1, 1)));
        assert!(pack.put(dropped(Kind::Potion, 5145, 1), (1, 1), (1, 1)));
        assert!(pack.put(dropped(Kind::Potion, 5172, 1), (1, 0), (1, 1)));
        let potions = pack.potions();
        let types: Vec<u32> = potions.iter().map(|&(_, item)| item).collect();
        assert_eq!(types, [5172, 5171, 5145]);
        let (key, _) = potions[1];
        assert_eq!(pack.use_up(key).map(|d| d.item), Some(5171));
        assert_eq!(pack.use_up(key), None);
        assert_eq!(pack.potions().len(), 2);
        assert!(pack.grid.at((2, 0)).is_none());
        // It sparkles where it lay.
        let sparkles: Vec<_> = pack.used.iter().map(|(p, item)| (p.at, *item)).collect();
        assert_eq!(sparkles, [((2, 0), 5171)]);
    }

    #[test]
    fn a_scripts_item_keeps_its_use_and_leaves_without_a_sparkle() {
        let mut pack = Pack::default();
        assert!(pack.take(dropped(Kind::Other, 6080, 1), (1, 1)));
        pack.bind_use("heilung".into());
        assert!(pack.take(dropped(Kind::Potion, 5171, 1), (1, 1)));
        let (first, second) = (pack.items[0].0, pack.items[1].0);
        assert_eq!(pack.use_of(first), Some("heilung"));
        assert_eq!(pack.use_of(second), None);
        // Lifted and put back, it keeps its key and its use.
        assert!(pack.lift((0, 0)));
        assert_eq!(pack.put_held((0, 0)), Put::Placed);
        assert_eq!(pack.use_of(first), Some("heilung"));
        let (placed, dropped) = pack.take_out(first).unwrap();
        assert_eq!((placed.at, dropped.item), ((0, 0), 6080));
        assert_eq!(pack.use_of(first), None);
        assert!(pack.used.is_empty());
    }

    #[test]
    fn a_book_joins_the_books_read_once() {
        let mut pack = Pack::default();
        assert!(pack.read(4097));
        assert!(pack.read(4125));
        assert!(!pack.read(4097));
        assert_eq!(pack.books, [4097, 4125]);
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
    fn an_item_is_put_on_taken_off_and_worn_from_the_grid() {
        let mut pack = Pack::default();
        assert!(pack.put(dropped(Kind::Armour, 3005, 1), (0, 0), (2, 2)));
        assert!(pack.put(dropped(Kind::Armour, 3006, 1), (4, 0), (2, 2)));
        // Lifted onto the cursor and put on: worn, the cursor empty.
        assert!(pack.lift((0, 0)));
        assert!(pack.put_on(0, (2, 2)));
        assert!(pack.held.is_none());
        assert_eq!(pack.worn[0].map(|(_, d)| d.item), Some(3005));
        // The other worn from the grid in its place: the first back where the second lay.
        let second = pack.grid.at((4, 0)).map(|placed| placed.key).unwrap();
        assert!(pack.wear(second, 0, (2, 2)));
        assert_eq!(pack.worn[0].map(|(_, d)| d.item), Some(3006));
        assert_eq!(pack.grid.at((4, 0)).map(|placed| placed.size), Some((2, 2)));
        // Taken off onto the cursor, then by the right button into the grid.
        assert!(pack.take_off(0, (2, 2)));
        assert_eq!(pack.held.map(|held| held.dropped.item), Some(3006));
        assert!(pack.put_on(0, (2, 2)));
        assert!(pack.unwear(0, (2, 2)));
        assert!(pack.worn[0].is_none());
        assert_eq!(pack.items.len(), 2);
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
