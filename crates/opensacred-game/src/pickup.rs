//! The hero picks up the items on the ground (#911; docs/re/items.md, "Picking up"). A press of
//! the left button that goes to no attack, on an item lying under the cursor, sends the hero to
//! the item's point and 25 along both axes, as the AI's state 1 walks a creature to an item
//! (`0x53A170`); standing, within a hero's reach of 100 and with a straight line of walkable
//! cells to it (`0x4FCC80`), the hero takes it, else it gives up. Gold goes into the hero's purse
//! (`0x57FD80`), every other item into its pack (docs/decisions/2026-10-26-pickup.md).

use bevy::prelude::*;
use opensacred_rules::drops::{Dropped, Kind};

use crate::attack::{self, Aim};
use crate::doors::Opens;
use crate::drops::Lying;
use crate::frame::{Control, Step, Unpaused};
use crate::game::AppState;
use crate::hero::{self, Body};
use crate::hud::Pointer;
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

/// What the hero carries: its gold (the stats' word `+0x46`), and the items it picked up, in the
/// order taken.
#[derive(Resource, Default, Debug)]
pub struct Pack {
    pub gold: u32,
    pub items: Vec<Dropped>,
    /// The main quest's items, which the scripts' collecting keeps (`SetOnCollect`, #1021).
    pub quest: Vec<Dropped>,
}

/// An item that the hero took into its pack, its type, which the scripts' collecting counts
/// (`cCreature::pickupItem` at `0x558175`, `scripts`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Taken(pub u32);

impl Pack {
    /// Takes `dropped`: gold into the purse, held below `0x7FFFFFFF` (`0x57FD80`), any other item
    /// into the pack.
    pub fn take(&mut self, dropped: Dropped) {
        match dropped.kind {
            Kind::Gold => {
                self.gold = self.gold.saturating_add(dropped.amount).min(0x7FFF_FFFE);
            }
            _ => self.items.push(dropped),
        }
    }

    /// Moves the last item of the type `item` of the pack among the main quest's items.
    pub fn keep_for_quest(&mut self, item: u32) {
        if let Some(at) = self.items.iter().rposition(|dropped| dropped.item == item) {
            let kept = self.items.remove(at);
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
    camera: Single<(&Camera, &GlobalTransform)>,
    items: Query<(Entity, &GlobalTransform, &Lying)>,
    mut heroes: Query<(Entity, &mut Body)>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for (hero, _) in &heroes {
        commands.entity(hero).remove::<Picks>();
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

/// The hero that stands, once its way to an item it was sent to ends: within its reach of the
/// item, with a straight line of walkable cells to it, it takes it, else it gives up; gold
/// sounds `GOLD01` and the weapon taken back into its hand `UI_INV_PUTMETAL01`, in the middle
/// (`cCreature::pickupItem`, the events `(9, 0x83)` and `(9, 5)`; #1167), and the scripts'
/// collecting sounds any other item (`scripts`).
pub(crate) fn take(
    mut commands: Commands,
    mut pack: ResMut<Pack>,
    (mut taken, mut sounds): (MessageWriter<Taken>, MessageWriter<sfx::Play>),
    (arsenal, mut throws): (
        Option<Res<crate::arsenal::Arsenal>>,
        MessageWriter<crate::arsenal::Throws>,
    ),
    heights: Res<Heights>,
    items: Query<(&GlobalTransform, &Lying)>,
    heroes: Query<(Entity, &Transform, &Body, &Picks)>,
) {
    for (hero, transform, body, picks) in &heroes {
        if body.target().is_some() {
            continue;
        }
        commands.entity(hero).remove::<Picks>();
        let Ok((at, lying)) = items.get(picks.0) else {
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
        info!(
            "pickup: the hero takes {:?} {} x{}",
            lying.dropped.kind, lying.dropped.item, lying.dropped.amount
        );
        // The weapon the hero hurled goes back into its empty hand (`0x54ACC0`, #755).
        if arsenal
            .as_deref()
            .is_some_and(|arsenal| arsenal.takes_back(&lying.dropped))
        {
            throws.write(crate::arsenal::Throws(false));
            sounds.write(sfx::Play {
                sound: sfx::Of::Id(PUT_METAL),
                at: None,
                rate: (0.45, sfx::Roll::Share(0.1)),
            });
        } else {
            if lying.dropped.kind == Kind::Gold {
                sounds.write(sfx::Play {
                    sound: sfx::Of::Id(GOLD),
                    at: None,
                    rate: (0.3, sfx::Roll::Share(0.4)),
                });
            } else {
                taken.write(Taken(lying.dropped.item));
            }
            pack.take(lying.dropped);
        }
        commands.entity(picks.0).despawn();
    }
}

/// Whether a hero standing at `hero` takes the item at `item`: within [`REACH`] of it on the
/// ground, the integer root rounded up (`0x53A170`).
fn takes(hero: Vec3, item: Vec3) -> bool {
    (item - hero).xz().length().ceil() <= REACH
}

/// The hero's pack, empty at each game's start.
fn empty(mut commands: Commands) {
    commands.insert_resource(Pack::default());
}

/// The items that the hero picks up.
pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pack>()
            .add_message::<Taken>()
            .add_systems(OnEnter(AppState::InGame), empty)
            .add_systems(
                Update,
                (
                    pick.in_set(Control)
                        .after(attack::aim)
                        .after(hero::steer)
                        .after(crate::doors::pick),
                    take.after(hero::run),
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
    fn gold_goes_into_the_purse_and_the_rest_into_the_pack() {
        let mut pack = Pack::default();
        pack.take(dropped(Kind::Gold, 5133, 37));
        pack.take(dropped(Kind::Weapon, 1712, 1));
        pack.take(dropped(Kind::Gold, 5132, 5));
        pack.take(dropped(Kind::Other, 6005, 1));
        assert_eq!(pack.gold, 42);
        let items: Vec<u32> = pack.items.iter().map(|d| d.item).collect();
        assert_eq!(items, [1712, 6005]);
        pack.gold = 0x7FFF_FFF0;
        pack.take(dropped(Kind::Gold, 5135, 1000));
        assert_eq!(pack.gold, 0x7FFF_FFFE);
    }
}
