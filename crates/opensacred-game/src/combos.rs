//! The hero's combos (#1530; docs/re/arts.md, "A hero's combos"): the four that a new game gives
//! it empty (`0x55E1B0`, `opensacred_rules::combos`), their regeneration on the world's tick at
//! the speed of the hero's regenerations, their images in the bar's slots (the combos' table at
//! `0x955348`) and their description's lines (`0x581550`); the console's `combos` lists them and
//! makes one as the combat master does. The combos page shows them (`combos_page`), the master's
//! window makes them (`master`).
//!
//! Their run in play (#1531): the right button with a combo on it runs a ready one, its steps
//! given one after another as orders of their arts at their levels ([`ComboRun`], `order`,
//! `steps`), and the arts' uses while it runs and until the hero stands idle go to the combos'
//! regeneration rather than to their own (`uses`, `opensacred_rules::combos::used`).

use bevy::prelude::*;
use opensacred_rules::combos::{self, COMBOS, Combo, STEPS, Step};

use crate::ai::Mind;
use crate::arts::HeroArts;
use crate::fight::Health;
use crate::fight::{Base, Experience, Holding};
use crate::game::AppState;
use crate::hero::Body;
use crate::lines::Texts;
use crate::making::{Making, Playing};
use crate::npcs::Dies;

/// The image of the first combo's grey picture; each combo's are three, grey, pale and vivid, in
/// `gui_combo01` (the table's `+0x24` to `+0x2C`).
const FIRST_IMAGE: u32 = 801;

/// The hero's combos.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HeroCombos(pub [Combo; COMBOS]);

impl Default for HeroCombos {
    fn default() -> Self {
        Self(combos::new_hero())
    }
}

impl HeroCombos {
    /// The combo of the id `id`, none for another art.
    pub fn get(&self, id: u16) -> Option<&Combo> {
        combos::index_of(id).and_then(|index| self.0.get(index))
    }

    /// Lines for the console: each combo's steps, its time, its speed and what of it has
    /// regenerated.
    pub fn lines(&self) -> Vec<String> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, combo)| {
                let steps: Vec<String> = combo
                    .steps
                    .iter()
                    .flatten()
                    .map(|(art, level)| format!("{art}:{level}"))
                    .collect();
                let steps = if steps.is_empty() {
                    "no steps".to_owned()
                } else {
                    steps.join(" ")
                };
                format!(
                    "combo {} ({:#x}): {steps}, time {:.1} s, speed {:.2}, regenerated {:.1} s, \
                     shown {:.1} s",
                    index + 1,
                    combo.id,
                    combo.time,
                    combo.speed,
                    combo.regenerated,
                    combo.shown_time()
                )
            })
            .collect()
    }
}

/// Whether the art `art` is one of the hero's combos (the cursor's and a slot's kind 4).
pub(crate) fn is_combo(art: u16) -> bool {
    combos::index_of(art).is_some()
}

/// The three images of the combo `id`, grey, pale and vivid.
pub(crate) fn sprites(id: u16) -> Option<[u32; 3]> {
    let first = FIRST_IMAGE + 3 * u32::try_from(combos::index_of(id)?).ok()?;
    Some([first, first + 1, first + 2])
}

/// The steps that `words` name, each `ART:LEVEL`, at most four.
pub(crate) fn parse_steps(words: &[String]) -> Result<[Step; STEPS], String> {
    if words.len() > STEPS {
        return Err(format!("a combo has {STEPS} steps at most"));
    }
    let mut steps = [None; STEPS];
    for (step, word) in steps.iter_mut().zip(words) {
        let parsed = word
            .split_once(':')
            .and_then(|(art, level)| Some((art.parse().ok()?, level.parse().ok()?)))
            .filter(|&(_, level): &(u16, u8)| level > 0)
            .ok_or_else(|| format!("{word}: a step is ART:LEVEL, its level 1 or more"))?;
        *step = Some(parsed);
    }
    Ok(steps)
}

