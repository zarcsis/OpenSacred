//! A hero's combos (docs/re/arts.md, "A hero's combos"): the four that every hero has, their
//! steps (an art and its level each), their time, speed and regeneration, and what a combat master
//! asks for making one. `opensacred-game` (`combos_page`, `art_slots`) keeps them and shows them;
//! their run in play is #1531's.

use opensacred_data::tables::Arts as Tables;

use crate::arts;

/// The combos a hero has, made empty by its stats' constructor (`0x55E1B0`), and the first's id
/// (`0x200` and the index).
pub const COMBOS: usize = 4;
pub const FIRST: u16 = 0x200;
/// The steps of a combo.
pub const STEPS: usize = 4;
/// The least time of a combo (`0x5813D0`: below 0.001 it is held at 1.0).
const LEAST_TIME: f32 = 0.001;
/// What a step changed costs at a master by its level (`0x4D2770`).
const PRICE_PER_LEVEL: u32 = 100;

/// A combo's step: its art and the level it makes it at, the art's with its bonus as the step took
/// it.
pub type Step = Option<(u16, u8)>;

/// A hero's combo (the stats' list at `+0x10E`, entries of 38 bytes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Combo {
    /// Its id, `0x200` and its index (`+0`).
    pub id: u16,
    /// Its four steps (`+2`, `+0x12`).
    pub steps: [Step; STEPS],
    /// Its time (`+0x1A`), its regeneration's speed (`+0x1E`) and the time regenerated, counting
    /// up (`+0x22`).
    pub time: f32,
    pub speed: f32,
    pub regenerated: f32,
}

impl Combo {
    /// The combo of the index `index` as a new hero has it (`0x55E1B0`): no steps, its time 1.0.
    pub fn empty(index: u16) -> Self {
        Self {
            id: FIRST + index,
            steps: [None; STEPS],
            time: 1.0,
            speed: 1.0,
            regenerated: 0.0,
        }
    }

    /// Sets its steps (`0x5813D0`): its time 1.0 plus each step's time at its level from the arts'
    /// tables (`0x4D1C00`, `0x4D25C0`), 1.0 where that comes below 0.001; its speed 1.0 and its
    /// regeneration from 0.
    pub fn set(&mut self, steps: [Step; STEPS], tables: &Tables) {
        self.steps = steps;
        let sum: f64 = steps
            .iter()
            .flatten()
            .map(|&(art, level)| arts::time(tables, art, u32::from(level)).unwrap_or(0.0))
            .sum();
        let time = narrow(1.0 + sum);
        self.time = if time < LEAST_TIME { 1.0 } else { time };
        self.speed = 1.0;
        self.regenerated = 0.0;
    }

    /// The steps that hold an art.
    pub fn held_steps(&self) -> usize {
        self.steps.iter().flatten().count()
    }

    /// Its speed from the hero's regenerations (docs/re/arts.md): 1 plus the mean over its arts of
    /// their attribute over 100, `magic` for a spell (an id below 1000) and `physical` for a
    /// weapon art; 1.0 without an art.
    pub fn speed_of(&self, magic: f32, physical: f32) -> f32 {
        let attributes: Vec<f32> = self
            .steps
            .iter()
            .flatten()
            .map(|&(art, _)| if art < 1000 { magic } else { physical })
            .collect();
        if attributes.is_empty() {
            return 1.0;
        }
        let count = f32::from(u16::try_from(attributes.len()).unwrap_or(u16::MAX));
        1.0 + attributes.iter().sum::<f32>() / count / 100.0
    }

    /// Whether it is ready (`0x581A60`): its time regenerated.
    pub fn ready(&self) -> bool {
        self.regenerated >= self.time
    }

    /// The share of its time regenerated (`0x581A20`), held at 1.
    pub fn share(&self) -> f32 {
        if self.time <= 0.0 {
            1.0
        } else {
            (self.regenerated / self.time).clamp(0.0, 1.0)
        }
    }

