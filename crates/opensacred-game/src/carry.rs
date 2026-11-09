//! A saved hero carried into a game, as the original's loader puts the hero's file over the hero
//! made from its class's template (`0x6058A0`; docs/re/underworld.md, "What the hero's file
//! carries"; #2362): once the hero stands, its level and experience, the attribute points spent
//! on each attribute and those left, its skills with their bases, the skill points and the
//! skills left to learn, and its gold, of its creature section (docs/02 §9, "The creature"); its
//! items in place of its template's: those its equipment's slots hold worn in them, those of its
//! backpack at their cells and its weapon sets, the set in its hands selected, each of its
//! record's values with the items in its sockets; its values made anew at its level and its
//! health full once the items' are in them. Its combat arts are its file's from the start
//! (`game`).

use bevy::prelude::*;
use opensacred_data::heroes::{HeroEntry, Socketed, Worn};
use opensacred_rules::drops::{Dropped, Kind};
use opensacred_rules::inventory::Grid;

use crate::drops::Tables;
use crate::fight::{Base, Combat, Experience, Health, Holding, Regenerates, Rules};
use crate::frame::Step;
use crate::game::AppState;
use crate::hero::Body;
use crate::pickup::Pack;
use crate::sets::{SETS, WeaponSets};

/// The equipment's slots of the hands, the left's and the right's, whose items are the selected
/// weapon set's (#1271).
const HANDS: [u8; 2] = [12, 13];
/// How many frames the carried hero's health is held full after the carry, while its items'
/// values come into its most health (`equipment`, `sets`, `fight::keep_health`).
const FILLING: u8 = 8;

/// A saved hero to carry into the game once its hero stands: its level, its creature section's
/// stats and its items.
#[derive(Resource, Clone, Debug)]
pub struct Carry {
    level: u32,
    stats: opensacred_data::heroes::HeroStats,
    hero: HeroEntry,
}

impl Carry {
    /// The values and items of the saved hero `hero`.
    pub fn of(hero: &HeroEntry) -> Self {
        Self {
            level: hero.level,
            stats: hero.stats,
            hero: hero.clone(),
        }
    }

    /// The item of the object `id` that the hero wears or holds.
    fn worn(&self, id: u32) -> Option<&Worn> {
        self.hero.worn.iter().find(|worn| worn.id == id)
    }

    /// Its weapon sets, each half's item of the object its file names, and the selected one the
    /// set whose halves its hands' slots hold (`0x6058A0` puts the hands' items in the slots 12
    /// and 13), else the first.
    fn sets(&self, tables: Option<&Tables>) -> WeaponSets {
        let mut sets = WeaponSets::default();
        for (set, halves) in self.hero.sets.iter().enumerate().take(SETS) {
            for (half, &id) in halves.iter().enumerate() {
                sets.halves[set][half] = self
                    .worn(id)
                    .map(|worn| dropped(tables, (worn.item, &worn.words), &worn.socketed));
            }
        }
        let hands = HANDS.map(|slot| {
            self.hero
                .equipment
                .iter()
                .find(|&&(at, _)| at == slot)
                .map_or(0, |&(_, id)| id)
        });
        sets.selected = self
            .hero
            .sets
            .iter()
            .position(|&halves| halves == hands && hands != [0, 0])
            .unwrap_or(0);
        sets
    }

    /// Puts its items into the pack `pack` in place of its template's: its backpack's at their
    /// cells and those its equipment's slots hold but the hands' worn, its gold its own.
    fn pack(&self, pack: &mut Pack, tables: Option<&Tables>) {
        pack.gold = self.stats.gold;
        pack.items.clear();
        pack.grid = Grid::default();
        pack.worn = Default::default();
        for carried in &self.hero.backpack {
            let item = dropped(tables, (carried.item, &carried.words), &carried.socketed);
            if !pack.put(item, carried.at, carried.size) {
                warn!(
                    "carry: the item {} does not fit at {:?}",
                    carried.item, carried.at
                );
            }
        }
        for &(slot, id) in &self.hero.equipment {
            if HANDS.contains(&slot) {
                continue;
            }
            let Some(worn) = self.worn(id) else {
                warn!("carry: no item {id} for the slot {slot}");
                continue;
            };
            let item = dropped(tables, (worn.item, &worn.words), &worn.socketed);
            if !pack.wear_picked(item, slot) {
                warn!(
                    "carry: the slot {slot} does not take the item {}",
                    worn.item
                );
            }
        }
    }