/// The name of the art `art`, of `arts`' tables and `texts`.
pub(crate) fn art_name<'a>(art: u16, arts: &HeroArts, texts: &'a Texts) -> Option<&'a str> {
    let tables = &arts.tables;
    let name = tables
        .spells
        .iter()
        .find(|spell| spell.id == art)
        .map(|spell| spell.name)
        .or_else(|| {
            tables
                .weapon_arts
                .iter()
                .find(|record| record.id == art)
                .map(|record| record.name)
        })?;
    texts.id(name)
}

/// The description's lines of `combo` (`0x581550`'s mode 2): each step's art's name and its level.
pub(crate) fn step_lines(combo: &Combo, arts: &HeroArts, texts: &Texts) -> Vec<String> {
    combo
        .steps
        .iter()
        .flatten()
        .map(|&(art, level)| {
            let name = art_name(art, arts, texts).map_or_else(|| art.to_string(), str::to_owned);
            format!("{name}: {level}")
        })
        .collect()
}

/// The combos' regeneration on the world's tick, as the arts': each at the speed of the hero's
/// regenerations (`Combo::speed_of`), its attributes `MagicRegen` and `PhysicalRegen` grown by its
/// level and raised by its items' (`fight::regenerations`).
fn regenerate(
    tick: Res<crate::frame::WorldTick>,
    experience: Res<Experience>,
    combos: Option<ResMut<HeroCombos>>,
    heroes: Query<(&Base, Option<&Holding>), With<Body>>,
) {
    let (Some(seconds), Some(mut combos), Some((base, holding))) =
        (tick.seconds, combos, heroes.iter().next())
    else {
        return;
    };
    let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
    let (magic, physical) = opensacred_rules::fight::regenerations(base, experience.level, &holds);
    for combo in &mut combos.0 {
        let speed = combo.speed_of(magic, physical);
        if combo.speed != speed {
            combo.speed = speed;
        }
        if !combo.ready() {
            combo.regenerate(seconds);
        }
    }
}

/// A combo that runs: its index, the step it gives next, the creature and the point on the
/// ground of its order (`0xAD3EA8`), whether a step's order is given and whether its art was seen
/// playing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub index: usize,
    pub step: usize,
    target: Option<Entity>,
    ground: Vec3,
    given: bool,
    seen: bool,
}

/// The combo that runs (the creature's `+0x2B4`, its index and 1, and `+0x2B3`, its step) and the
/// mark of a combo whose step an art's use was (`+0x401`, the stats' `+0x59`): while either holds,
/// the arts' uses go to the combos (`0x562470`), and the mark goes once no combo runs and the hero
/// stands idle (`0x542C91`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ComboRun {
    pub running: Option<Run>,
    pub mark: bool,
}

impl ComboRun {
    /// A line for the console, as the original's driver prints it: the combo that runs (its
    /// index and 1, 0 for none), its step and the mark.
    pub fn line(&self) -> String {
        let (running, step) = self.running.map_or((0, 0), |run| (run.index + 1, run.step));
        format!("running {running} step {step} mark {}", u8::from(self.mark))
    }
}

/// The console's run of the combo `id` on the hostile creature nearest the hero, as a right
/// click on it orders it ([`order`]): where none runs and it is ready.
pub(crate) fn make(world: &mut World, id: u16) -> Result<(), String> {
    let index = combos::index_of(id).ok_or("no such combo")?;
    if world.resource::<ComboRun>().running.is_some() {
        return Err("a combo runs".into());
    }
    let ready = world
        .get_resource::<HeroCombos>()
        .and_then(|combos| combos.0.get(index))
        .is_some_and(Combo::ready);
    if !ready {
        return Err(format!("{id:#x} is not ready"));
    }
    let mut heroes = world.query_filtered::<&GlobalTransform, (With<Body>, Without<Dies>)>();
    let here = heroes
        .iter(world)
        .next()
        .ok_or("the hero is not in the world yet")?
        .translation();
    let mut creatures = world.query_filtered::<
        (Entity, &GlobalTransform, &Mind, &Health),
        (Without<Dies>, Without<Body>),
    >();
    let (target, ground) = creatures
        .iter(world)
        .filter(|(_, _, mind, health)| mind.hostile() && health.current > 0)
        .map(|(entity, at, ..)| (entity, at.translation()))
        .min_by(|a, b| (a.1 - here).length().total_cmp(&(b.1 - here).length()))
        .ok_or("no hostile creature is about")?;
    world.resource_mut::<ComboRun>().running = Some(Run {
        index,
        step: 0,
        target: Some(target),
        ground,
        given: false,
        seen: false,
    });
    info!("combos: {id:#x} runs on {target}");
    Ok(())
}