    /// Its regeneration on by `seconds` at its speed while it is below its time (`0x562730`),
    /// which its last tick may pass (seen 2026-10-30, #1531: 24.01 s of 24).
    pub fn regenerate(&mut self, seconds: f32) {
        if self.regenerated < self.time {
            self.regenerated += seconds * self.speed;
        }
    }

    /// The time its tooltip gives (`0x581550`, the mode 3): its time over its speed.
    pub fn shown_time(&self) -> f32 {
        self.time / self.speed
    }
}

/// A use of an art while a combo runs or its mark holds (`0x562470`, its combos' branch, the
/// art's time `time`): the running combo's regeneration, `running` its index, back to 0, then
/// every combo's less the time, held within 0 and its time; the arts' own regeneration does not
/// start (seen 2026-10-30, #1531).
pub fn used(combos: &mut [Combo], running: Option<usize>, time: f32) {
    for (index, combo) in combos.iter_mut().enumerate() {
        if running == Some(index) {
            combo.regenerated = 0.0;
        }
        combo.regenerated = (combo.regenerated - time).max(0.0).min(combo.time);
    }
}

/// A yellow potion's cocktail (`0x562470` with the kind 3 and the time −40 times its per
/// cent): every combo's regeneration set to `factor` times its time, held at its time,
/// `factor` its size's cocktail of the balance over the difficulty's `PotionDiv`
/// ([`crate::potions::Drink::Refills`]); no share of 100 is taken, so that every cocktail of
/// the installation's balance, 120 at least over 5 at most, fills every combo.
pub fn refilled(combos: &mut [Combo], factor: f32) {
    for combo in combos {
        combo.regenerated = (factor * combo.time).min(combo.time);
    }
}

/// A hero's four combos as its stats' constructor makes them.
pub fn new_hero() -> [Combo; COMBOS] {
    std::array::from_fn(|index| Combo::empty(u16::try_from(index).unwrap_or(0)))
}

/// The index among a hero's combos of the combo `id`, none for another art.
pub fn index_of(id: u16) -> Option<usize> {
    id.checked_sub(FIRST)
        .map(usize::from)
        .filter(|&index| index < COMBOS)
}

/// What a combat master asks for making the steps `new` of a combo whose steps are `old`
/// (`0x6D23E0`): the steps that hold an art times the sum over the steps changed, by their art or
/// their level (`0x6D1F80`), of 100 times their level.
pub fn cost(old: &[Step; STEPS], new: &[Step; STEPS]) -> u32 {
    let held = u32::try_from(new.iter().flatten().count()).unwrap_or(0);
    let changed: u32 = old
        .iter()
        .zip(new)
        .filter(|(old, new)| old != new)
        .filter_map(|(_, new)| *new)
        .map(|(_, level)| PRICE_PER_LEVEL * u32::from(level))
        .sum();
    held * changed
}

/// Whether a master's button may be pressed (`0x6D26D0`): a step holds an art and the gold is the
/// cost or more.
pub fn affordable(new: &[Step; STEPS], cost: u32, gold: u32) -> bool {
    new.iter().any(Option::is_some) && cost <= gold
}

/// Whether the master makes the combo (`0x6D22A0`): the gold above the cost, strictly, so that
/// gold equal to the cost lights the button and makes nothing (**seen**).
pub fn makes(cost: u32, gold: u32) -> bool {
    gold > cost
}