    /// Its experience: its points at its level, the points and skills its file leaves it, and
    /// the skills it holds counted as it learnt them.
    fn experience(&self) -> opensacred_rules::fight::Experience {
        let s = &self.stats;
        let held = s.skills.iter().filter(|&&skill| skill != 0).count();
        opensacred_rules::fight::Experience::carried(
            (u64::from(s.experience), self.level),
            (
                u32::from(s.attribute_points),
                u32::from(s.skill_points),
                u32::from(s.to_learn),
            ),
            u32::try_from(held).unwrap_or(0),
        )
    }

    /// Its class's values `base` with its own skills, their bases and the attribute points it
    /// spent.
    fn base(&self, base: &mut opensacred_rules::fight::Base) {
        base.skills = self.stats.skills;
        base.levels = self.stats.bases;
        base.extras = self.stats.spent;
    }
}

/// An item of the hero's file, as the game holds it (as `sets` makes its template's): of its
/// type's kind, its record's values at its template's scale, by which its own level is worked
/// out as the client works it out for every record it loads (`0x5D3AB0`), and the items in its
/// sockets of theirs with their smiths' tiers (#1661).
fn dropped(
    tables: Option<&Tables>,
    (item, words): (u32, &[u16]),
    socketed: &[Socketed],
) -> Dropped {
    let kind = tables
        .and_then(|tables| tables.catalogue().kind(item))
        .unwrap_or(Kind::Other);
    let mut values = opensacred_rules::fight::ItemValues::of_record(words);
    values.scale = tables.map_or(0, |tables| tables.template_scale(item));
    Dropped {
        kind,
        item,
        level: 0,
        key: 16,
        quality: 0,
        amount: 1,
        values: Some(values),
        named: None,
        socketed: socketed
            .iter()
            .map(|socket| {
                let inner = dropped(tables, (socket.item, &socket.words), &[]);
                let values = tables.map_or_else(
                    || opensacred_rules::fight::ItemValues::of_record(&socket.words),
                    |tables| tables.values(&inner),
                );
                opensacred_rules::smith::Socketed {
                    item: inner,
                    values,
                    tier: socket.tier,
                }
            })
            .collect(),
    }
}

/// The carried hero's health held full for a few more frames ([`FILLING`]).
#[derive(Resource)]
struct Filling(u8);

/// Carries the saved hero into the game once its hero stands in place (`hero::Ready`): its
/// experience, its base's skills and points, its gold, its items and weapon sets, its values made
/// anew at its level and its health full; the taskbar's "+" as its points left it.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn carry(
    mut commands: Commands,
    carry: Res<Carry>,
    (rules, mut experience, mut pack): (Res<Rules>, ResMut<Experience>, ResMut<Pack>),
    (tables, mut unearned): (Option<Res<Tables>>, ResMut<crate::rewards::Unearned>),
    mut heroes: Query<
        (
            &mut Base,
            Option<&Holding>,
            Option<&crate::effects::Effects>,
            &mut Combat,
            &mut Regenerates,
            &mut Health,
            Option<&crate::riding::Saddle>,
        ),
        With<Body>,
    >,
    (mut arts, mut hero): (
        Option<ResMut<crate::arts::HeroArts>>,
        Option<ResMut<crate::hero::Hero>>,
    ),
) {
    let Some((mut base, holding, effects, mut combat, mut regenerates, mut health, saddle)) =
        heroes.iter_mut().next()
    else {
        return;
    };
    commands.remove_resource::<Carry>();
    experience.0 = carry.experience();
    unearned.0 = true;
    carry.base(&mut base.0);
    carry.pack(&mut pack, tables.as_deref());
    commands.insert_resource(carry.sets(tables.as_deref()));
    commands.insert_resource(Filling(FILLING));
    crate::statistics::refit(
        carry.level,
        &rules,
        (&base, holding, effects, saddle),
        (&mut combat, &mut regenerates, &mut health),
        (arts.as_deref_mut(), hero.as_deref_mut()),
    );
    health.current = health.most;
    health.level = carry.level;
    commands.insert_resource(crate::points::Plus::loaded(&experience.0));
    info!(
        "carry: the hero of level {} carried, {} experience, {} gold, the skills {:?}, {} items \
         worn and {} in the backpack",
        carry.level,
        carry.stats.experience,
        carry.stats.gold,
        carry.stats.skills,
        pack.worn.iter().flatten().count(),
        pack.items.len()
    );
}