/// The share of a step's clip after which the next step is given: the original's runner waits
/// while 0.3 of it or more is left to play (`0x4FE870`).
const NEXT_AT: f32 = 0.7;

/// A press of the right button with a combo on it (the engine's `+0x58` 2, its id at `+0x5A`):
/// where no combo runs and that one is ready (`0x581A60`), it runs from its first step, on the
/// hostile creature under the cursor and the point of the ground there (`0x530186` to
/// `0x530276`); a combo not ready does nothing (with Ctrl held the original's posts another event,
/// not followed).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn order(
    buttons: Res<ButtonInput<MouseButton>>,
    selected: Res<crate::making::Selected>,
    (pointer, camera): (
        Res<crate::hud::Pointer>,
        Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    ),
    combos: Option<Res<HeroCombos>>,
    mut run: ResMut<ComboRun>,
    heroes: Query<&GlobalTransform, (With<Body>, Without<Dies>)>,
    creatures: Query<(Entity, &GlobalTransform, &Mind, &Health), (Without<Dies>, Without<Body>)>,
) {
    if !buttons.just_pressed(MouseButton::Right) || run.running.is_some() {
        return;
    }
    let (Some(id), Some(combos), Some(cursor), Ok(hero)) = (
        selected.0.filter(|&id| is_combo(id)),
        combos,
        pointer.0,
        heroes.single(),
    ) else {
        return;
    };
    let Some(index) = combos::index_of(id) else {
        return;
    };
    if !combos.0.get(index).is_some_and(Combo::ready) {
        info!("combos: {id:#x} is not ready");
        return;
    }
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let target = creatures
        .iter()
        .filter(|(_, _, mind, health)| mind.hostile() && health.current > 0)
        .map(|(entity, at, ..)| (entity, crate::attack::from_line(ray, at.translation())))
        .filter(|&(_, distance)| distance <= crate::attack::PICKED_WITHIN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity);
    let along = (hero.translation().y - ray.origin.y) / ray.direction.y;
    let ground = ray.origin + ray.direction.as_vec3() * along;
    info!("combos: {id:#x} runs on {target:?}");
    run.running = Some(Run {
        index,
        step: 0,
        target,
        ground,
        given: false,
        seen: false,
    });
}

/// The order of the step's art `art` of the run `run`: a boon on the hero, an art of the launch
/// type 3 at the order's point, else its creature where it lives, else the nearest hostile
/// creature alive (`0x4FE870`'s choice anew; **hypothesis**: its list at `0xAD3CE0` is the
/// creatures about the hero, nearest first); none where none is.
fn target_of(
    arts: &HeroArts,
    art: u16,
    (hero, here): (Entity, Vec3),
    run: &Run,
    creatures: &[(Entity, Vec3, bool)],
) -> Option<(Entity, Option<Vec3>)> {
    use opensacred_rules::arts::Style;
    let launch = crate::making::launch(arts, art);
    let style = opensacred_rules::arts::style(art);
    if matches!(
        style,
        Some(
            Style::Boon
                | Style::Frenzy
                | Style::Bolts
                | Style::Glance
                | Style::Changeling
                | Style::Dash
        )
    ) || matches!(launch, Some(2 | 5))
    {
        return Some((hero, (style == Some(Style::Dash)).then_some(run.ground)));
    }
    if launch == Some(3) {
        return Some((hero, Some(run.ground)));
    }
    let alive = |entity: Entity| {
        creatures
            .iter()
            .any(|&(e, _, hostile)| e == entity && hostile)
    };
    if let Some(target) = run.target.filter(|&target| alive(target)) {
        return Some((target, None));
    }
    let nearest = creatures
        .iter()
        .filter(|&&(_, _, hostile)| hostile)
        .min_by(|a, b| (a.1 - here).length().total_cmp(&(b.1 - here).length()))
        .map(|&(entity, ..)| entity);
    match nearest {
        Some(target) => Some((target, None)),
        // A spell of the launch type 0 admits the ground (`0x51D840`).
        None if art < 1000 && launch == Some(0) => Some((hero, Some(run.ground))),
        None => None,
    }
}