/// A float of the original's stores to an `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a combo's time, a few hundred seconds, rounded to the f32 the original stores"
)]
fn narrow(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use opensacred_data::tables::WeaponArt;

    use super::*;

    fn tables() -> Tables {
        let art = |id: u16, base: f32, step: f32| WeaponArt {
            id,
            name: 0,
            description: 0,
            short_description: 0,
            icon: String::new(),
            sprites: [0; 3],
            value_3c: 0,
            regeneration: [base, step],
            distance: 0,
            launch: 0,
            factor: [0.0; 2],
            value_58: [0.0; 2],
            value_60: [0; 2],
            value_68: [0; 2],
            classes: 0,
            flags: 0,
        };
        Tables {
            spells: Vec::new(),
            weapon_arts: vec![
                art(1070, 11.0, 1.0),
                art(1071, 30.0, 1.0),
                art(1072, 30.0, 1.0),
            ],
        }
    }

    #[test]
    fn a_new_heros_four_combos_are_empty() {
        let combos = new_hero();
        assert_eq!(combos.map(|combo| combo.id), [0x200, 0x201, 0x202, 0x203]);
        assert!(
            combos
                .iter()
                .all(|combo| combo.held_steps() == 0 && combo.time == 1.0)
        );
        assert_eq!(
            combos.map(|combo| index_of(combo.id)),
            [0, 1, 2, 3].map(Some)
        );
        assert_eq!((index_of(0x204), index_of(1071)), (None, None));
    }

    #[test]
    fn a_combos_time_is_one_and_its_steps_times() {
        let mut combo = Combo::empty(0);
        combo.set(
            [Some((1070, 3)), Some((1071, 2)), Some((1072, 2)), None],
            &tables(),
        );
        // 1 + (11 + 3) + (30 + 2) + (30 + 2).
        assert!((combo.time - 79.0).abs() < 1e-4);
        // Its regenerations of 37 each: 1.37, and the tooltip's 79 / 1.37.
        combo.speed = combo.speed_of(37.0, 37.0);
        assert!((combo.speed - 1.37).abs() < 1e-5);
        assert!((combo.shown_time() - 79.0 / 1.37).abs() < 1e-3);
        combo.regenerate(10.0);
        assert!((combo.regenerated - 13.7).abs() < 1e-4 && !combo.ready());
        combo.regenerate(100.0);
        assert!(combo.ready() && combo.share() == 1.0);
    }

    #[test]
    fn a_step_takes_its_time_from_every_combo_and_its_own_back_to_none() {
        let mut combos = new_hero();
        for (combo, regenerated) in combos.iter_mut().zip([24.0, 10.0, 3.0, 1.0]) {
            combo.time = 24.0;
            combo.regenerated = regenerated;
        }
        // The first combo runs, its step of 1071's 15 s: its own from 0, the others less 15.
        used(&mut combos, Some(0), 15.0);
        let after: Vec<f32> = combos.iter().map(|c| c.regenerated).collect();
        assert_eq!(after, [0.0, 0.0, 0.0, 0.0]);
        combos[1].regenerated = 20.0;
        // With the mark alone, none runs: each less the time.
        used(&mut combos, None, 8.0);
        assert_eq!(combos[1].regenerated, 12.0);
        // The regeneration passes its time by its last tick, as the original's (24.01 of 24), and
        // stops there.
        combos[1].regenerate(10.0);
        assert_eq!(combos[1].regenerated, 22.0);
        combos[1].regenerate(10.0);
        assert_eq!(combos[1].regenerated, 32.0);
        combos[1].regenerate(10.0);
        assert_eq!(combos[1].regenerated, 32.0);
    }

    #[test]
    fn a_cocktail_fills_every_combo() {
        let mut combos = new_hero();
        combos[0].time = 24.0;
        combos[2].time = 66.4;
        refilled(&mut combos, 120.0 / 5.0);
        assert!(combos.iter().all(Combo::ready));
        assert_eq!(combos[2].regenerated, 66.4);
    }

    #[test]
    fn a_master_asks_by_the_steps_held_and_changed() {
        let old = [None; STEPS];
        let new = [Some((1070, 3)), Some((1071, 2)), Some((20, 1)), None];
        // Three steps hold an art, each changed: 3 · (300 + 200 + 100) = 1,800 (seen).
        assert_eq!(cost(&old, &new), 1_800);
        // A fourth of the level 2 added: 4 · 200 = 800.
        let more = [new[0], new[1], new[2], Some((1072, 2))];
        assert_eq!(cost(&new, &more), 800);
        // The button lit at the cost, the make strictly above it (seen).
        assert!(affordable(&new, 1_800, 1_800));
        assert!(!makes(1_800, 1_800));
        assert!(makes(1_800, 1_801));
        assert!(!affordable(&old, 0, 100));
    }
}