/// Holds the carried hero's health at its most while its items' values come into it.
fn fill(
    mut commands: Commands,
    mut filling: ResMut<Filling>,
    mut heroes: Query<&mut Health, With<Body>>,
) {
    for mut health in &mut heroes {
        health.current = health.most;
    }
    filling.0 = filling.0.saturating_sub(1);
    if filling.0 == 0 {
        commands.remove_resource::<Filling>();
    }
}

/// Carries a saved hero's values into its game.
pub struct CarryPlugin;

impl Plugin for CarryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), |mut commands: Commands| {
            commands.remove_resource::<Carry>();
            commands.remove_resource::<Filling>();
        })
        .add_systems(
            PostUpdate,
            fill.run_if(in_state(AppState::InGame))
                .run_if(resource_exists::<Filling>),
        )
        .add_systems(
            Update,
            carry
                .after(Step::HeroSpawn)
                .run_if(in_state(AppState::InGame))
                .run_if(resource_exists::<Carry>)
                .run_if(|ready: Option<Res<crate::hero::Ready>>| ready.is_some_and(|r| r.0)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An item worn of the object `id` and the type `item`, its record's words empty.
    fn worn(id: u32, item: u32) -> Worn {
        Worn {
            id,
            item,
            words: Vec::new(),
            socketed: Vec::new(),
        }
    }

    #[test]
    fn the_hands_set_is_selected_and_the_hands_slots_are_not_worn() {
        let hero = HeroEntry {
            worn: vec![
                worn(69, 1848),
                worn(73, 3096),
                worn(74, 1856),
                worn(56, 7725),
            ],
            equipment: vec![(0, 56), (12, 73), (13, 74)],
            sets: vec![[69, 0], [0, 0], [0, 0], [73, 74], [0, 0]],
            ..HeroEntry::default()
        };
        let carry = Carry::of(&hero);
        let sets = carry.sets(None);
        assert_eq!(sets.selected, 3);
        let items = |set: usize| sets.halves[set].clone().map(|half| half.map(|d| d.item));
        assert_eq!(
            (items(0), items(3)),
            ([Some(1848), None], [Some(3096), Some(1856)])
        );
        let mut pack = Pack::default();
        carry.pack(&mut pack, None);
        let worn: Vec<_> = pack
            .worn
            .iter()
            .enumerate()
            .filter_map(|(slot, worn)| Some((slot, worn.as_ref()?.1.item)))
            .collect();
        assert_eq!(worn, [(0, 7725)]);
    }

    #[test]
    fn the_installations_dwarf_carries_his_files_values() {
        let carry = Carry {
            hero: HeroEntry::default(),
            level: 29,
            stats: opensacred_data::heroes::HeroStats {
                experience: 1_800_000,
                gold: 977_384,
                spent: [0, 0, 14, 14, 0, 0],
                attribute_points: 0,
                skills: [2, 10, 11, 5, 29, 0, 0, 0],
                bases: [23, 13, 10, 10, 13, 0, 0, 0],
                skill_points: 9,
                to_learn: 1,
            },
        };
        let experience = carry.experience();
        assert_eq!(
            (
                experience.points,
                experience.level,
                experience.skill_points,
                experience.to_learn()
            ),
            (1_800_000, 29, 9, 1)
        );
        let mut base = opensacred_rules::fight::Base {
            skills: [2, 10, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        };
        carry.base(&mut base);
        assert_eq!(base.skills, carry.stats.skills);
        assert_eq!(base.levels, carry.stats.bases);
        assert_eq!(base.extras, carry.stats.spent);
    }
}