/// Gives the running combo's steps (`0x542B5B`, `0x4FE870`): the first at once, each next once
/// less than 0.3 of its step's clip is left to play, or once its action ended; a step without an
/// art passes, and the combo ends as its last step with an art is given. Each step is an order of
/// its art in the hero's form (`0x552370`) at its level (`0x56ABE0`), its readiness not asked.
/// The combo stops where the hero's order ends before its art played: its AI state no longer
/// coming on or making an art (6 or 11, `0x542C6A`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn steps(
    mut commands: Commands,
    mut run: ResMut<ComboRun>,
    (combos, arts, clips): (
        Option<Res<HeroCombos>>,
        Option<Res<HeroArts>>,
        Option<Res<crate::hero::Clips>>,
    ),
    players: Query<&AnimationPlayer>,
    heroes: Query<
        (
            Entity,
            &GlobalTransform,
            Has<Making>,
            Option<&Playing>,
            Option<&crate::effects::Effects>,
        ),
        (With<Body>, Without<Dies>),
    >,
    creatures: Query<(Entity, &GlobalTransform, &Mind, &Health), (Without<Dies>, Without<Body>)>,
) {
    let Some(mut current) = run.running else {
        return;
    };
    let (Some(combos), Some(arts), Ok((hero, at, making, playing, effects))) =
        (combos, arts, heroes.single())
    else {
        run.running = None;
        return;
    };
    let next = if !current.given {
        true
    } else if let Some(playing) = playing {
        current.seen = true;
        let player = clips
            .as_deref()
            .and_then(|clips| clips.player)
            .and_then(|player| players.get(player).ok());
        player
            .and_then(|player| playing.done(player))
            .is_none_or(|done| done >= NEXT_AT)
    } else if making {
        false
    } else if current.seen {
        true
    } else {
        info!("combos: the combo stops, the hero's order gone");
        run.running = None;
        return;
    };
    if !next {
        if run.running != Some(current) {
            run.running = Some(current);
        }
        return;
    }
    let Some(combo) = combos.0.get(current.index).copied() else {
        run.running = None;
        return;
    };
    let living: Vec<(Entity, Vec3, bool)> = creatures
        .iter()
        .filter(|(.., health)| health.current > 0)
        .map(|(entity, place, mind, _)| (entity, place.translation(), mind.hostile()))
        .collect();
    while current.step < STEPS {
        let step = current.step;
        current.step += 1;
        let Some((art, level)) = combo.steps[step] else {
            continue;
        };
        let art = opensacred_rules::arts::right_button(art, arts.kind, |key| {
            effects.is_some_and(|effects| effects.holds_any(&[key]))
        });
        let Some((target, point)) =
            target_of(&arts, art, (hero, at.translation()), &current, &living)
        else {
            info!("combos: the combo ends, nothing to make {art} on");
            run.running = None;
            return;
        };
        info!(
            "combos: the step {} makes {art} at level {level} on {target}",
            step + 1
        );
        commands
            .entity(hero)
            .remove::<(
                Making,
                Playing,
                crate::attack::Attack,
                crate::attack::Striking,
            )>()
            .insert(Making {
                art,
                target,
                point,
                level: Some(u32::from(level)),
            });
        current.given = true;
        current.seen = false;
        // The runner finds no art in the steps left: the combo ends as it is given.
        run.running = combo.steps[current.step..]
            .iter()
            .any(Option::is_some)
            .then_some(current);
        return;
    }
    run.running = None;
}

/// The arts' uses while a combo runs or its mark holds go to the combos (`0x562470`): the running
/// combo's regeneration from 0, every combo's less the art's time, and the mark set while a combo
/// runs; the mark goes once no combo runs and the hero stands idle (`0x542C91`).
fn uses(
    arts: Option<ResMut<HeroArts>>,
    combos: Option<ResMut<HeroCombos>>,
    mut run: ResMut<ComboRun>,
    heroes: Query<(Has<Making>, Has<Playing>), With<Body>>,
) {
    let (Some(mut arts), Some(mut combos)) = (arts, combos) else {
        return;
    };
    let running = run.running.map(|running| running.index);
    if arts
        .combo_uses
        .as_ref()
        .is_some_and(|uses| !uses.is_empty())
    {
        let taken = arts.combo_uses.replace(Vec::new()).unwrap_or_default();
        for time in taken {
            combos::used(&mut combos.0, running, time);
            if running.is_some() {
                run.mark = true;
            }
        }
    }
    let idle = heroes
        .iter()
        .next()
        .is_none_or(|(making, playing)| !making && !playing);
    if running.is_none() && run.mark && idle {
        run.mark = false;
    }
    let wanted = run.running.is_some() || run.mark;
    if wanted != arts.combo_uses.is_some() {
        arts.combo_uses = wanted.then(Vec::new);
    }
}

/// A new game's combos, empty, none running.
fn reset(mut commands: Commands) {
    commands.insert_resource(HeroCombos::default());
    commands.insert_resource(ComboRun::default());
}

/// The hero's combos.
pub struct CombosPlugin;

impl Plugin for CombosPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeroCombos>()
            .init_resource::<ComboRun>()
            .add_systems(OnEnter(AppState::InGame), reset)
            .add_systems(
                Update,
                (
                    order
                        .in_set(crate::frame::Control)
                        .before(crate::hero::steer),
                    (uses, steps)
                        .chain()
                        .after(crate::making::Made)
                        .before(crate::hero::take_orders),
                )
                    .in_set(crate::frame::Unpaused)
                    .in_set(crate::frame::Step::HeroMove)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                regenerate
                    .in_set(crate::frame::Unpaused)
                    .after(crate::frame::Step::Tick)
                    .run_if(crate::frame::ticked),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_goes_to_its_creature_else_the_nearest_hostile_one() {
        let arts = HeroArts::default();
        let entity = |bits: u32| Entity::from_raw_u32(bits).unwrap();
        let (hero, foe, far, friend) = (entity(1), entity(2), entity(3), entity(4));
        let mut run = Run {
            index: 0,
            step: 0,
            target: Some(foe),
            ground: Vec3::new(5.0, 0.0, 5.0),
            given: false,
            seen: false,
        };
        let living = [
            (foe, Vec3::new(30.0, 0.0, 0.0), true),
            (far, Vec3::new(90.0, 0.0, 0.0), true),
            (friend, Vec3::new(10.0, 0.0, 0.0), false),
        ];
        let here = (hero, Vec3::ZERO);
        // The Seraphim's 1071 on the combo's creature; a boon (the Dwarf's frenzy) on the hero.
        assert_eq!(
            target_of(&arts, 1071, here, &run, &living),
            Some((foe, None))
        );
        assert_eq!(
            target_of(&arts, 1082, here, &run, &living),
            Some((hero, None))
        );
        // Its creature dead: the nearest hostile one, never a friend.
        run.target = Some(entity(9));
        assert_eq!(
            target_of(&arts, 1071, here, &run, &living),
            Some((foe, None))
        );
        assert_eq!(target_of(&arts, 1071, here, &run, &living[2..]), None);
    }

    #[test]
    fn each_combo_has_its_three_images() {
        assert_eq!(sprites(0x200), Some([801, 802, 803]));
        assert_eq!(sprites(0x203), Some([810, 811, 812]));
        assert_eq!(sprites(1071), None);
        assert!(is_combo(0x201) && !is_combo(0x204) && !is_combo(13));
    }

    #[test]
    fn the_console_names_a_combos_steps() {
        let words = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            parse_steps(&words("1070:3 1071:2 20:1")),
            Ok([Some((1070, 3)), Some((1071, 2)), Some((20, 1)), None])
        );
        assert_eq!(parse_steps(&[]), Ok([None; STEPS]));
        assert!(parse_steps(&words("1070:0")).is_err());
        assert!(parse_steps(&words("1070")).is_err());
        assert!(parse_steps(&words("1:1 2:1 3:1 4:1 5:1")).is_err());
    }
}
