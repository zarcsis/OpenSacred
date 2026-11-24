//! The hero makes its combat arts (#509; docs/re/arts.md, "An art in a strike", "The arts'
//! clips"): a right click on a hostile creature makes the selected art on it, as the original's
//! does, and the console's `arts make` on the nearest. The order (`0x4FD0F0`) needs the art
//! ready and of a style whose action is read; the hero comes on to within 10 of the reach of the
//! two creatures, stops, turns to the target and plays its class's clip of the art for what it
//! holds (`opensacred_rules::arts::action`) at its attack's speed; the strikes are the rules'
//! (`arts::attacke_target`, `arts::rundum_target`, `fight::art_strike`), the first starting the
//! regeneration of every art of its kind (`arts::HeroArts::start`). A spell is cast where the
//! hero stands with the clip of its slot at its casts' speed, and acts once the share of its clip
//! still to play falls below its own (`opensacred_rules::spells`, #872): its regeneration starts
//! and `spells` carries out what it does.
//!
//! The special weapon arts (docs/re/arts.md, "The special weapon arts") are carried out in the
//! arms of `make`, by their styles (`opensacred_rules::arts::Style`), and the right button aims
//! them by their launch types (`aim`, `launch`). Here too are the hurled blades' strikes
//! (`blades`), the jumps' flights (`Leap`), the knockbacks and the throws (`Knocked`), and the
//! point of a model's bone but its worn items' (`bone`, `Worn`), which the effects' modules read.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use opensacred_data::tables::WeaponArt;
use opensacred_rules::arts::{
    self, ArtClip, Candidate, Creature, SaddleRate, Style, attacke_target, due, hardhit_point,
    held_factor, made_from, rundum_radius, rundum_target,
};
use opensacred_rules::particles::DarkSightKind;
use opensacred_rules::{minds, spells};

use crate::ai::Mind;
use crate::arts::HeroArts;
use crate::fight::{self, Combat, Experience, Health};
use crate::frame::{Control, Step, Unpaused, WorldTick};
use crate::hero::{self, ArtClipName, Body, Clips, GameSpeed};
use crate::hit_sparks::Spark;
use crate::motions::Slots;
use crate::npcs::Dies;
use crate::spawning::Rng;

/// The blend into an art's clip (`0x593320`'s 0.5 over the attack speed, at most 0.2 s;
/// **hypothesis**: seconds).
const BLEND: Duration = Duration::from_millis(200);
/// The bones a `Rundum` strikes about: a weapon's effects' bone, else the right hand
/// (`0x44CA30`).
const WEAPON_BONES: [&str; 2] = ["weapon_fx01", "Weapon_fx01"];
const HAND_BONE: &str = "Bip01 R Hand";
/// The domination's object's loop (857, `cParticleSystem` at `0x79DBF0`'s start):
/// `VAMP_BEHERRSCHUNG_LOOP`.
const DOMINATION_LOOP: u32 = 2075;
/// The raising's object's sound as it starts (866, `0x79E5D0`): `VAMP_PENTA`.
const RAISE_SOUND: u32 = 2082;
/// The soulcatcher's object's loop (869 `FX_SOULCATCHER`, `0x76AA40`).
const SOULCATCHER_LOOP: u32 = 2068;
/// The Dark Elf's testosterone and adrenaline, whose two objects 864 (`dark_sm_hit`) loop
/// `DELF_ADRENALIN_LOOP` from their starts (`0x79503D`).
const ADRENALINE: [u16; 2] = [1047, 1048];
const ADRENALINE_LOOP: u32 = 2053;
/// The Dwarf's frenzy, whose object 1019 (`kriegsrausch`) loops `SERA_LICHTSCHILD_LOOP` from its
/// start (`0x7B65F1`), with its effect 47, fading in over a second, holding for the effect's time
/// and fading out over a second (`0x7B5F30`).
const FRENZY: u16 = 1082;
const FRENZY_EFFECT: u32 = 47;
const FRENZY_LOOP: u32 = 2040;

/// The art that the right button makes: the original's right button's slot, at a new game the
/// hero's first art whose action is read.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selected(pub Option<u16>);

/// The hero's order to make an art on a creature, or at a point on the ground (`0x4FD0F0`).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Making {
    pub art: u16,
    pub target: Entity,
    /// The point on the ground an art of the launch type 3 is aimed at.
    pub point: Option<Vec3>,
    /// The level of a combo's step, which its executor takes instead of the art's own
    /// (`0x56ABE0`, `combos`).
    pub level: Option<u32>,
}

/// An art's action under way: its style, its clip's node and length, its hit points and how many
/// strikes it has sent, its target, the creature a `Rundum` struck last, and its strikes' level
/// and factor.
#[derive(Component, Debug, Clone)]
pub struct Playing {
    style: Style,
    node: AnimationNodeIndex,
    length: f32,
    /// How long its clip plays, in seconds, at its speed.
    seconds: f32,
    /// Its target's place as it started, a jump's point where its target is gone.
    point: Vec3,
    points: Vec<f32>,
    struck: usize,
    /// How many of its hit points a `Rundum` of a gun has fired at (#1963).
    fired: usize,
    /// The creatures that the Dwarf's `Rundum` with a gun gathered before its clip, which its
    /// second to fourth shots take in turn (`arts::gun_rundum_gathers`, #3122).
    gathered: Vec<Entity>,
    /// Whether the Dwarf's `Rundum` has started its regeneration (#3011).
    regenerated: bool,
    target: Entity,
    last: Option<Entity>,
    level: u32,
    factor: f32,
}

impl Playing {
    /// Its target.
    pub fn target(&self) -> Entity {
        self.target
    }

    /// The rate its clip plays at, its length over its seconds (the original's `+0x3E6` over 100).
    pub fn rate(&self) -> f32 {
        self.length / self.seconds.max(f32::EPSILON)
    }

    /// The share of its clip played on `player`, none where the clip is gone.
    pub fn done(&self, player: &AnimationPlayer) -> Option<f32> {
        player
            .animation(self.node)
            .map(|active| active.seek_time() / self.length.max(f32::EPSILON))
    }
}

/// The hero's making of its arts, which the combos' steps follow (`combos`).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Made;

/// The dice of the hero's arts (the `Hardhit` of an attack's clip at random).
#[derive(Resource)]
struct Dice(Rng);

/// The clips of the arts' actions of a hero of type `kind` holding what is of `class`, by the
/// slots of its model, `slots`, and the hit points of the motions, `points` (`npcs::hit_points`).
pub fn clips(
    kind: u32,
    class: u8,
    slots: Option<&Slots>,
    points: &HashMap<String, Vec<f32>>,
) -> Vec<ArtClipName> {
    [
        Style::Attacke,
        Style::Rundum,
        Style::Hardhit,
        Style::Kick,
        Style::Boon,
        Style::Frenzy,
        Style::Bolts,
        Style::Glance,
        Style::Charge,
        Style::Ring,
        Style::Fan,
        Style::Flamer,
        Style::Mortar,
        Style::Cannon,
        Style::Breath,
        Style::Sweep,
        Style::Hurl,
        Style::Cast(154),
        Style::Cast(155),
        Style::Cast(156),
        Style::Cast(157),
        Style::Cast(158),
        Style::Cast(159),
        Style::Cast(160),
        Style::Cast(161),
        Style::Cast(162),
        Style::Cast(163),
        Style::Cast(164),
        // The riders' clips of the spells and, with the slots 169 and 194, of the arts from the
        // saddle (`arts::saddle_clip`, #3134).
        Style::Cast(169),
        Style::Cast(194),
        Style::Cast(197),
        Style::Cast(199),
        Style::Smite,
        Style::Blades,
        Style::Throw,
        Style::Whirl,
        Style::Jump,
        Style::Roar,
        Style::Viper,
        Style::Ward,
        Style::WardSpells,
        Style::Trap,
        Style::Form,
        Style::Panther,
        Style::Changeling,
        Style::Dash,
        Style::Backbreaker,
        Style::Bite,
        Style::Dominate,
        Style::Summon,
        Style::Raise,
        Style::MasterBite,
        Style::Bats,
        Style::Kiss,
    ]
    .into_iter()
    .filter_map(|style| {
        let action = arts::action(kind, style, class)?;
        // The slot's clip, else the one it falls back to (the Dwarf's).
        let clip = match action.clip {
            ArtClip::Slot(slot) => Some(
                slots?
                    .slot(slot)
                    .or_else(|| slots?.slot(action.fallback?))?
                    .to_owned(),
            ),
            ArtClip::Attack => None,
        };
        let floats = clip
            .as_ref()
            .and_then(|name| points.get(&name.to_ascii_uppercase()))
            .cloned()
            .unwrap_or_default();
        // A kick strikes at its class's point of its clip, not at the clip's hit points, and the
        // backbreaker at its own by what the hero holds.
        let points = if style == Style::Kick {
            arts::kick_point(kind, class).into_iter().collect()
        } else if style == Style::Backbreaker {
            vec![1.0 - arts::backbreaker_lets_go(class)]
        } else {
            arts::hit_points(&floats)
        };
        Some(ArtClipName {
            style,
            clip,
            points,
            rate: action.rate,
            fixed: action.fixed,
            halved: class == 8,
        })
    })
    .collect()
}

/// What a boon of `record` gives at the level in play `level` ([`Style::Boon`] and its kin): the
/// effects of `arts::boon` (1094's time `flight` times as long, set 59's), `ward`, `form` or
/// `glance` (by the factor `glance`), or the Dark Elf's 1044's by the first table's record 47
/// (`arts::soulcatcher`).
fn boon_of(
    arts: &HeroArts,
    record: &WeaponArt,
    level: u32,
    (glance, flight): (f64, f32),
) -> Option<arts::Boon> {
    arts::boon(record, level, flight)
        .or_else(|| arts::ward(record, level))
        .or_else(|| arts::form(record, level))
        .or_else(|| arts::glance(record, level, glance))
        .or_else(|| {
            let spell = arts.tables.spells.iter().find(|spell| spell.id == 47)?;
            // Its shown time, its time over the weapon arts' speed (`0x5828E0`).
            let speed = arts.speeds.weapon_arts;
            let time = arts.arts.time_of(record.id, &arts.tables);
            let shown = if speed > 0.0 { time / speed } else { time };
            (record.id == 1044).then(|| arts::soulcatcher(spell, level, shown))
        })
}

/// The launch type of `art`, a weapon art's or a spell's (`0x51D840`): 1 on an enemy, 2 on a
/// friend, 3 on the ground, 4 on the dead, 5 on itself, 0 any.
pub fn launch(arts: &HeroArts, art: u16) -> Option<u8> {
    let weapon = arts.tables.weapon_arts.iter().find(|a| a.id == art);
    weapon.map(|a| a.launch).or_else(|| {
        let spell = arts.tables.spells.iter().find(|s| s.id == art)?;
        u8::try_from(spell.launch).ok()
    })
}

/// The hero's first art whose action is read, which a new game's first art slot holds (#678,
/// `art_slots`).
pub(crate) fn first_art(arts: &HeroArts) -> Option<u16> {
    arts.arts
        .entries
        .iter()
        .find(|e| arts::style(e.art).is_some())
        .map(|e| e.art)
}

/// Whether the hero may make `art` now: it holds it, it is ready, its action is read, no effect
/// holds on it that refuses it (`effects`, `arts::refused_while`), the Vampiress's form is the
/// art's (`arts::in_form`), and it wears the cannon that its art wants (`pack`'s slot 14,
/// `arts::wants_cannon`; `0x51D840`); riding (`riding`, with a bow or a crossbow in hand or not),
/// an art that may be made on horseback alone (`HeroArts::on_horseback`; the others go to the
/// horse, whose trample they start, `trample`).
pub fn may_make(
    arts: &HeroArts,
    art: u16,
    (effects, pack, riding): (
        Option<&crate::effects::Effects>,
        Option<&crate::pickup::Pack>,
        Option<bool>,
    ),
) -> Result<(), String> {
    if arts.arts.get(art).is_none() {
        return Err(format!("the hero holds no art {art}"));
    }
    if !arts.arts.ready(art) {
        return Err(format!("{art} is not ready"));
    }
    if arts::style(art).is_none() {
        return Err(format!("{art}'s action is not read yet"));
    }
    if effects.is_some_and(|effects| effects.holds_any(arts::refused_while(art))) {
        return Err(format!("{art}'s effects still hold"));
    }
    if !arts::in_form(art, arts.kind) {
        return Err(format!("{art} is not made in this form"));
    }
    let cannon = pack
        .and_then(|pack| pack.worn.get(usize::from(arts::CANNON_SLOT)))
        .is_some_and(Option::is_some);
    if arts::wants_cannon(art) && !cannon {
        return Err(format!("{art} wants a cannon worn"));
    }
    if let Some(bow) = riding
        && !arts.on_horseback(art, bow)
    {
        return Err(format!("{art} is not made on horseback"));
    }
    Ok(())
}

/// Whether the mortar 1086 may be made at `point` on the level `on`, made from `here` on the level
/// `level` (`0x51D840`, its case of 1086): the point on the maker's floor (`0x6354D0` against
/// the maker's `+0x2C`, `0x449930`), and where the maker stands in a dungeon's sector (`dungeon`;
/// `0x63D260`, `0x622E20`) a straight way of cells that `walkable` takes to it (`0x4FCC80`);
/// outside one the shell flies over whatever lies between.
pub(crate) fn mortar_reaches(
    (here, level): (Vec3, crate::terrain::Level),
    (point, on): (Vec3, crate::terrain::Level),
    dungeon: bool,
    walkable: impl Fn(crate::paths::Cell) -> bool,
) -> bool {
    level == on && (!dungeon || crate::paths::straight(here.xz(), point.xz(), walkable))
}

/// Whether an art of the launch type 1 may be made at the enemy standing at `target`, made from
/// `here` on the level `level` (`0x51D840`, its case 1): a line of sight to it, the straight way's
/// test with the flyer's flag (`0x51DD7B`), else a way to its place (`0x4F7710` at `0x51DDC5`;
/// docs/re/arts.md, "The special weapon arts", #3361).
pub(crate) fn enemy_reached(
    (here, level): (Vec3, crate::terrain::Level),
    target: Vec3,
    heights: &crate::terrain::Heights,
) -> bool {
    crate::paths::straight(here.xz(), target.xz(), |cell| {
        heights.flown_onto(cell, level)
    }) || crate::ai::way_to(
        here,
        level,
        opensacred_rules::minds::Spot::new(target.x, target.z),
        heights,
    )
}

/// Whether an art of the launch type 3 may be made at `point` on the level `on`, made from `here`
/// on the level `level` (`0x51D840`, its case 3): the point on the maker's floor (`0x6354D0`
/// against `0x449930`, asked where either stands above the ground), a straight line of cells that
/// `flown` takes to it, the test with the flyer's flag, over the water and the fences
/// (`0x4FCC80`, #3358), and a cell there that `stands` takes (`0x636890`); a jump's and the
/// Daemon's charge's start too, on the maker's own floor (`0x505B6E`, `0x51B65D`).
pub(crate) fn point_reached(
    (here, level): (Vec3, crate::terrain::Level),
    (point, on): (Vec3, crate::terrain::Level),
    (flown, stands): (
        impl Fn(crate::paths::Cell) -> bool,
        impl Fn(crate::paths::Cell) -> bool,
    ),
) -> bool {
    level == on
        && crate::paths::straight(here.xz(), point.xz(), &flown)
        && stands(crate::paths::cell_of(point.x, point.z))
}

/// On a press of the right button on a hostile creature alive that the cursor picks
/// (`under_cursor`), the hero makes the selected art on it (a launch on an enemy, the type 1), its
/// attack ending; with the art not ready it walks to the creature's place instead, as the
/// original's handler of the order takes it (about `0x52FE40`, docs/re/arts.md, "The right
/// button", #654), and with Ctrl held does nothing, the walk to himself that the original's then
/// posts doing nothing (`0x52FE9D`, seen, #3434). A press of the left button ends an order not yet
/// under way.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
pub(crate) fn aim(
    mut commands: Commands,
    (buttons, keys): (Res<ButtonInput<MouseButton>>, crate::controls::Keys),
    (pointer, under, heights): (
        Res<crate::hud::Pointer>,
        Res<crate::under_cursor::UnderCursor>,
        Option<Res<crate::terrain::Heights>>,
    ),
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    (selected, arts, pack, mut tramples): (
        Res<Selected>,
        Option<Res<HeroArts>>,
        Option<Res<crate::pickup::Pack>>,
        MessageWriter<crate::trample::Start>,
    ),
    (creatures, corpses, ranks, experience, followers): (
        Query<(Entity, &GlobalTransform, &Mind, &Health), Without<Dies>>,
        Query<(Entity, &GlobalTransform), With<crate::npcs::Corpse>>,
        Query<&fight::Ranked>,
        Option<Res<fight::Experience>>,
        Query<(&crate::followers::Follower, Option<&crate::crowd::InTheWay>)>,
    ),
    mut heroes: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Body,
            Has<Playing>,
            Option<&crate::effects::Effects>,
            Option<&crate::riding::Seat>,
            Option<&fight::Holding>,
        ),
        (
            Without<Dies>,
            Without<fight::Downed>,
            Without<fight::Pushed>,
        ),
    >,
) {
    if buttons.just_pressed(MouseButton::Left) {
        for (hero, _, _, playing, ..) in &heroes {
            if !playing {
                commands.entity(hero).remove::<Making>();
            }
        }
    }
    if !buttons.just_pressed(MouseButton::Right) {
        return;
    }
    let (Some(cursor), Some(art), Some(arts)) = (pointer.0, selected.0, arts) else {
        return;
    };
    // A combo on the right button is the combos' to run (`combos`, #1531).
    if crate::combos::is_combo(art) {
        return;
    }
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let under = under.0;
    let picked = under.and_then(|entity| {
        let (_, at, mind, health) = creatures.get(entity).ok()?;
        (mind.hostile() && health.current > 0).then(|| (entity, at.translation()))
    });
    // The dead creature under the cursor, an art of the launch type 4's (`0x51D840`, case 4).
    let dead = under
        .and_then(|entity| corpses.get(entity).ok())
        .map(|(entity, at)| (entity, at.translation()));
    let selected = art;
    for (hero, at, mut body, playing, effects, seat, holding) in &mut heroes {
        if playing {
            continue;
        }
        // The art of the hero's form for the selected one (`0x552370`).
        let art = arts::right_button(selected, arts.kind, |key| {
            effects.is_some_and(|effects| effects.holds_any(&[key]))
        });
        let launch = launch(&arts, art);
        let spell = art < 1000;
        // The right button on the hero himself, picked, makes him the target (seen: the
        // Gladiator's 1015, of the launch type 2, made on himself so, and refused on a hostile
        // creature or on the ground).
        let on_hero = under == Some(hero);
        // Where the cursor's line meets the hero's height: the point of an art of the launch
        // type 3 (`0x51D840`'s action's point).
        let along = (at.translation().y - ray.origin.y) / ray.direction.y;
        let ground = ray.origin + ray.direction.as_vec3() * along;
        let (target, place) = match (launch, on_hero, picked) {
            (Some(3), ..) => (hero, ground),
            (Some(4), ..) => match dead {
                Some((corpse, place)) => (corpse, place),
                None => {
                    info!("making: {art} wants a dead creature under the cursor");
                    continue;
                }
            },
            // A spell of the launch type 5 takes its caster for its target, whatever the cursor
            // is on (`0x51D840`).
            (Some(5), ..) if spell => (hero, at.translation()),
            (Some(2), true, _) => (hero, at.translation()),
            // A follower of his alive on his floor, his friend (`0x5490B0`, #3124), but for the
            // Daemon's forms, which are his alone.
            (Some(2), false, _)
                if !arts::forms(art)
                    && let Some(friend) = under.and_then(|entity| {
                        let (_, place, _, health) = creatures.get(entity).ok()?;
                        let (follower, way) = followers.get(entity).ok()?;
                        let on_floor = way.is_none_or(|way| way.level == body.level());
                        (follower.master == hero && health.current > 0 && on_floor)
                            .then(|| (entity, place.translation()))
                    }) =>
            {
                friend
            }
            (Some(2), false, _) => {
                info!("making: {art} wants a friend, or the hero himself, under the cursor");
                continue;
            }
            (_, true, _)
                if matches!(
                    arts::style(art),
                    Some(Style::Boon | Style::Frenzy | Style::Bolts | Style::Glance)
                ) =>
            {
                (hero, at.translation())
            }
            // The changeling is made where the hero stands, wherever the button (seen), and the
            // dash at the ground under the cursor.
            _ if arts::style(art) == Some(Style::Changeling) => (hero, at.translation()),
            _ if arts::style(art) == Some(Style::Dash) => (hero, ground),
            (_, _, Some((entity, place))) => (entity, place),
            // A spell of the launch type 0 admits the ground under the cursor (`0x51D840`).
            (Some(0), _, None) if spell => (hero, ground),
            _ => continue,
        };
        let riding = crate::riding::riding_with(seat, holding);
        // From the saddle an art that may not be made on horseback reaches the horse, whose
        // trample it starts where it may (`0x52FEEB`, `trample`).
        if let Some(bow) = riding
            && arts.arts.get(art).is_some()
            && !arts.on_horseback(art, bow)
        {
            tramples.write(crate::trample::Start);
            continue;
        }
        // An art on an enemy wants a line of sight or a way to it (`0x51DD7B`, `0x51DDC5`,
        // #3361).
        if launch == Some(1)
            && target != hero
            && heights.as_deref().is_some_and(|heights| {
                !enemy_reached((at.translation(), body.level()), place, heights)
            })
        {
            info!("making: {art} refused at {target}, no line nor way to it");
            continue;
        }
        // The stone, the cage, 19 and 36 are refused at a creature above their bound of levels
        // (`0x51D840` at `0x51D90D`, #3239).
        let hero_level = experience
            .as_deref()
            .map_or(1, |experience| experience.level);
        if spell
            && launch == Some(1)
            && target != hero
            && let Ok((_, _, mind, _)) = creatures.get(target)
            && let Some(level) = arts.arts.level_in_play(
                art,
                Creature {
                    kind: arts.kind,
                    level: hero_level,
                    player: true,
                },
            )
        {
            let boss = ranks
                .get(target)
                .is_ok_and(|rank| rank.0 == fight::Rank::Boss);
            if !opensacred_rules::spells::level_admits(art, (mind.level, boss), (hero_level, level))
            {
                info!("making: {art} refused at {target}, of a level too high");
                continue;
            }
        }
        match may_make(&arts, art, (effects, pack.as_deref(), riding)) {
            Ok(()) => {
                // A boon's target is its maker (the launch type 5, `0x51D840`).
                let target = if matches!(
                    arts::style(art),
                    Some(
                        Style::Boon
                            | Style::Frenzy
                            | Style::Bolts
                            | Style::Glance
                            | Style::Changeling
                            | Style::Dash
                    )
                ) {
                    hero
                } else {
                    target
                };
                info!("making: the hero makes {art} on {target}");
                commands
                    .entity(hero)
                    .remove::<(crate::attack::Attack, crate::attack::Striking)>()
                    .insert(Making {
                        art,
                        target,
                        point: (launch == Some(3)
                            || arts::style(art) == Some(Style::Dash)
                            || (spell && target == hero))
                            .then_some(ground),
                        level: None,
                    });
            }
            Err(_)
                if arts.arts.get(art).is_some()
                    && !arts.arts.ready(art)
                    && keys.pressed(crate::controls::Action::StrikeAtOnce) =>
            {
                info!("making: {art} is not ready: under Ctrl the hero does nothing");
            }
            Err(_) if arts.arts.get(art).is_some() && !arts.arts.ready(art) => {
                info!("making: {art} is not ready: the hero walks to {target}");
                commands
                    .entity(hero)
                    .remove::<(crate::attack::Attack, crate::attack::Striking, Making)>();
                body.send(place);
            }
            Err(why) => info!("making: {why}"),
        }
    }
}

/// The worn items' models (`npcs::WornBy`), whose nodes of their wearers' bones' names keep their
/// rest pose under their wearers, their skinned meshes following the wearers' bones (#2030).
pub(crate) type Worn<'w, 's> = Query<'w, 's, (), With<crate::npcs::WornBy>>;

/// The world point of the first of `names` among `root`'s descendants, but its worn items'
/// ([`Worn`]).
pub(crate) fn bone(
    root: Entity,
    names: &[&str],
    children: &Query<&Children>,
    (bones, worn): (&Query<(&Name, &GlobalTransform)>, &Worn),
) -> Option<Vec3> {
    bone_entity(root, names, children, (bones, worn))
        .and_then(|bone| bones.get(bone).ok())
        .map(|(_, at)| at.translation())
}

/// The first of `names` among `root`'s descendants, but its worn items' ([`Worn`]).
pub(crate) fn bone_entity(
    root: Entity,
    names: &[&str],
    children: &Query<&Children>,
    (bones, worn): (&Query<(&Name, &GlobalTransform)>, &Worn),
) -> Option<Entity> {
    let mut found: Option<(usize, Entity)> = None;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if entity != root && worn.contains(entity) {
            continue;
        }
        if let Ok((name, _)) = bones.get(entity)
            && let Some(rank) = names.iter().position(|n| *n == name.as_str())
            && found.is_none_or(|(best, _)| rank < best)
        {
            found = Some((rank, entity));
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    found.map(|(_, bone)| bone)
}

/// The distance of `point` from the segment from `from` to `to` (`0x6226B0`).
fn segment_distance(point: Vec2, from: Vec2, to: Vec2) -> f32 {
    let line = to - from;
    let along = if line.length_squared() > f32::EPSILON {
        ((point - from).dot(line) / line.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (from + line * along - point).length()
}

/// A whole distance on the ground, as the original's integer square root gives it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a distance on the ground, whole and positive, far below the integers' range"
)]
pub(crate) fn whole_distance(a: Vec3, b: Vec3) -> u32 {
    (a - b).xz().length().round() as u32
}

/// The hero's order (`0x4FD0F0`) and its art's action (the executors, `0x4FF0B0`, `0x5038F0`):
/// it comes on to within 10 of the reach of the two creatures, or makes it where it stands for
/// an art of the distance 0, then plays its art's clip and strikes: an `Attacke` at each of its
/// clip's hit points, on its target or the next hostile creature within the reach and 45, and
/// ends with none or after its last; a `Rundum`, the Gladiator's from 0.3 of its clip, about its
/// weapon's or its hand's bone, on the nearest creature within its radius but the one it struck
/// last, until its clip ends; a `Hardhit` once at its hit point, on its target. The Dwarf's three
/// with a gun are made where he stands, a pistol's by a line of sight, his `Attacke` striking its
/// one target at any distance and his `Rundum` its target and those it gathered in turn, a shot at
/// each hit point (#3122). The first strike, or a `Rundum`'s start, starts the regeneration. The
/// spells' casts and the special weapon arts are carried out in their styles' arms, told there.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::too_many_lines,
    reason = "a Bevy system takes its resources and queries as arguments; the order and its three \
              styles read best together"
)]
fn make(
    mut commands: Commands,
    (tick, speed, buttons, clock, rules): (
        Res<WorldTick>,
        Res<GameSpeed>,
        Res<ButtonInput<MouseButton>>,
        Option<Res<crate::daynight::Clock>>,
        Res<fight::Rules>,
    ),
    experience: Res<Experience>,
    clips: Option<Res<Clips>>,
    mut arts: Option<ResMut<HeroArts>>,
    mut dice: ResMut<Dice>,
    (animations, factors): (Res<Assets<AnimationClip>>, Res<crate::motions::Factors>),
    mut heroes: Query<
        (
            Entity,
            &mut Transform,
            &mut Body,
            &Making,
            Option<&mut Playing>,
            Option<&Combat>,
            Option<&hero::Size>,
            Option<&fight::Holding>,
            Option<&fight::Frenzy>,
            Option<&crate::riding::Seat>,
            Option<&fight::Base>,
        ),
        (
            Without<Dies>,
            Without<fight::Downed>,
            Without<fight::Pushed>,
        ),
    >,
    creatures: Query<(Entity, &GlobalTransform, &Mind, &Health), (Without<Dies>, Without<Body>)>,
    children: Query<&Children>,
    bones: Query<(&Name, &GlobalTransform)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    (mut hits, mut cries, ranks, leaps, sizes, mut launches, mut acts, corpses, ways, mut points): (
        MessageWriter<fight::Hit>,
        MessageWriter<crate::voices::BattleCry>,
        Query<&fight::Ranked>,
        Query<(), With<Leap>>,
        Query<&crate::arrows::Size>,
        MessageWriter<crate::bolts::Launch>,
        MessageWriter<crate::spells::Acts>,
        Query<(&GlobalTransform, &crate::npcs::Corpse)>,
        Query<&crate::crowd::InTheWay>,
        MessageWriter<crate::sfx::PlayAt>,
    ),
    (nearest, heights, mut fires, mut sparks, worn, mut rings, mut impacts, dungeons, wielded): (
        Res<crate::crowd::Nearest>,
        Res<crate::terrain::Heights>,
        MessageWriter<crate::sfx::Gunfire>,
        MessageWriter<Spark>,
        Worn,
        MessageWriter<crate::poison_rings::Made>,
        MessageWriter<crate::gun_fires::Impact>,
        Option<Res<crate::daynight::Dungeons>>,
        Query<(Entity, &crate::npcs::Wielded, &crate::models::OrderedWith)>,
    ),
    places: Query<&GlobalTransform>,
) {
    let (Some(clips), Some(arts)) = (clips, arts.as_mut()) else {
        return;
    };
    let by_day = clock.is_none_or(|clock| clock.phase() == crate::daynight::Phase::Day);
    for (
        hero,
        mut transform,
        mut body,
        making,
        playing,
        combat,
        size,
        holding,
        frenzy,
        seat,
        base,
    ) in &mut heroes
    {
        // From the saddle its spells take the rider's clips, its `Hardhit` the rider's 194, and
        // it does not turn to its target (#2302).
        let riding = seat.is_some_and(|seat| seat.ridden().is_some());
        // How many times as long set 59 makes the Daemon's 1094 last (#3136).
        let flight_times = holding.map_or(1.0, |holding| holding.0.sets.flight_times());
        // The reach of the hero and a creature, by their sizes (`0x4DBC60`, #642).
        let size = size.map_or(minds::SIZE, |size| size.0);
        let reach = |mind: &Mind| minds::reach(size, mind.size());
        let Some((mut player, mut transitions)) =
            clips.player.and_then(|player| players.get_mut(player).ok())
        else {
            continue;
        };
        let here = transform.translation;
        let end = |commands: &mut Commands| {
            commands.entity(hero).remove::<(Making, Playing)>();
        };
        let Some(mut playing) = playing else {
            // An art made at a point wants it on the hero's floor, a straight way to it and a
            // cell there he may stand on, else it ends without its regeneration, the hero
            // standing (`0x51D840`, its case 3).
            if launch(arts, making.art) == Some(3)
                && let Some(point) = making.point
            {
                let level = body.level();
                let walkable = |cell| heights.walkable_on(cell, level);
                let flown = |cell| heights.flown_onto(cell, level);
                if !point_reached((here, level), (point, level), (flown, walkable)) {
                    info!(
                        "making: no straight way leads to {}'s point, or it may not be stood on",
                        making.art
                    );
                    body.stop();
                    end(&mut commands);
                    continue;
                }
            }
            if let Some(Style::Cast(slot)) = arts::style(making.art) {
                // A rider's cast plays its own slot (`0x522030`).
                let slot = if riding {
                    spells::riding_slot(making.art)
                } else {
                    slot
                };
                // A spell is cast where the hero stands (its order's reach not read), the
                // fireball turning it to its target's place, or its point (`0x51E110`), with
                // the clip of its slot, blended in over 0.2, at its casts' speed (`+0x3E`).
                body.stop();
                let point = making.point.unwrap_or_else(|| {
                    creatures
                        .get(making.target)
                        .map_or(here, |(_, at, ..)| at.translation())
                });
                let toward = (point - here).with_y(0.0);
                if !riding && spells::turns(making.art) && toward.length() > f32::EPSILON {
                    transform.rotation = hero::facing(toward);
                }
                let level = making.level.or_else(|| {
                    arts.arts.level_in_play(
                        making.art,
                        Creature {
                            kind: arts.kind,
                            level: experience.level,
                            player: true,
                        },
                    )
                });
                let action = clips
                    .arts
                    .iter()
                    .find(|a| a.style == Style::Cast(slot) && a.node.is_some())
                    .or_else(|| {
                        let mut arts = clips.arts.iter();
                        arts.find(|a| a.style == Style::Cast(154) && a.node.is_some())
                    });
                let (Some(level), Some(share), Some(action)) =
                    (level, spells::share(making.art), action)
                else {
                    info!("making: the hero has no cast of {} read", making.art);
                    end(&mut commands);
                    continue;
                };
                let (Some(node), Some(clip)) = (action.node, &action.clip) else {
                    end(&mut commands);
                    continue;
                };
                let length = animations
                    .get(clip)
                    .map_or(1.0, AnimationClip::duration)
                    .max(0.01);
                let cast = combat.map_or(opensacred_rules::fight::CAST_SPEED, |combat| combat.cast);
                // Times its motion's own factor (`0x413610`, #3567).
                let played = speed.0 * cast / 100.0 * factors.of(clip);
                transitions.play(&mut player, node, BLEND).set_speed(played);
                info!(
                    "making: the hero casts {} at level {level} on {}",
                    making.art, making.target
                );
                commands.entity(hero).insert(Playing {
                    style: Style::Cast(slot),
                    node,
                    length,
                    seconds: length / played.max(f32::EPSILON),
                    point,
                    points: vec![1.0 - share],
                    struck: 0,
                    fired: 0,
                    gathered: Vec::new(),
                    regenerated: false,
                    target: making.target,
                    last: None,
                    level,
                    factor: 1.0,
                });
                continue;
            }
            let Some(record) = arts.tables.weapon_arts.iter().find(|a| a.id == making.art) else {
                end(&mut commands);
                continue;
            };
            // The awakening takes a dead creature that it may raise within 400, with a straight
            // way to it, else it ends without its regeneration (`0x513868`).
            let raises = arts::style(making.art) == Some(Style::Raise);
            // A class's own `Rundum`, `Hardhit` or `Attacke` with a bow or a crossbow in hand, not
            // a gun (#2238, #2275): the art whose shot it makes.
            let bow = [
                (arts::bow_rundum as fn(u16) -> bool, arts::BOW_RUNDUM),
                (arts::bow_hardhit, arts::BOW_HARDHIT),
                (arts::bow_attacke, arts::BOW_ATTACKE),
            ]
            .into_iter()
            .find(|(own, _)| own(making.art))
            .map(|(_, shot)| shot)
            .filter(|_| holding.is_some_and(|holding| holding.0.ranged && !holding.0.gun));
            // The class of the gun in hand of one of the Dwarf's own three (#3122).
            let gun = holding
                .filter(|holding| holding.0.gun && arts::dwarf_gun_art(making.art))
                .map(|holding| holding.0.class);
            if raises {
                let level = body.level();
                let takes = corpses.get(making.target).is_ok_and(|(at, corpse)| {
                    let at = at.translation();
                    arts::raises(corpse.class, corpse.creature, corpse.dialogue, false)
                        && (at - here).xz().length() <= arts::RAISES_WITHIN
                        && crate::paths::clear(here.xz(), at.xz(), |cell| {
                            heights.walkable_on(cell, level)
                        })
                });
                if !takes {
                    info!("making: {} finds no dead creature to raise", making.art);
                    end(&mut commands);
                    continue;
                }
            }
            // The mortar wants its target's place, else its point, on the hero's floor and in a
            // dungeon a straight way to it, else it ends without its regeneration, the hero
            // standing (`0x51D840`).
            if arts::style(making.art) == Some(Style::Mortar) {
                let level = body.level();
                let aimed = creatures.get(making.target).ok().map(|(_, at, ..)| {
                    let on = ways.get(making.target).map_or(level, |way| way.level);
                    (at.translation(), on)
                });
                let point = aimed.or_else(|| making.point.map(|point| (point, level)));
                let dungeon = dungeons
                    .as_deref()
                    .is_some_and(|dungeons| dungeons.holds(here.x, here.z));
                let reaches = point.is_some_and(|point| {
                    mortar_reaches((here, level), point, dungeon, |cell| {
                        heights.flown_onto(cell, level)
                    })
                });
                if !reaches {
                    info!(
                        "making: {}'s point is off the hero's floor or no straight way leads there",
                        making.art
                    );
                    body.stop();
                    end(&mut commands);
                    continue;
                }
            }
            // A boon is made where the hero stands, on itself; any other art on its target alive,
            // within its distance.
            if making.target != hero && !raises {
                // On an enemy hostile; one of the launch type 2 on a friend, which the aim
                // admitted (#3124).
                let friendly = launch(arts, making.art) == Some(2);
                let Some((target, target_reach)) = creatures
                    .get(making.target)
                    .ok()
                    .filter(|(_, _, mind, health)| {
                        (mind.hostile() || friendly) && health.current > 0
                    })
                    .map(|(_, at, mind, _)| (at.translation(), reach(mind)))
                else {
                    end(&mut commands);
                    continue;
                };
                // With a bow, the `Rundum`, the `Hardhit` and the `Attacke` are shots made where
                // the hero stands (`0x4FD0F0`); with a gun the Dwarf's own three too, a pistol's
                // only by a line of sight to the target, which the order asks again as he walks.
                let level = body.level();
                let by_gun = gun.and_then(|class| {
                    arts::gun_order_distance(class, || in_sight(here, target, level, &heights))
                });
                let distance = if bow.is_some() {
                    0
                } else if let Some(distance) = by_gun {
                    distance
                } else {
                    arts::order_distance(making.art, record.distance)
                };
                let Some((at, within)) = made_from(distance, to_f32(target_reach)) else {
                    info!("making: {}'s distance is not made yet", making.art);
                    end(&mut commands);
                    continue;
                };
                let distance = (target - here).with_y(0.0).length();
                if at > 0.0 && distance > at + within {
                    // On the level it stands on, which the hero's way to it is tested on
                    // (`0x4F8560`).
                    let onto = ways.get(making.target).ok().map(|way| way.level);
                    body.send_to(target, onto);
                    continue;
                }
                // From the saddle a `Hardhit` turns the rider to its target first, and the
                // horse it sits on with it (`0x4DBAD0`; seen, #2908); any other art keeps the
                // pair's facing.
                if !riding || arts::style(making.art) == Some(Style::Hardhit) {
                    transform.rotation = hero::facing((target - here).with_y(0.0));
                }
            }
            body.stop();
            // A combo's step at its own level (`0x56ABE0`).
            let (Some(style), Some(level)) = (
                arts::style(making.art),
                making.level.or_else(|| {
                    arts.arts.level_in_play(
                        making.art,
                        Creature {
                            kind: arts.kind,
                            level: experience.level,
                            player: true,
                        },
                    )
                }),
            ) else {
                end(&mut commands);
                continue;
            };
            // The backbreaker takes up a creature of its classes alone, small, and for any other
            // ends at once, without its regeneration (`0x4FF0B0` at `0x502D01`).
            if style == Style::Backbreaker
                && !creatures.get(making.target).is_ok_and(|(_, _, mind, _)| {
                    let thinks = &mind.thinks;
                    arts::backbreaker_takes(thinks.creature, thinks.class, thinks.big)
                })
            {
                info!("making: {} does not take up {}", making.art, making.target);
                end(&mut commands);
                continue;
            }
            // The multi-shot plays no clip: its effect at once, its regeneration, and it ends
            // (`0x50AC00` at `0x50C585`).
            if style == Style::Volley {
                let Some(record) = arts.tables.weapon_arts.iter().find(|a| a.id == making.art)
                else {
                    end(&mut commands);
                    continue;
                };
                let (effect, seconds) = arts::volley(record, level);
                info!(
                    "making: {} gives the hero {} arrows a shot for {seconds} s",
                    making.art, effect.whole
                );
                commands
                    .entity(hero)
                    .entry::<crate::effects::Effects>()
                    .or_default()
                    .and_modify(move |mut effects| effects.add(effect, seconds));
                // Its comets at her left hand for its seconds (861, `0x50C633`).
                crate::elf_sights::multi_shot(&mut commands, hero, level, seconds);
                arts.start(making.art);
                end(&mut commands);
                continue;
            }
            // The `Rundum` with a bow is the hero's shot of its arrows at its target and its
            // nearest hostiles, by 1061's factor at the art's level (#2238); the `Hardhit` its
            // ray at its target by 1062's, the `Attacke` its arrows fanned by 1063's at the level
            // of 1063 itself, which the hero may not hold (#2275).
            if let Some(shot) = bow {
                let struck = if shot == arts::BOW_ATTACKE {
                    arts.arts
                        .level_in_play(
                            shot,
                            Creature {
                                kind: arts.kind,
                                level: experience.level,
                                player: true,
                            },
                        )
                        .unwrap_or(0)
                } else {
                    level
                };
                let factor = arts
                    .tables
                    .weapon_arts
                    .iter()
                    .find(|a| a.id == shot)
                    .map_or(1.0, |record| arts::factor(record, struck));
                let special = crate::arrows::Special {
                    art: shot,
                    level: struck,
                    factor,
                    seconds: None,
                    count: if shot == arts::BOW_HARDHIT {
                        1
                    } else {
                        arts::bow_rundum_arrows(level)
                    },
                };
                info!(
                    "making: {} with a bow shoots {} at the level {level}",
                    making.art, making.target
                );
                commands
                    .entity(hero)
                    .insert(crate::attack::Attack::shot(making.target, special));
                // The `Hardhit`'s and the `Attacke`'s start of the group 0, their time to every art
                // of their kind, the Vampiress's bites and the Dark Elf's traps too (`0x5133A1`,
                // `0x50A29C`, `0x50A409`; #2995); the `Rundum`'s executor starts none, its shot
                // starts 1061's time as it looses (`arrows::loose`, `0x4DEB8C`; #3297).
                if shot != arts::BOW_RUNDUM {
                    arts.start_with(making.art, 1.0, None);
                }
                end(&mut commands);
                continue;
            }
            // A special arrow is the hero's shot at its target, of the art's own arrows, with a
            // bow or a crossbow in hand, its regeneration started; without one, nothing
            // (`0x50AC00`).
            if style == Style::Arrow {
                if holding.is_some_and(|holding| holding.0.ranged) && making.target != hero {
                    // The blades' strikes carry their own factor, and fly for a time.
                    let (seconds, factor) = if making.art == 1008 {
                        let (seconds, factor) = arts::blades(record, level);
                        (Some(seconds), factor)
                    } else {
                        (None, arts::factor(record, level))
                    };
                    let special = crate::arrows::Special {
                        art: making.art,
                        level,
                        factor,
                        seconds,
                        count: 1,
                    };
                    info!(
                        "making: {} shoots {} at the level {level}",
                        making.art, making.target
                    );
                    commands
                        .entity(hero)
                        .insert(crate::attack::Attack::shot(making.target, special));
                    arts.start(making.art);
                } else {
                    info!("making: {} wants a bow or a crossbow", making.art);
                }
                end(&mut commands);
                continue;
            }
            // With a gun the Dwarf's own three test a line of sight to their target from his floor,
            // the way of the flyer's flag, at their first run; without it he stands and the art
            // ends without its clip or its regeneration (`0x518484` to `0x51852B`).
            if gun.is_some()
                && making.target != hero
                && let Ok((_, at, ..)) = creatures.get(making.target)
                && !in_sight(here, at.translation(), body.level(), &heights)
            {
                info!("making: {} sees no line to its target", making.art);
                end(&mut commands);
                continue;
            }
            let Some(action) = clips.arts.iter().find(|a| a.style == style) else {
                info!(
                    "making: the hero has no clip of {} for its model and what it holds",
                    making.art
                );
                end(&mut commands);
                continue;
            };
            // From the saddle a `Hardhit` plays the rider's 194 at its attack speed (the Gladiator's
            // `0x4FF0B0` at `0x502B01`, the Seraphim's `0x5038F0` at `0x505EC6`, the Wood Elf's
            // `0x50AC00` at `0x50C35D`, the others' alike), at its first hit; the arts of a
            // rider's clip of their own play it, at its hit points where it has them, else at
            // the art's (`arts::saddle_clip`, #3134).
            let saddle = if !riding {
                None
            } else if style == Style::Hardhit {
                clips.riding_attack.as_ref().map(|(node, clip, fractions)| {
                    (*node, clip.clone(), vec![fractions[0]], SaddleRate::Attack)
                })
            } else {
                arts::saddle_clip(making.art).and_then(|(slot, rate)| {
                    let own = clips
                        .arts
                        .iter()
                        .find(|a| a.style == Style::Cast(slot) && a.node.is_some())?;
                    let points = if own.points.is_empty() {
                        action.points.clone()
                    } else {
                        own.points.clone()
                    };
                    Some((own.node?, own.clip.clone()?, points, rate))
                })
            };
            // The art's clip, or one of the attack's at random for the Seraphim's `Hardhit`.
            let (node, clip, mut points) = match (action.node, &action.clip) {
                _ if let Some((node, clip, points, _)) = &saddle => {
                    (*node, clip.clone(), points.clone())
                }
                (Some(node), Some(clip)) => (node, clip.clone(), action.points.clone()),
                _ => {
                    let count = u32::try_from(clips.attacks.len()).unwrap_or(0);
                    let Some((node, clip, fractions)) = usize::try_from(dice.0.below(count))
                        .ok()
                        .and_then(|index| clips.attacks.get(index))
                    else {
                        end(&mut commands);
                        continue;
                    };
                    (*node, clip.clone(), vec![fractions[0]])
                }
            };
            let length = animations
                .get(&clip)
                .map_or(1.0, AnimationClip::duration)
                .max(0.01);
            // At the attack speed, quickened by the Dwarf's frenzy (`0x584FE0`).
            let clip_speed = combat.map_or(1.0, |combat| {
                arts::quickened(combat.speed, frenzy.map_or(0.0, |frenzy| frenzy.0)) / 100.0
            });
            // The target's place, the jump's point.
            let mut point = creatures
                .get(making.target)
                .map_or(here, |(_, at, ..)| at.translation());
            let mut rate = action.rate;
            if style == Style::Panther {
                // Its point at its reach at most, at its reach where the middle button is held,
                // where it may stand, and its clip at a rate by its reach (`0x506770` at
                // `0x509E7B`).
                let aimed = making.point.unwrap_or(point);
                let reach = arts::panther_reach(record, level);
                let [x, z] = arts::panther_point(
                    here.xz().into(),
                    aimed.xz().into(),
                    reach,
                    buttons.pressed(MouseButton::Middle),
                );
                point = Vec3::new(x, here.y, z);
                let to = crate::paths::cell_of(point.x, point.z);
                if !heights.walkable_on(to, body.level()) {
                    info!("making: the panther may not stand at its point");
                    end(&mut commands);
                    continue;
                }
                rate = arts::panther_rate(reach);
            }
            if style == Style::Dash {
                // A straight way to its point and a place to stand there, else it ends at once
                // (`0x5139FA`); its line ends at the first thirtieth step beyond her reach, counted
                // along its longer axis, where she may stand, else at its point (`0x50F6AD`).
                let aimed = making.point.unwrap_or(point);
                let floor = body.level();
                let walkable = |cell| heights.walkable_on(cell, floor);
                let stands = |at: Vec3| walkable(crate::paths::cell_of(at.x, at.z));
                if !stands(aimed) || !crate::paths::clear(here.xz(), aimed.xz(), walkable) {
                    info!(
                        "making: the dash's point {:?} from {:?} has no straight way or may not \
                         be stood on",
                        crate::paths::cell_of(aimed.x, aimed.z),
                        crate::paths::cell_of(here.x, here.z)
                    );
                    end(&mut commands);
                    continue;
                }
                let reach = arts::dash_reach(record, level);
                let [x, z] = arts::dash_end(here.xz().into(), aimed.xz().into(), reach, |at| {
                    stands(Vec3::new(at[0], here.y, at[1]))
                });
                point = Vec3::new(x, here.y, z);
            }
            if matches!(style, Style::Jump | Style::Charge) {
                // A jump, and the Daemon's charge, want a straight way to their point and a place
                // to stand there, else end at once (`0x5038F0` at `0x505B6E`; `0x5192E0` at
                // `0x51B65D`, #3173).
                let level = body.level();
                let walkable = |cell| heights.walkable_on(cell, level);
                let flown = |cell| heights.flown_onto(cell, level);
                if !point_reached((here, level), (point, level), (flown, walkable)) {
                    info!("making: no straight way for the jump or the charge to its target");
                    end(&mut commands);
                    continue;
                }
            }
            if style == Style::Jump {
                // A jump plays at a rate by its length (`0x5038F0` at `0x505B6E`).
                rate = if arts.kind == arts::VAMPIRESS_BY_NIGHT {
                    arts::CLAW_JUMP_RATE
                } else {
                    arts::jump_rate(whole_distance(here, point))
                };
            }
            // From the saddle at the attack speed (`0x593320`'s rate; seen 1.54 at the speed 154,
            // #2908).
            // Times its motion's own factor, the Vampiress's clip of the slot 221 among them
            // (`0x413610`, #3567).
            let played = factors.of(&clip)
                * match saddle.as_ref().map(|saddle| saddle.3) {
                    Some(SaddleRate::Attack) => speed.0 * clip_speed,
                    Some(SaddleRate::One) => speed.0,
                    None if action.fixed => speed.0 * rate,
                    None => speed.0 * clip_speed * rate,
                };
            transitions.play(&mut player, node, BLEND).set_speed(played);
            let factor = if style == Style::Backbreaker {
                arts::backbreaker_factor(level)
            } else {
                held_factor(arts::factor(record, level), action.halved)
            };
            // The backbreaker takes its target up, at his neck, and holds it (`0x4FF0B0`,
            // `backbreaker`).
            if style == Style::Backbreaker
                && let Ok((_, at, _, health)) = creatures.get(making.target)
            {
                let seconds = arts::backbreaker_hold(experience.level, health.level);
                let mut held = commands.entity(making.target);
                held.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
                held.entry::<Mind>()
                    .and_modify(move |mut mind| mind.stunned(seconds));
                crate::backbreaker::take_up(&mut commands, making.target, hero, at.rotation());
            }
            // A smite strikes at its own point of its clip, the hurled blades throw at theirs.
            if style == Style::Smite {
                points = vec![1.0 - arts::SMITE_LEFT];
            }
            if style == Style::Throw {
                points = vec![1.0 - arts::THROW_LEFT];
            }
            // A boon gives its effects at its own point of its clip.
            if matches!(
                style,
                Style::Boon
                    | Style::Frenzy
                    | Style::Bolts
                    | Style::Glance
                    | Style::Ward
                    | Style::WardSpells
                    | Style::Form
            ) {
                let boon = boon_of(arts, record, level, (1.0, flight_times));
                points = boon
                    .as_ref()
                    .map(|boon| vec![boon.point])
                    .unwrap_or_default();
                // What it takes off and gives as it starts (the Daemon's 1094).
                if let Some(boon) = boon.filter(|b| !b.clears.is_empty() || !b.starts.is_empty()) {
                    commands
                        .entity(hero)
                        .entry::<crate::effects::Effects>()
                        .or_default()
                        .and_modify(move |mut effects| {
                            effects.take_off(&boon.clears);
                            for (effect, seconds) in boon.starts {
                                effects.add(effect, seconds);
                            }
                        });
                }
            }
            // A `Rundum` and the whirling blow start their regeneration as they start, by their
            // classes' groups (`arts::rundum_starts`, #2995); the hurled blades, the backbreaker
            // and the domination likewise, each its own group.
            if matches!(style, Style::Rundum | Style::Whirl) {
                if let Some(group) = arts::rundum_starts(making.art) {
                    arts.start_with(making.art, 1.0, group);
                }
            } else if matches!(style, Style::Blades | Style::Backbreaker | Style::Dominate) {
                arts.start(making.art);
            }
            info!(
                "making: the hero makes {} ({style:?}) at level {level}, its factor {factor}",
                making.art
            );
            // The hero's battle cry (`0x67A7A0`'s event 5, `0x693FE0`, #713).
            cries.write(crate::voices::BattleCry {
                hero,
                sure: crate::art_sounds::cries_surely(making.art),
            });
            let seconds = length / played.max(f32::EPSILON);
            // The Dwarf's `Rundum` with a gun gathers up to three other hostiles of the hero's
            // list before its clip (`0x518760` to `0x518894`).
            let gathered = if gun.is_some() && style == Style::Rundum {
                let listed = nearest.hostiles.iter().filter_map(|&entity| {
                    let (_, at, mind, health) = creatures.get(entity).ok()?;
                    let hostile = mind.hostile() && health.current > 0;
                    Some((entity, at.translation(), reach(mind), hostile))
                });
                gathers((here, body.level(), making.target), listed, &heights)
            } else {
                Vec::new()
            };
            commands.entity(hero).insert(Playing {
                style,
                node,
                length,
                seconds,
                point,
                points,
                struck: 0,
                fired: 0,
                gathered,
                regenerated: false,
                target: making.target,
                last: None,
                level,
                factor,
            });
            // The weapons' streaks of the arts that its executor streaks, for its clip (#3454).
            crate::weapon_streaks::lay(
                &mut commands,
                hero,
                (making.art, arts.kind, true),
                crate::weapon_streaks::Lasts::Seconds(seconds),
            );
            continue;
        };
        let Some(active) = player.animation(playing.node) else {
            end(&mut commands);
            continue;
        };
        let done = active.seek_time() / playing.length;
        let finished = active.is_finished();
        let hit = |target: Entity, playing: &Playing| fight::Hit {
            fixed: None,
            sure: false,
            poison: false,
            ward: false,
            hurled: false,
            object: false,
            mount: false,
            attacker: hero,
            target,
            share: playing.factor,
            ranged: false,
            art: Some(playing.level),
        };
        let gun_held = holding.is_some_and(|holding| holding.0.gun);
        match playing.style {
            Style::Cast(_) => {
                // Once the share of its clip still to play falls below the spell's, it acts
                // once: its regeneration starts and its object does what it does (`0x521D60`,
                // `0x522480`; `spells`); with less than 0.3 s of its clip to play after, or its
                // clip done, it ends.
                if playing.struck == 0 && playing.points.first().is_some_and(|&p| due(p, done)) {
                    playing.struck = 1;
                    arts.start(making.art);
                    acts.write(crate::spells::Acts {
                        caster: hero,
                        spell: making.art,
                        target: playing.target,
                        point: playing.point,
                        level: playing.level,
                    });
                }
                let left = (1.0 - done) * playing.seconds;
                if finished || (playing.struck > 0 && left < spells::STANDS_WITHIN) {
                    end(&mut commands);
                }
            }
            Style::Kiss => {
                // Below 0.6 left of its clip a skull leaves her neck and homes on its target,
                // cursing each hostile creature it touches (`0x51042C`, `spells::kiss`); its
                // regeneration starts; then it ends.
                if !due(1.0 - arts::KISSES, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                // Its regeneration as the skull leaves her, whatever its target (`0x50E6C0`,
                // #2827).
                arts.start(making.art);
                if let Some(record) = arts.tables.weapon_arts.iter().find(|a| a.id == making.art) {
                    let neck = bone(hero, &[crate::arrows::NECK], &children, (&bones, &worn))
                        .unwrap_or(here + Vec3::Y * 100.0);
                    crate::spells::kiss(
                        &mut commands,
                        (hero, playing.target),
                        neck,
                        (
                            arts::curse(record, playing.level),
                            arts::bleed_value(record, playing.level),
                        ),
                        arts::skull_life(record, playing.level),
                        body.level(),
                    );
                    info!(
                        "making: {} sends its skull at {}",
                        making.art, playing.target
                    );
                }
                end(&mut commands);
            }
            Style::Dominate => {
                // Below 0.6 left of its clip its target, no boss nor champion, is stopped and takes
                // the effect 13, under which it does not fight her (`ai`); then it ends
                // (`0x50E6C0`).
                if !due(1.0 - arts::DOMINATES, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                let ranked = ranks.get(playing.target).map(|rank| rank.0).ok();
                let common = !matches!(
                    ranked,
                    Some(
                        opensacred_rules::fight::Rank::Boss
                            | opensacred_rules::fight::Rank::Champion
                    )
                );
                if let (Some(record), Ok((_, _, mind, health))) =
                    (record, creatures.get(playing.target))
                    && mind.hostile()
                    && health.current > 0
                    && common
                {
                    let seconds = arts::domination_seconds(record, playing.level, health.level);
                    let effect = opensacred_rules::fight::Effect {
                        whole: 1,
                        ..opensacred_rules::fight::Effect::new(arts::DOMINATED, 0.0)
                    };
                    let mut dominated = commands.entity(playing.target);
                    dominated.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
                    dominated
                        .entry::<Mind>()
                        .and_modify(|mut mind| mind.stunned(0.1));
                    dominated
                        .entry::<crate::effects::Effects>()
                        .or_default()
                        .and_modify(move |mut effects| effects.add(effect, seconds));
                    // Its object 857 at it (`0x5124CE`), looping `VAMP_BEHERRSCHUNG_LOOP` while
                    // it lives, for the effect's time (**hypothesis** for its life), and its
                    // ring about its neck (`vamp_sights`).
                    dominated.with_child((
                        crate::hums::Hum(DOMINATION_LOOP),
                        crate::spells::Fades(seconds),
                        Transform::default(),
                    ));
                    crate::vamp_sights::dominate(&mut commands, playing.target, seconds);
                    info!(
                        "making: {} dominates {} for {seconds} s",
                        making.art, playing.target
                    );
                }
                end(&mut commands);
            }
            Style::Trap => {
                // Below 0.7 left of its clip he throws its ball to the action's point, which makes
                // the trap where it lands; its regeneration starts; then it ends (`0x506770`).
                if !due(1.0 - arts::TRAPS, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                arts.start(making.art);
                let multipliers = combat.map_or([1.0; 4], |combat| combat.spells);
                // Its strikes' factor by his skills 2 and 26, his `a2` and the points spent on it
                // (`0x5AC6E9`, #3135).
                let trap = rules.trap_factor(
                    &arts.skills,
                    (
                        combat.map_or(0.0, |combat| combat.attributes[2]),
                        base.map_or(0, |base| base.extras[2]),
                    ),
                );
                // From his right hand (the bone 2, `0x5A55B2`, #2112).
                let hand = bone(hero, &[HAND_BONE], &children, (&bones, &worn)).unwrap_or(here);
                crate::spells::throw_trap(
                    &mut commands,
                    hero,
                    (making.art, playing.level),
                    (multipliers, trap),
                    (hand, playing.point, body.level()),
                );
                end(&mut commands);
            }
            Style::Bats => {
                // Below 0.87 left of its clip a bat of its type beside her as her follower, sent at
                // its target, her bats of that type beyond its factor dismissed; its regeneration
                // starts; then it ends (`0x50E6C0`).
                if !due(1.0 - arts::BATS, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                arts.start(making.art);
                if let Some(record) = arts.tables.weapon_arts.iter().find(|a| a.id == making.art) {
                    crate::followers::bats(
                        &mut commands,
                        hero,
                        playing.target,
                        arts::bat(making.art),
                        playing.level,
                        arts::bats_most(record, playing.level),
                    );
                }
                end(&mut commands);
            }
            Style::MasterBite => {
                // Below 0.6 left of its clip its target, of no big type and below her bound,
                // takes the effect 14 for 12 + 3·L s, which turns
                // its sides over, so that it fights its old friends (seen); then it ends
                // (`0x50E6C0`).
                if !due(1.0 - arts::MASTER_BITES, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                // Its regeneration once it has a target of no big type (`0x426CB0`), whether its
                // level lets it take it or not; without one it waits at each frame, and a clip
                // that ends so starts none (`0x50E6C0` at `0x50FF4D`, #2827).
                if !creatures
                    .get(playing.target)
                    .is_ok_and(|(_, _, mind, _)| !mind.big)
                {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                arts.start(making.art);
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                let boss = ranks
                    .get(playing.target)
                    .is_ok_and(|rank| rank.0 == opensacred_rules::fight::Rank::Boss);
                if let (Some(record), Ok((_, _, mind, health))) =
                    (record, creatures.get(playing.target))
                    && mind.hostile()
                    && health.current > 0
                    && arts::master_bites(
                        mind.big,
                        health.level,
                        boss,
                        experience.level,
                        playing.level,
                    )
                {
                    let seconds = arts::master_bite_seconds(record, playing.level);
                    // Its three ghosts about its head, each lighting where she stands
                    // (`0x50FC88`, #3062).
                    let size = sizes
                        .get(playing.target)
                        .map_or(crate::arrows::size(0), |size| size.0);
                    crate::bite_sights::master_bite(
                        &mut commands,
                        playing.target,
                        (size, seconds),
                        transform.translation,
                    );
                    let effect = opensacred_rules::fight::Effect {
                        whole: 1,
                        art: u32::from(making.art),
                        ..opensacred_rules::fight::Effect::new(
                            opensacred_rules::spells::CONVERTED,
                            0.0,
                        )
                    };
                    let mut bitten = commands.entity(playing.target);
                    bitten.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
                    bitten
                        .entry::<crate::effects::Effects>()
                        .or_default()
                        .and_modify(move |mut effects| effects.add(effect, seconds));
                    info!(
                        "making: {} turns {} over for {seconds} s",
                        making.art, playing.target
                    );
                }
                end(&mut commands);
            }
            Style::Raise => {
                // Below 0.6 left of its clip its dead creature rises as her follower, its level
                // `⌊3·L + 0.4·l⌋`, for twice its factor in seconds (the effect 29); its
                // regeneration starts; then it ends (`0x511088`).
                if !due(1.0 - arts::RAISES, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                arts.start(making.art);
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                if let Some(record) = record
                    && let Ok((corpse, _)) = corpses.get(playing.target)
                {
                    // Its object 866 at the corpse (`0x51127E`): its `VAMP_PENTA` as it starts
                    // (`0x79E5D0`), and its pentagram under it (`vamp_sights`).
                    points.write(crate::sfx::PlayAt {
                        sound: crate::sfx::Of::Id(RAISE_SOUND),
                        point: corpse.translation(),
                        rate: (0.5, crate::sfx::Roll::None),
                        factor: 1.0,
                    });
                    crate::vamp_sights::raise(&mut commands, playing.target, corpse.translation());
                    crate::followers::raise(
                        &mut commands,
                        hero,
                        playing.target,
                        arts::raised_level(experience.level, playing.level),
                        Some(arts::raised_seconds(record, playing.level)),
                    );
                }
                end(&mut commands);
            }
            Style::Summon => {
                // Below 0.6 left of its clip a summoning at the first point about the call's that a
                // straight way from her reaches, which makes her a wolf of her level and the
                // call's for her follower; its regeneration starts whether one is made or not;
                // then it ends (`0x512A13`, `0x512AEC`).
                if !due(1.0 - arts::SUMMONS, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                arts.start(making.art);
                let level = body.level();
                let reaches = |[x, z]: [f32; 2]| {
                    heights.walkable_on(crate::paths::cell_of(x, z), level)
                        && crate::paths::straight(here.xz(), Vec2::new(x, z), |cell| {
                            heights.walkable_on(cell, level)
                        })
                };
                let point = [playing.point.x, playing.point.z];
                match arts::summoning_point(point, &mut dice.0, reaches) {
                    Some([x, z]) => crate::followers::summon(
                        &mut commands,
                        hero,
                        opensacred_rules::spells::summoned(arts.kind),
                        arts::wolf_level(experience.level, playing.level, !by_day),
                        Vec3::new(x, here.y, z),
                    ),
                    None => info!("making: {} finds no point to summon at", making.art),
                }
                end(&mut commands);
            }
            Style::Bite => {
                // Below 0.9 left of its clip its target is held with the effect 15 and struck with
                // a fixed physical damage of its drain, and she takes as much health as a heal,
                // her poisons cured; then it ends (`0x50E6C0`).
                if !due(1.0 - arts::BITES, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                // Its regeneration, with a target or none, whatever her bones and her beam reach
                // (`0x50E6C0` at `0x510418`, #2827).
                arts.start(making.art);
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                // Her star at her spine where her target is a creature (`0x50FFAC`, #3062).
                if creatures.contains(playing.target) {
                    crate::bite_sights::bite(&mut commands, hero);
                }
                // Her beam from her spine to its (`beams`, #2119), whose line, which a cell it may
                // not pass cuts short, the bite's hold and drain go with (`0x510140`, `0x604030`,
                // #2123).
                crate::beams::lay(
                    &mut commands,
                    (
                        crate::beams::End::spine(hero),
                        crate::beams::End::spine(playing.target),
                    ),
                    opensacred_rules::particles::BITE_BEAM,
                );
                if let (Some(record), Ok((_, at, mind, health))) =
                    (record, creatures.get(playing.target))
                    && mind.hostile()
                    && health.current > 0
                    && crate::paths::laser_reaches(
                        transform.translation,
                        at.translation(),
                        |cell| heights.passes(cell, body.level()),
                    )
                {
                    let drain = arts::bite_drain(record, playing.level);
                    // Its star at its spine, as the bite reaches it (`0x51014D`, #3062).
                    crate::bite_sights::bite(&mut commands, playing.target);
                    let effect = opensacred_rules::fight::Effect {
                        art: u32::from(making.art),
                        ..opensacred_rules::fight::Effect::new(15, 0.0)
                    };
                    let mut bitten = commands.entity(playing.target);
                    bitten.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
                    bitten
                        .entry::<Mind>()
                        .and_modify(|mut mind| mind.stunned(arts::BITE_HOLD));
                    bitten
                        .entry::<crate::effects::Effects>()
                        .or_default()
                        .and_modify(move |mut effects| effects.add(effect, arts::BITE_HOLD));
                    hits.write(fight::Hit {
                        share: 1.0,
                        fixed: Some([to_f32(drain), 0.0, 0.0, 0.0]),
                        ..hit(playing.target, &playing)
                    });
                    // Her health by her own event 6 of the sub-kind 3, the heal's, which cures
                    // her poisons too (`0x510248` to `0x5102DC`, #3213).
                    commands
                        .entity(hero)
                        .entry::<Health>()
                        .and_modify(move |mut health| {
                            health.current = (health.current + drain).min(health.most);
                        });
                    crate::effects::heal_cures(&mut commands, hero);
                    info!("making: {} drains {drain}", making.art);
                }
                end(&mut commands);
            }
            Style::Backbreaker => {
                // At its point it lets its target go with its strike, thrown from his right hand
                // along the line from him to where it was taken up (`0x5014C7`–`0x501769`,
                // `backbreaker`); the target stays held by a roll, else its timer comes back
                // within 0.5 s (`0x4FF0B0`).
                let point = playing.points.first().copied().unwrap_or(0.5);
                if !due(point, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                if let Ok((_, _, _, health)) = creatures.get(playing.target)
                    && health.current > 0
                {
                    let speed = arts
                        .tables
                        .weapon_arts
                        .iter()
                        .find(|a| a.id == making.art)
                        .map_or(0.0, |record| {
                            arts::backbreaker_throw(
                                record,
                                playing.level,
                                experience.level,
                                health.level,
                            )
                        });
                    let hand = bone(
                        hero,
                        &crate::backbreaker::LET_GO_AT,
                        &children,
                        (&bones, &worn),
                    )
                    .unwrap_or(here);
                    let level = ways.get(playing.target).ok().and_then(|way| way.level);
                    let ground = heights.standing(hand.x, hand.z, level).unwrap_or(here.y);
                    let along = (playing.point - here).xz().normalize_or_zero() * speed;
                    let flight = arts::Flight {
                        height: crate::backbreaker::model_height(hand.y, ground),
                        ..arts::Flight::new(along.into(), arts::BACKBREAKER_RISE)
                    };
                    info!(
                        "making: {} throws {} at {speed:.0}",
                        making.art, playing.target
                    );
                    crate::backbreaker::throw(&mut commands, playing.target, hand, flight);
                    hits.write(hit(playing.target, &playing));
                    sparks.write(Spark {
                        art: making.art,
                        by: hero,
                        on: playing.target,
                    });
                    let boss = ranks
                        .get(playing.target)
                        .is_ok_and(|rank| rank.0 == opensacred_rules::fight::Rank::Boss);
                    let roll = dice.0.below(100);
                    if !arts::backbreaker_holds(playing.level, health.level, boss, roll) {
                        commands
                            .entity(playing.target)
                            .entry::<Mind>()
                            .and_modify(|mut mind| mind.let_go(arts::LET_GO));
                    }
                }
                end(&mut commands);
            }
            Style::Dash => {
                // Below 0.8 left of its clip she runs her line, a figure of her in red sparks at
                // every 30th step of it (`0x50F857`, #3468), is put at its point, starts its
                // regeneration and ends (`0x50E6C0`).
                if due(1.0 - arts::DASHES, done) {
                    let places = arts::dash_sparks(
                        transform.translation.xz().into(),
                        playing.point.xz().into(),
                    );
                    commands.queue(move |world: &mut World| {
                        crate::group_looks::figures_at(world, hero, &places, arts::DASH_SPARKS);
                    });
                    body.teleport(&mut transform, playing.point);
                    arts.start(making.art);
                    end(&mut commands);
                } else if finished {
                    end(&mut commands);
                }
            }
            Style::Changeling => {
                // Below 0.5 left of its clip the Vampiress changes her form: 1022 into her
                // night's for its time and its regeneration started, 1023 back to her day's, the
                // change setting 1022's regeneration whole again (seen; `0x50E6C0`, `0x557040`).
                if !due(1.0 - arts::CHANGES, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                let night = making.art == 1022;
                let seconds = arts
                    .tables
                    .weapon_arts
                    .iter()
                    .find(|a| a.id == making.art)
                    .map_or(0.0, |record| {
                        arts::changeling_seconds(
                            record,
                            playing.level,
                            by_day,
                            holding.map_or(1.0, |holding| holding.0.sets.night_form_times()),
                        )
                    });
                commands.trigger(crate::forms::Change { night, seconds });
                // 1022 its own start; 1023 its own only out of her night's form, which it is
                // made in, so none then (`0x5129D7`); her change of form then sets her day's
                // arts' waits whole (`forms`, `0x557040`; #2995).
                if night || arts.kind != opensacred_rules::arts::VAMPIRESS_BY_NIGHT {
                    arts.start(making.art);
                }
                end(&mut commands);
            }
            Style::Panther => {
                // Below 0.7 left of its clip it leaps to its point in half its clip's time, starts
                // its regeneration and ends (`0x506770` at `0x507DAA`).
                if due(1.0 - arts::PANTHER_LEAPS, done) {
                    let to = Vec3::new(playing.point.x, here.y, playing.point.z);
                    commands.entity(hero).insert(Leap {
                        from: here,
                        to,
                        time: playing.seconds * arts::PANTHER_FLIGHT,
                        flown: 0.0,
                        peak: arts::jump_height((to - here).xz().length(), 0.5),
                        stops: arts::flight_stops(arts.kind),
                    });
                    arts.start(making.art);
                    end(&mut commands);
                } else if finished {
                    end(&mut commands);
                }
            }
            Style::Viper => {
                // Below 0.2 left of its clip, its target and each hostile of the hero's look
                // within its radius of the target, by a kick's roll, stunned with the effect 35;
                // then it ends (`0x506770` at `0x508C3B`).
                if !due(1.0 - arts::VIPER_LEFT, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                // Its regeneration, with a target or none (`0x506770` at `0x507D84`, #2827).
                arts.start(making.art);
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                if let (Some(record), Ok((_, centre, ..))) = (record, creatures.get(playing.target))
                {
                    let (radius, seconds) = arts::viper(record, playing.level);
                    let centre = centre.translation();
                    let mut stung = Vec::new();
                    let others = nearest.hostiles.iter().filter(|&&e| e != playing.target);
                    for &entity in std::iter::once(&playing.target).chain(others) {
                        let Ok((_, at, mind, health)) = creatures.get(entity) else {
                            continue;
                        };
                        let near = entity == playing.target
                            || to_f32(whole_distance(at.translation(), centre)) < radius;
                        if !near || !mind.hostile() || health.current == 0 {
                            continue;
                        }
                        if knocks_back(&mut dice.0, playing.level, mind) {
                            stung.push(entity);
                            let effect = opensacred_rules::fight::Effect {
                                id: 35,
                                whole: 0,
                                value: 0.0,
                                art: u32::from(making.art),
                                level: 0,
                            };
                            let mut stunned = commands.entity(entity);
                            stunned.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
                            stunned
                                .entry::<Mind>()
                                .and_modify(move |mut mind| mind.stunned(seconds));
                            stunned
                                .entry::<crate::effects::Effects>()
                                .or_default()
                                .and_modify(move |mut effects| effects.add(effect, seconds));
                            // Its object 864 at it, the magic circle for its time
                            // (`dark_sights`).
                            commands.spawn(crate::dark_sights::sight(crate::dark_sights::Sight {
                                bearer: entity,
                                kind: DarkSightKind::Circle,
                                seconds,
                                bone: None,
                                follows: true,
                            }));
                        }
                    }
                    info!(
                        "making: {} stuns {} creatures for {seconds} s",
                        making.art,
                        stung.len()
                    );
                }
                end(&mut commands);
            }
            Style::Roar => {
                // At its first run each hostile creature within its radius of the hero, or of
                // the friend it is made on (`0x5490B0`, #3124), by a kick's roll, is knocked back
                // from there and held; then its regeneration starts and its clip plays on
                // (`0x4FF0B0` at `0x4FFB75`).
                if playing.struck == 0 {
                    playing.struck = 1;
                    let here = creatures
                        .get(playing.target)
                        .map_or(here, |(_, at, ..)| at.translation());
                    if let Some(record) =
                        arts.tables.weapon_arts.iter().find(|a| a.id == making.art)
                    {
                        let radius = arts::roar_radius(record, playing.level);
                        let left = to_f32(arts::roar_knockback(record, playing.level));
                        let held = arts::roar_hold(record, playing.level);
                        let mut roared = 0;
                        for (entity, at, mind, health) in &creatures {
                            if !mind.hostile()
                                || health.current == 0
                                || whole_distance(at.translation(), here) >= radius
                            {
                                continue;
                            }
                            roared += 1;
                            sparks.write(Spark {
                                art: making.art,
                                by: hero,
                                on: entity,
                            });
                            // Its hit clip, roared at, knocked or not (`0x593770`, #3141).
                            hit_clip(&mut commands, entity, left);
                            if knocks_back(&mut dice.0, playing.level, mind) {
                                commands
                                    .entity(entity)
                                    .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                                    .insert(Knocked {
                                        by: hero,
                                        from: here,
                                        left,
                                        held,
                                        throw: None,
                                        unheld: false,
                                    });
                            }
                        }
                        info!("making: {} roars at {roared} creatures", making.art);
                    }
                    arts.start(making.art);
                }
                if finished {
                    end(&mut commands);
                }
            }
            Style::Hurl => {
                // Once what is left of its clip is below its first hit point, once: its target
                // thrown along the line from the hero, held where it lives and struck; then its
                // regeneration starts, with a target or without, and it ends (`0x514790` at
                // `0x517EE2`).
                let Some(&point) = playing.points.first() else {
                    end(&mut commands);
                    continue;
                };
                if !due(point, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                if let (Some(record), Ok((_, at, mind, health))) =
                    (record, creatures.get(playing.target))
                    && health.current > 0
                {
                    let speed = arts::hurl_speed(
                        record,
                        playing.level,
                        (mind.creature, mind.level),
                        mind.big,
                    );
                    let along = (at.translation() - here).xz().normalize_or_zero() * speed;
                    let hold = arts::hurl_hold(playing.level, mind.level);
                    info!(
                        "making: {} throws {} at {speed:.0}, held {hold:.1} s",
                        making.art, playing.target
                    );
                    // Its burning light for its hold's whole seconds (the FX event `0x2B`'s `[1]`,
                    // #3142).
                    crate::holding_sights::burn(&mut commands, playing.target, hold);
                    // Its hit clip at the rate 1.0 (`0x5182F3`, seen, #3141).
                    commands
                        .entity(playing.target)
                        .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                        .insert((
                            Knocked {
                                by: hero,
                                from: here,
                                left: 0.0,
                                held: hold,
                                throw: Some(arts::Flight::new(along.into(), arts::THROW_RISE)),
                                unheld: false,
                            },
                            crate::npcs::ScriptClip::fall(arts::HIT_CLIP),
                        ));
                    hits.write(fight::Hit {
                        share: arts::hurl_factor(record, playing.level),
                        ..hit(playing.target, &playing)
                    });
                }
                arts.start(making.art);
                end(&mut commands);
            }
            Style::Throw => {
                // Once what is left of its clip is below 0.6, once: the weapon leaves the hero's
                // hand toward where its target stands (`hurled`), its regeneration starts, and
                // the art ends at the throw (`0x501D39`, `0x50226D`).
                let point = playing.points.first().copied().unwrap_or(0.4);
                if !due(point, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                let at = creatures.get(playing.target).map_or_else(
                    |_| here + transform.rotation * Vec3::X,
                    |(_, at, ..)| at.translation(),
                );
                commands.entity(hero).insert(crate::hurled::Hurls {
                    at,
                    level: playing.level,
                    factor: playing.factor,
                    clip_left: playing.seconds * (1.0 - done.clamp(0.0, 1.0)),
                });
                arts.start(making.art);
                end(&mut commands);
            }
            Style::Sweep => {
                // Once what is left of its clip is below its first hit point: its target alive and
                // every hostile creature alive about it, those at a part of its factor; then its
                // regeneration starts, with a target or without, and it ends (`0x514790` at
                // `0x517BB5`).
                let Some(&point) = playing.points.first() else {
                    end(&mut commands);
                    continue;
                };
                if !due(point, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                let target = creatures
                    .get(playing.target)
                    .ok()
                    .filter(|(.., health)| health.current > 0);
                if let (Some(record), Some((_, at, ..))) = (record, target) {
                    hits.write(hit(playing.target, &playing));
                    sparks.write(Spark {
                        art: making.art,
                        by: hero,
                        on: playing.target,
                    });
                    let centre = at.translation();
                    let part = arts::sweep_factor(record, playing.level);
                    let about: Vec<Entity> = creatures
                        .iter()
                        .filter(|&(entity, at, mind, health)| {
                            entity != playing.target
                                && mind.hostile()
                                && health.current > 0
                                && arts::sweep_reaches(
                                    record,
                                    (at.translation() - centre).xz().into(),
                                )
                        })
                        .map(|(entity, ..)| entity)
                        .collect();
                    info!(
                        "making: {} strikes its target and {} about it",
                        making.art,
                        about.len()
                    );
                    for entity in about {
                        hits.write(fight::Hit {
                            share: part,
                            ..hit(entity, &playing)
                        });
                        sparks.write(Spark {
                            art: making.art,
                            by: hero,
                            on: entity,
                        });
                    }
                }
                arts.start(making.art);
                end(&mut commands);
            }
            Style::Flamer | Style::Mortar | Style::Cannon => {
                // The Dwarf's guns (`0x514790`), their damages by his multipliers and his gun's
                // factor, his `a2`'s and his skill 2's (`0x4D21C0`).
                let Some(record) = arts
                    .tables
                    .weapon_arts
                    .iter()
                    .find(|a| a.id == making.art)
                    .cloned()
                else {
                    end(&mut commands);
                    continue;
                };
                let multipliers = combat.map_or([1.0; 4], |combat| combat.multipliers);
                let gun = rules.glance_factor(
                    &arts.skills,
                    combat.map_or(0.0, |combat| combat.attributes[2]),
                );
                let facing = (transform.rotation * Vec3::X).xz().normalize_or(Vec2::X);
                let target = creatures
                    .get(playing.target)
                    .ok()
                    .filter(|(.., health)| health.current > 0)
                    .map(|(_, at, ..)| at.translation());
                let first = playing.points.first().copied().unwrap_or(0.5);
                let hostile_near = |from: Vec2, to: Vec2| -> Vec<(Entity, &Mind)> {
                    creatures
                        .iter()
                        .filter(|&(entity, at, mind, health)| {
                            let size = sizes.get(entity).map_or(crate::arrows::size(0), |s| s.0);
                            mind.hostile()
                                && health.current > 0
                                && segment_distance(at.translation().xz(), from, to)
                                    < crate::arrows::size(0) + size
                        })
                        .map(|(entity, _, mind, _)| (entity, mind))
                        .collect()
                };
                match playing.style {
                    Style::Flamer => {
                        // At its clip's first hit point its regeneration starts; while what is
                        // left of its clip lies within the flames' part, every hostile creature
                        // along the line before him burns, once a burn; below it, it ends
                        // (`0x516D45`).
                        if playing.struck == 0 && due(first, done) {
                            playing.struck = 1;
                            arts.start(making.art);
                            gun_sound(&mut points, FLAMER_SOUND, here);
                            // Its flames at his cannon's muzzle (1016, `dwarf_fires`).
                            crate::dwarf_fires::fire(
                                &mut commands,
                                opensacred_rules::particles::DwarfFire::Flamer,
                                hero,
                            );
                        }
                        let left = 1.0 - done;
                        if left < arts::FLAMES[1] || finished {
                            end(&mut commands);
                            continue;
                        }
                        if left < arts::FLAMES[0] {
                            let fire = arts::gun_damage(
                                record.value_58,
                                playing.level,
                                multipliers[1],
                                gun,
                            );
                            let seconds = to_f32(arts::flame_seconds(&record, playing.level));
                            let from = here.xz();
                            // On his layer and reached by a straight way from him (`0x5FFC70`,
                            // `0x4FCC80`; #3143).
                            let level = body.level();
                            let reached = |entity: Entity| {
                                ways.get(entity).is_ok_and(|way| way.level == level)
                                    && creatures.get(entity).is_ok_and(|(_, at, ..)| {
                                        crate::paths::straight(
                                            from,
                                            at.translation().xz(),
                                            |cell| heights.walkable_on(cell, level),
                                        )
                                    })
                            };
                            let burned: Vec<Entity> =
                                hostile_near(from, from + facing * arts::FLAMES_LENGTH)
                                    .into_iter()
                                    .map(|(entity, _)| entity)
                                    .filter(|&entity| reached(entity))
                                    .collect();
                            if !burned.is_empty() {
                                commands
                                    .entity(hero)
                                    .entry::<crate::burns::Burns>()
                                    .or_default()
                                    .and_modify(move |mut burns| {
                                        for entity in burned {
                                            if burns.add(entity, arts::Burn::new(fire, 1, seconds))
                                            {
                                                debug!("making: the flames burn {entity}");
                                            }
                                        }
                                    });
                            }
                        }
                    }
                    Style::Mortar => {
                        // At its clip's first hit point, a shell to its target's place, else
                        // its point, at least 200 off, which strikes 0.775 s later; its
                        // regeneration starts and it ends (`0x514790`, `0x5B40CD`).
                        if !due(first, done) {
                            if finished {
                                end(&mut commands);
                            }
                            continue;
                        }
                        let aim = target.unwrap_or(playing.point);
                        let at =
                            arts::mortar_point(here.xz().into(), aim.xz().into(), facing.into());
                        let damages = [
                            arts::gun_damage(record.factor, playing.level, multipliers[0], gun),
                            arts::gun_damage(record.value_58, playing.level, multipliers[1], gun),
                            0.0,
                            0.0,
                        ];
                        info!(
                            "making: {} throws a shell to {at:?} of {damages:?}",
                            making.art
                        );
                        gun_sound(&mut points, MORTAR_SOUND, here);
                        // Its burst at his cannon's muzzle (1018, `dwarf_fires`).
                        crate::dwarf_fires::fire(
                            &mut commands,
                            opensacred_rules::particles::DwarfFire::Grenade,
                            hero,
                        );
                        commands.spawn(crate::guns::Shell::thrown(
                            hero,
                            Vec2::from(at),
                            body.level(),
                            damages,
                        ));
                        arts.start(making.art);
                        end(&mut commands);
                    }
                    _ => {
                        // While what is left of its clip lies above `0.5 − 0.0002·d` he turns to
                        // his target; then the shot along the line to it, to the first cell
                        // that stops it, strikes every hostile creature on it and may knock it
                        // back; its regeneration starts and it ends (`0x5174E7`).
                        // Its flash 1017 once what is left of its clip falls below
                        // `1 − h₀ + 0.05·r` (`0x5172F0`).
                        if playing.struck == 0 && due(first - 0.05 * arts::GUN_RATE, done) {
                            playing.struck = 1;
                            gun_sound(&mut points, CANNON_SOUND, here);
                            // Its flash at his cannon's muzzle (1017, `dwarf_fires`).
                            crate::dwarf_fires::fire(
                                &mut commands,
                                opensacred_rules::particles::DwarfFire::Shoot,
                                hero,
                            );
                        }
                        let aim = target.unwrap_or(playing.point);
                        let line = (aim - here).xz();
                        if !due(1.0 - arts::cannon_left(line.length()), done) {
                            if target.is_some() && line.length() > f32::EPSILON {
                                transform.rotation = hero::facing(line.extend(0.0).xzy());
                            }
                            if finished {
                                end(&mut commands);
                            }
                            continue;
                        }
                        let direction = line.try_normalize().unwrap_or(facing);
                        let from = here.xz();
                        let mut reach = 0.0;
                        for step in 1..=arts::CANNON_STEPS {
                            let at = from + direction * arts::CANNON_STEP * to_f32(step);
                            if !heights.walkable_on(crate::paths::cell_of(at.x, at.y), body.level())
                            {
                                break;
                            }
                            reach = arts::CANNON_STEP * to_f32(step);
                        }
                        let far = from + direction * reach;
                        let struck = hostile_near(from, far);
                        info!("making: {} shoots {} creatures", making.art, struck.len());
                        for (entity, mind) in struck {
                            // Its impact at the creature along the line's far end less it
                            // (`0x517A10`, `gun_fires`).
                            let away = creatures
                                .get(entity)
                                .ok()
                                .and_then(|(_, at, _, _)| {
                                    (far - at.translation().xz()).try_normalize()
                                })
                                .unwrap_or(direction);
                            impacts.write(crate::gun_fires::Impact {
                                on: entity,
                                direction: Vec3::new(away.x, 0.0, away.y),
                            });
                            let damages = arts::cannon_damages(
                                &record,
                                playing.level,
                                multipliers,
                                gun,
                                entity == playing.target,
                            );
                            hits.write(fight::Hit {
                                attacker: hero,
                                target: entity,
                                share: 1.0,
                                ranged: false,
                                art: None,
                                fixed: Some(damages),
                                sure: true,
                                poison: false,
                                ward: false,
                                hurled: false,
                                object: false,
                                mount: false,
                            });
                            if !mind.big && knocks_back(&mut dice.0, playing.level, mind) {
                                hit_clip(&mut commands, entity, arts::CANNON_KNOCK);
                                commands
                                    .entity(entity)
                                    .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                                    .insert(Knocked {
                                        by: hero,
                                        from: here,
                                        left: arts::CANNON_KNOCK,
                                        held: 0.0,
                                        throw: None,
                                        // Its hold cleared at once (`0x51798E`, #3430).
                                        unheld: true,
                                    });
                            }
                        }
                        arts.start(making.art);
                        end(&mut commands);
                    }
                }
            }
            Style::Ring | Style::Fan | Style::Breath => {
                // Below 0.3 of its clip, once: the ring of poison burns every hostile creature
                // alive within its reach and the creature's size (`0x5192E0` at `0x51A9B7`), or
                // the fan of bolts leaves him toward his target, else his point (at `0x51AD88`);
                // its regeneration starts and it ends.
                if !due(1.0 - arts::DAEMON_ACTS, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                let multipliers = combat.map_or([1.0; 4], |combat| combat.multipliers);
                if let Some(record) = record {
                    if playing.style == Style::Breath {
                        // His breath toward his point (`0x51AD2B`).
                        let fire = crate::breath::Fire {
                            by: hero,
                            damage: arts::ring_poison(record, playing.level, multipliers[1]),
                            seconds: to_f32(arts::daemon_ratio(record, playing.level)),
                        };
                        info!("making: {} breathes {fire:?}", making.art);
                        commands.spawn(crate::breath::Breath::new(
                            fire,
                            here.xz(),
                            playing.point.xz(),
                            body.level(),
                        ));
                    } else if playing.style == Style::Ring {
                        let seconds = to_f32(arts::daemon_ratio(record, playing.level));
                        let poison = arts::ring_poison(record, playing.level, multipliers[3]);
                        let burned: Vec<Entity> = creatures
                            .iter()
                            .filter(|&(entity, at, mind, health)| {
                                let size =
                                    sizes.get(entity).map_or(crate::arrows::size(0), |s| s.0);
                                mind.hostile()
                                    && health.current > 0
                                    && (at.translation() - here).xz().length()
                                        < arts::RING_REACH + size
                            })
                            .map(|(entity, ..)| entity)
                            .collect();
                        info!(
                            "making: {}'s ring burns {} creatures for {poison} a second, {seconds} s",
                            making.art,
                            burned.len()
                        );
                        // Its look, the object 1008 at him (`0x51B46E`).
                        rings.write(crate::poison_rings::Made {
                            at: here,
                            kind: crate::poison_rings::Kind::Poison,
                        });
                        commands
                            .entity(hero)
                            .entry::<crate::burns::Burns>()
                            .or_default()
                            .and_modify(move |mut burns| {
                                for entity in burned {
                                    burns.add(entity, arts::Burn::new(poison, 3, seconds));
                                }
                            });
                    } else {
                        let aim = creatures
                            .get(playing.target)
                            .ok()
                            .filter(|(.., health)| health.current > 0)
                            .map_or(playing.point, |(_, at, ..)| at.translation());
                        let n = arts::daemon_ratio(record, playing.level);
                        let physical = arts::ring_poison(record, playing.level, multipliers[0]);
                        let magic = arts::fan_magic(record, playing.level, multipliers[2]);
                        let fan = arts::fan((aim - here).xz().into(), n);
                        info!(
                            "making: {} fans out {} bolts of {magic} magic",
                            making.art,
                            fan.len()
                        );
                        for direction in fan {
                            launches.write(crate::bolts::Launch {
                                shooter: hero,
                                direction: Vec2::from(direction),
                                course: crate::bolts::Course::emperor(),
                                damages: [physical, 0.0, magic, 0.0],
                                wait: 0.0,
                            });
                        }
                    }
                }
                arts.start(making.art);
                end(&mut commands);
            }
            Style::Charge => {
                // At its run after its first, the world's next tick, he flies to his target short
                // of half the two's reach, or to its point, in a flight of his own (`0x5192E0` at
                // `0x519BD0`); landed, at the next tick, his rage's effects go (those keyed by 1094
                // and `DAEMON_FORM`), its regeneration starts, he strikes his target within the
                // reach, else the first of his hostiles within it, makes his blast about him and
                // ends (at `0x519D41`).
                if tick.seconds.is_none() {
                    continue;
                }
                if playing.struck == 0 {
                    playing.struck = 1;
                    let target = creatures
                        .get(playing.target)
                        .ok()
                        .filter(|(.., health)| health.current > 0);
                    let to = target.map_or(playing.point, |(_, at, ..)| at.translation());
                    let within = target.map(|(_, _, mind, _)| reach(mind));
                    let [x, z] = arts::jump_landing(here.xz().into(), to.xz().into(), within);
                    let (time, peak) = arts::charge_flight((to - here).xz().length());
                    commands.entity(hero).insert(Leap {
                        from: here,
                        to: Vec3::new(x, here.y, z),
                        time,
                        flown: 0.0,
                        peak,
                        stops: arts::flight_stops(arts.kind),
                    });
                    continue;
                }
                if leaps.contains(hero) {
                    continue;
                }
                commands
                    .entity(hero)
                    .entry::<crate::effects::Effects>()
                    .or_default()
                    .and_modify(|mut effects| effects.take_off(&[1094, arts::DAEMON_FORM]));
                arts.start(making.art);
                let near = |(_, at, _, health): &(Entity, &GlobalTransform, &Mind, &Health),
                            within: u32| {
                    health.current > 0 && whole_distance(at.translation(), here) <= within
                };
                let struck = creatures
                    .get(playing.target)
                    .ok()
                    .filter(|target| near(target, reach(target.2)))
                    .or_else(|| {
                        let first = creatures.get(*nearest.hostiles.first()?).ok()?;
                        near(&first, reach(first.2).saturating_sub(1)).then_some(first)
                    })
                    .filter(|(_, _, mind, _)| mind.hostile());
                if let Some((entity, ..)) = struck {
                    hits.write(hit(entity, &playing));
                    sparks.write(Spark {
                        art: making.art,
                        by: hero,
                        on: entity,
                    });
                }
                if let Some(record) = arts.tables.weapon_arts.iter().find(|a| a.id == making.art) {
                    let blast = arts::Blast::of(record, playing.level);
                    info!("making: the hero's charge lands, its blast {blast:?}");
                    commands.spawn(crate::blast::Blasting::new(
                        hero,
                        blast,
                        body.level(),
                        playing.level,
                        here,
                    ));
                }
                end(&mut commands);
            }
            Style::Jump => {
                // Once what is left of its clip is below its first point it flies to its target,
                // short of it by half the two's reach, or to its point; below its second it lands:
                // the Seraphim strikes her target where it lives (`0x5038F0` at `0x504570`), the
                // Gladiator every hostile of his list about him, stunning some by the kicks' roll
                // (`0x4FF0B0` at `0x50057C`).
                let [flies, lands] = arts::jump_points(arts.kind);
                if playing.struck == 0 && due(1.0 - flies, done) {
                    playing.struck = 1;
                    let target = creatures
                        .get(playing.target)
                        .ok()
                        .filter(|(.., health)| health.current > 0);
                    let to = target.map_or(playing.point, |(_, at, ..)| at.translation());
                    let within = target.map(|(_, _, mind, _)| reach(mind));
                    let [x, z] = arts::jump_landing(here.xz().into(), to.xz().into(), within);
                    let to = Vec3::new(x, here.y, z);
                    commands.entity(hero).insert(Leap {
                        from: here,
                        to,
                        time: playing.seconds * arts::JUMP_FLIGHT,
                        flown: 0.0,
                        peak: arts::jump_height((to - here).xz().length(), 0.5),
                        stops: arts::flight_stops(arts.kind),
                    });
                    arts.start(making.art);
                } else if playing.struck == 1 && due(1.0 - lands, done) {
                    playing.struck = 2;
                    let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                    match (arts.kind, record) {
                        (2, Some(record)) => {
                            // The screen's quake of 45 as it lands (`0x4FF0B0` at `0x500765`).
                            commands.queue(|world: &mut World| {
                                if let Some(mut weather) =
                                    world.get_resource_mut::<crate::weather::Weather>()
                                {
                                    weather.set(opensacred_rules::weather::Event::Quake(
                                        arts::JUMP_SHAKES,
                                    ));
                                }
                            });
                            let (radius, factor) = arts::jump_quake(record, playing.level);
                            for &entity in &nearest.hostiles {
                                let Ok((_, at, mind, health)) = creatures.get(entity) else {
                                    continue;
                                };
                                let distance = to_f32(whole_distance(at.translation(), here));
                                if !mind.hostile() || health.current == 0 || distance >= radius {
                                    continue;
                                }
                                hits.write(fight::Hit {
                                    fixed: None,
                                    sure: false,
                                    poison: false,
                                    ward: false,
                                    share: factor,
                                    ..hit(entity, &playing)
                                });
                                if knocks_back(&mut dice.0, playing.level, mind) {
                                    let seconds = arts::jump_stun(mind.level);
                                    commands
                                        .entity(entity)
                                        .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                                        .entry::<Mind>()
                                        .and_modify(move |mut mind| mind.stunned(seconds));
                                }
                            }
                        }
                        (arts::VAMPIRESS_BY_NIGHT, Some(record)) => {
                            // Each hostile about her beyond the two's reach: a strike of a fixed
                            // physical damage, and held by the kicks' roll (`0x50E6C0` at
                            // `0x512F6F`).
                            let (radius, damage) = arts::claw_quake(record, playing.level);
                            for &entity in &nearest.hostiles {
                                let Ok((_, at, mind, health)) = creatures.get(entity) else {
                                    continue;
                                };
                                let distance = to_f32(whole_distance(at.translation(), here));
                                if !mind.hostile()
                                    || health.current == 0
                                    || distance >= radius + to_f32(reach(mind))
                                {
                                    continue;
                                }
                                if knocks_back(&mut dice.0, playing.level, mind) {
                                    commands
                                        .entity(entity)
                                        .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                                        .entry::<Mind>()
                                        .and_modify(|mut mind| mind.stunned(arts::CLAW_HOLD));
                                }
                                hits.write(fight::Hit {
                                    share: 1.0,
                                    fixed: Some([damage, 0.0, 0.0, 0.0]),
                                    ..hit(entity, &playing)
                                });
                            }
                        }
                        _ => {
                            if let Ok((_, _, mind, health)) = creatures.get(playing.target)
                                && mind.hostile()
                                && health.current > 0
                            {
                                hits.write(hit(playing.target, &playing));
                                sparks.write(Spark {
                                    art: making.art,
                                    by: hero,
                                    on: playing.target,
                                });
                            }
                        }
                    }
                    end(&mut commands);
                } else if finished {
                    end(&mut commands);
                }
            }
            // The multi-shot ends as it starts, without a clip.
            Style::Volley | Style::Arrow => end(&mut commands),
            // Its strikes are `blades`'; it ends with its clip.
            Style::Blades => {
                if finished {
                    end(&mut commands);
                }
            }
            Style::Smite => {
                // Its target and the hero's hostiles nearer it than the two's reach, once
                // (`0x4FF0B0`).
                if playing.struck == 0
                    && playing
                        .points
                        .first()
                        .is_some_and(|&point| due(point, done))
                {
                    playing.struck = 1;
                    // The hostiles about the target, by their distance from it (`0x4FF0B0`).
                    let target = creatures.get(playing.target).ok();
                    let within = target.map_or(0, |(_, _, mind, _)| reach(mind));
                    let centre = target.map_or(here, |(_, at, ..)| at.translation());
                    let others: Vec<(Entity, Candidate)> = nearest
                        .hostiles
                        .iter()
                        .enumerate()
                        .filter_map(|(index, &entity)| {
                            let (_, at, mind, health) = creatures.get(entity).ok()?;
                            Some((
                                entity,
                                Candidate {
                                    id: u32::try_from(index).ok()?,
                                    distance: whole_distance(at.translation(), centre),
                                    reach: reach(mind),
                                    hostile: mind.hostile() && health.current > 0,
                                },
                            ))
                        })
                        .collect();
                    let target_id = others
                        .iter()
                        .find(|(entity, _)| *entity == playing.target)
                        .map(|&(_, c)| c.id);
                    let candidates: Vec<Candidate> = others.iter().map(|&(_, c)| c).collect();
                    let mut struck: Vec<Entity> = Vec::new();
                    if target.is_some() && target_id.is_none() {
                        struck.push(playing.target);
                    }
                    for id in arts::smitten(target_id, &candidates, within) {
                        if let Some(&(entity, _)) =
                            usize::try_from(id).ok().and_then(|index| others.get(index))
                        {
                            struck.push(entity);
                        }
                    }
                    info!("making: {} strikes {} creatures", making.art, struck.len());
                    for entity in struck {
                        hits.write(hit(entity, &playing));
                        sparks.write(Spark {
                            art: making.art,
                            by: hero,
                            on: entity,
                        });
                    }
                    arts.start(making.art);
                    end(&mut commands);
                } else if finished {
                    end(&mut commands);
                }
            }
            Style::Boon
            | Style::Frenzy
            | Style::Bolts
            | Style::Glance
            | Style::Ward
            | Style::WardSpells
            | Style::Form => {
                // Its effects on the hero at its point (`0x506770`), and its regeneration.
                if playing.struck == 0
                    && playing
                        .points
                        .first()
                        .is_some_and(|&point| due(point, done))
                {
                    playing.struck = 1;
                    for &id in crate::art_sounds::objects(making.art).1 {
                        points.write(crate::sfx::PlayAt {
                            sound: crate::sfx::Of::Id(id),
                            point: transform.translation,
                            rate: (0.5, crate::sfx::Roll::None),
                            factor: 1.0,
                        });
                    }
                    let boon = arts
                        .tables
                        .weapon_arts
                        .iter()
                        .find(|a| a.id == making.art)
                        .and_then(|record| {
                            // The Gladiator's 1016 by his skill 2 and his `a0`.
                            let glance = rules.glance_factor(
                                &arts.skills,
                                combat.map_or(0.0, |combat| combat.attributes[0]),
                            );
                            boon_of(arts, record, playing.level, (glance, flight_times))
                        });
                    // The Seraphim's 1018 and 1019 end as they give their effects (`0x5038F0`).
                    let ends = boon.as_ref().is_some_and(|boon| boon.ends);
                    // An art of the launch type 2 made on a friend gives it its effects and its
                    // sights there (`0x5490B0`; 1015's target, the wards' objects at it; #3124).
                    let bearer = if launch(arts, making.art) == Some(2)
                        && creatures.contains(playing.target)
                    {
                        playing.target
                    } else {
                        hero
                    };
                    if let Some(boon) = boon {
                        info!("making: {} gives {bearer} {:?}", making.art, boon.effects);
                        // The soulcatcher's object 869 hangs at him for its effect's time,
                        // looping `DELF_SOULCATCHER_LOOP` from its start (`0x76AC5F`), its
                        // twirls over his neck (`soulcatchers`).
                        if let Some(&(_, seconds)) = boon
                            .effects
                            .iter()
                            .find(|(effect, _)| effect.id == arts::SOULCATCHER)
                        {
                            commands.entity(hero).with_child((
                                crate::hums::Hum(SOULCATCHER_LOOP),
                                crate::spells::Fades(seconds),
                                crate::soulcatchers::Catcher,
                                Transform::default(),
                            ));
                        }
                        // The Daemon's forms' object 1014 at his head for the form's time
                        // (`0x51A47B`; `demon_forms`).
                        if (1097..=1100).contains(&making.art)
                            && let Some(&(_, seconds)) = boon.effects.first()
                        {
                            commands.spawn((
                                crate::demon_forms::Form {
                                    bearer: hero,
                                    art: making.art,
                                    seconds,
                                },
                                DespawnOnExit(crate::game::AppState::InGame),
                            ));
                        }
                        // The Dwarf's war cry's sign 1150 on him for the boon's seconds
                        // (`0x51630A`; `war_cry_signs`), and its effects at half their values
                        // on each of his followers, the sign on those within 500 (#3431).
                        if making.art == 1089
                            && let Some(&(_, seconds)) = boon.effects.first()
                        {
                            crate::war_cry_signs::show(&mut commands, hero, seconds);
                            let effects = boon.effects.clone();
                            let at = transform.translation;
                            commands.queue(move |world: &mut World| {
                                crate::war_cry_signs::share(world, (hero, at), &effects);
                            });
                        }
                        // The Seraphim's energy bolts' two objects 844 at her hands for the
                        // effect's seconds (`0x504B96`, `0x504C4F`; `sera_sights`).
                        if making.art == 1019
                            && let Some(&(_, seconds)) = boon.effects.first()
                        {
                            crate::sera_sights::bolts(&mut commands, hero, seconds);
                        }
                        // The arrowproof's object 883 and the spellproof's 870 about him for
                        // their effects' seconds, made with them (`0x5085DE`, `0x50894B`;
                        // `dark_wards`).
                        if matches!(making.art, 1045 | 1046)
                            && let Some(&(_, seconds)) = boon.effects.first()
                        {
                            commands.spawn((
                                crate::dark_wards::Ward {
                                    bearer,
                                    arrows: making.art == 1045,
                                    seconds,
                                },
                                DespawnOnExit(crate::game::AppState::InGame),
                            ));
                        }
                        // The testosterone's and the adrenaline's two objects 864 at his hands'
                        // bones 21 and 22, made with their effects (`0x5080F0`, `0x5081CB`;
                        // `0x508396`, `0x508471`), each living the effects' time (the event's
                        // `+0x2C`), crackling red or yellow (`dark_sights`) and looping its sound.
                        if ADRENALINE.contains(&making.art)
                            && let Some(&(_, seconds)) = boon.effects.first()
                        {
                            for hand in ["Bip01 L Finger3", "Bip01 R Finger3"] {
                                commands.entity(hero).with_child((
                                    crate::hums::Hum(ADRENALINE_LOOP),
                                    crate::spells::Fades(seconds),
                                    Transform::default(),
                                ));
                                commands.spawn(crate::dark_sights::sight(
                                    crate::dark_sights::Sight {
                                        bearer: hero,
                                        kind: DarkSightKind::Crackle {
                                            red: making.art == ADRENALINE[0],
                                        },
                                        seconds,
                                        bone: Some(hand),
                                        follows: true,
                                    },
                                ));
                            }
                        }
                        // The frenzy's object 1019 at him, made with its effect 47 (`0x5160AA`),
                        // for the effect's time (its event's `+0x30`) and a second's fades.
                        if making.art == FRENZY
                            && let Some(&(effect, seconds)) = boon
                                .effects
                                .iter()
                                .find(|(effect, _)| effect.id == FRENZY_EFFECT)
                        {
                            // Its ribbons over his head (`dwarf_berserks`).
                            let whole = f32::from(i16::try_from(effect.whole).unwrap_or(i16::MAX));
                            crate::dwarf_berserks::show(&mut commands, hero, seconds, whole);
                            commands.entity(hero).with_child((
                                crate::hums::Hum(FRENZY_LOOP),
                                crate::spells::Fades(seconds + 1.0),
                                Transform::default(),
                            ));
                        }
                        commands
                            .entity(bearer)
                            .entry::<crate::effects::Effects>()
                            .or_default()
                            .and_modify(move |mut effects| {
                                effects.take_off(&boon.replaces);
                                for (effect, seconds) in boon.effects {
                                    effects.add(effect, seconds);
                                }
                            });
                    }
                    arts.start(making.art);
                    if ends {
                        end(&mut commands);
                    }
                }
                if finished {
                    end(&mut commands);
                }
            }
            Style::Attacke => {
                let Some(&point) = playing.points.get(playing.struck) else {
                    end(&mut commands);
                    continue;
                };
                if !due(point, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                // The Dwarf's with a gun fires at each point at its one target, alive or not, at
                // any distance (`0x514B62` to `0x514CA8`, `0x514E8F`).
                if making.art == arts::DWARF_ATTACKE {
                    fires.write(crate::sfx::Gunfire { shooter: hero });
                }
                if making.art == arts::DWARF_ATTACKE && gun_held {
                    if playing.struck == 0 {
                        arts.start(making.art);
                    }
                    if creatures.contains(playing.target) {
                        hits.write(hit(playing.target, &playing));
                    }
                    if places.contains(playing.target) {
                        sparks.write(Spark {
                            art: making.art,
                            by: hero,
                            on: playing.target,
                        });
                    }
                    playing.struck += 1;
                    if playing.struck >= playing.points.len() {
                        end(&mut commands);
                    }
                    continue;
                }
                // Its target, else the first hostile creature within the reach and 45.
                let mut found: Vec<(Entity, Candidate)> = Vec::new();
                let candidate =
                    |index: usize, at: &GlobalTransform, mind: &Mind, health: &Health| Candidate {
                        id: u32::try_from(index).unwrap_or(u32::MAX),
                        distance: whole_distance(at.translation(), here),
                        reach: reach(mind),
                        hostile: mind.hostile() && health.current > 0,
                    };
                for (entity, at, mind, health) in &creatures {
                    let index = found.len();
                    found.push((entity, candidate(index, at, mind, health)));
                }
                let current = found
                    .iter()
                    .find(|(entity, _)| *entity == playing.target)
                    .map(|&(_, c)| c);
                let others: Vec<Candidate> = found.iter().map(|&(_, c)| c).collect();
                let Some(chosen) = attacke_target(current, &others)
                    .and_then(|id| usize::try_from(id).ok())
                    .and_then(|index| found.get(index))
                    .map(|&(entity, _)| entity)
                else {
                    end(&mut commands);
                    continue;
                };
                if playing.struck == 0 {
                    arts.start(making.art);
                }
                hits.write(hit(chosen, &playing));
                sparks.write(Spark {
                    art: making.art,
                    by: hero,
                    on: chosen,
                });
                // The Gladiator's bare-handed one holds its creature 3 s at least, as a knock does,
                // without ending what it does (the slot 210, `0x502711`, #3429).
                if making.art == opensacred_rules::casts::GLADIATORS_ATTACKE
                    && holding.is_none_or(|holding| holding.0.class == 0)
                {
                    commands
                        .entity(chosen)
                        .entry::<Mind>()
                        .and_modify(|mut mind| mind.held_at_least(BARE_ATTACKE_HOLD));
                }
                playing.target = chosen;
                playing.struck += 1;
                if playing.struck >= playing.points.len() {
                    end(&mut commands);
                }
            }
            Style::Kick => {
                // One strike at its point, which may knock its target back; its clip plays on
                // to its end (#650). The Wood Elf's 1064 and the generic 1009 have none and
                // strike nothing, their regeneration not started (#3121).
                if playing.struck == 0 && playing.points.first().is_some_and(|&at| due(at, done)) {
                    playing.struck = 1;
                    if let Ok((_, _, mind, health)) = creatures.get(playing.target)
                        && health.current > 0
                    {
                        hits.write(hit(playing.target, &playing));
                        sparks.write(Spark {
                            art: making.art,
                            by: hero,
                            on: playing.target,
                        });
                        let knocks = knocks_back(&mut dice.0, playing.level, mind);
                        let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                        if knocks
                            && let Some(left) = record.and_then(|record| {
                                arts::knockback(record, playing.level, mind.level)
                            })
                        {
                            hit_clip(&mut commands, playing.target, to_f32(left));
                            commands
                                .entity(playing.target)
                                .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                                .insert(Knocked {
                                    by: hero,
                                    from: here,
                                    left: to_f32(left),
                                    held: 0.0,
                                    throw: None,
                                    unheld: false,
                                });
                        }
                    }
                    arts.start(making.art);
                }
                if finished {
                    end(&mut commands);
                }
            }
            Style::Hardhit => {
                if due(hardhit_point(&playing.points), done) {
                    // The Dwarf's fires a gun held at its point (`0x515298`).
                    if making.art == arts::DWARF_HARDHIT {
                        fires.write(crate::sfx::Gunfire { shooter: hero });
                    }
                    if creatures.get(playing.target).is_ok() {
                        hits.write(hit(playing.target, &playing));
                        sparks.write(Spark {
                            art: making.art,
                            by: hero,
                            on: playing.target,
                        });
                    }
                    arts.start(making.art);
                    end(&mut commands);
                } else if finished {
                    end(&mut commands);
                }
            }
            Style::Rundum | Style::Whirl => {
                // The Dwarf's with a gun fires at each of its clip's hit points, its target first,
                // the gathered three in turn, the fifth at no one, turning to each, and ends after
                // its last, its clip playing on (`0x515C21` to `0x516028`, #3122); each shot at
                // the world's first tick after its point, as its executor runs (seen, #3607).
                if making.art == arts::DWARF_RUNDUM && gun_held {
                    while tick.seconds.is_some()
                        && playing
                            .points
                            .get(playing.fired)
                            .is_some_and(|&point| due(point, done))
                    {
                        let shot = playing.fired;
                        let whom =
                            arts::gun_rundum_shot(shot, Some(playing.target), &playing.gathered)
                                .filter(|&whom| places.contains(whom));
                        // At an object of the shot's word, alive or not, at any distance; with
                        // none toward the order's point, striking nothing.
                        let at = whom
                            .and_then(|whom| places.get(whom).ok())
                            .map_or(playing.point, GlobalTransform::translation);
                        let toward = (at - here).with_y(0.0);
                        if toward.length() > f32::EPSILON {
                            transform.rotation = hero::facing(toward);
                        }
                        info!(
                            "making: {}'s shot {shot} with a gun at {whom:?}, {} gathered",
                            making.art,
                            playing.gathered.len()
                        );
                        fires.write(crate::sfx::Gunfire { shooter: hero });
                        if let Some(whom) = whom {
                            if creatures.contains(whom) {
                                hits.write(hit(whom, &playing));
                            }
                            sparks.write(Spark {
                                art: making.art,
                                by: hero,
                                on: whom,
                            });
                        }
                        // The first starts its regeneration, of the group 0 (`0x515FB3`).
                        if shot == 0 {
                            arts.start_with(making.art, 1.0, None);
                        }
                        playing.fired += 1;
                    }
                    if finished || playing.fired >= playing.points.len() {
                        end(&mut commands);
                    }
                    continue;
                }
                // Without one its regeneration below 0.6 of its clip left, once, of the group 0
                // (`0x51556D`, #3011).
                if making.art == arts::DWARF_RUNDUM
                    && !playing.regenerated
                    && due(1.0 - arts::DWARF_RUNDUM_STARTS, done)
                {
                    arts.start_with(making.art, 1.0, None);
                    playing.regenerated = true;
                }
                if finished {
                    end(&mut commands);
                    continue;
                }
                // It looks for the creatures about its bones as its executor runs, on the
                // world's tick (**hypothesis**, #617), the Gladiator's from 0.3 of its clip.
                if !arts::rundum_looks(arts.kind, done) || tick.seconds.is_none() {
                    continue;
                }
                // The tables apart from the arts, whose regeneration each strike starts.
                let tables = std::sync::Arc::clone(&arts.tables);
                let Some(record) = tables.weapon_arts.iter().find(|a| a.id == making.art) else {
                    continue;
                };
                // About the effects' bone of the item in the right hand, else the hand's own,
                // its radius truncated; then about the bone of the item in the left hand, its
                // radius whole (docs/re/arts.md, "A `Rundum`'s run", #3120).
                let held_bone = |half: usize| {
                    wielded
                        .iter()
                        .find(|(_, hand, by)| by.0 == hero && hand.half() == half)
                        .and_then(|(model, ..)| {
                            bone(model, &WEAPON_BONES, &children, (&bones, &worn))
                        })
                };
                let hand = || bone(hero, &[HAND_BONE], &children, (&bones, &worn));
                for (point, truncated) in rundum_points((held_bone(1), hand), held_bone(0), here) {
                    let radius = |reach| {
                        let radius = rundum_radius(record, playing.level, reach);
                        if truncated { radius.trunc() } else { radius }
                    };
                    let found: Vec<(Entity, Candidate)> = creatures
                        .iter()
                        .enumerate()
                        .map(|(index, (entity, at, mind, health))| {
                            (
                                entity,
                                Candidate {
                                    id: u32::try_from(index).unwrap_or(u32::MAX),
                                    distance: whole_distance(at.translation(), point),
                                    reach: reach(mind),
                                    hostile: mind.hostile() && health.current > 0,
                                },
                            )
                        })
                        .collect();
                    let last = playing.last.and_then(|last| {
                        found
                            .iter()
                            .find(|(entity, _)| *entity == last)
                            .map(|&(_, c)| c.id)
                    });
                    let candidates: Vec<Candidate> = found.iter().map(|&(_, c)| c).collect();
                    if let Some(&(chosen, _)) = rundum_target(&candidates, radius, last)
                        .and_then(|id| usize::try_from(id).ok())
                        .and_then(|index| found.get(index))
                    {
                        hits.write(hit(chosen, &playing));
                        sparks.write(Spark {
                            art: making.art,
                            by: hero,
                            on: chosen,
                        });
                        playing.last = Some(chosen);
                        // A fifth of its time again for each new creature struck, by its class
                        // (`0x50F318`, `0x50758A`, `0x505434`, `0x50BAC7`, `0x50DA5D`; #2995).
                        if arts::rundum_hops(making.art) {
                            arts.start_with(making.art, arts::RUNDUM_HOP, Some(making.art));
                        }
                        // The whirling blow knocks back each creature it strikes by a kick's roll
                        // (`0x5038F0` at `0x50522A`).
                        if playing.style == Style::Whirl
                            && let Ok((_, _, mind, _)) = creatures.get(chosen)
                            && knocks_back(&mut dice.0, playing.level, mind)
                        {
                            commands
                                .entity(chosen)
                                .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                                .insert(Knocked {
                                    by: hero,
                                    from: here,
                                    left: to_f32(arts::whirl_knockback(record, playing.level)),
                                    held: 0.0,
                                    throw: None,
                                    unheld: false,
                                });
                        }
                    }
                }
            }
        }
    }
}

/// Whether a line of sight or of flight leads from `here` to `there`, tested on the level `level`:
/// the straight way of the flyer's flag (`0x4FCC80`), over the water and the fences, which the
/// Dwarf's gun arts ask at their order, at their first run and of the creatures his `Rundum`
/// gathers (`0x4FD40D`, `0x51852B`, `0x518760`).
fn in_sight(
    here: Vec3,
    there: Vec3,
    level: crate::terrain::Level,
    heights: &crate::terrain::Heights,
) -> bool {
    crate::paths::straight(here.xz(), there.xz(), |cell| {
        heights.flown_onto(cell, level)
    })
}

/// The creatures that the Dwarf's `Rundum` with a gun, made from `here` on the level `level` at
/// `target`, gathers of the hero's list of hostiles `listed` (`crowd::Nearest`, nearest first:
/// each creature with its place, its reach with the hero and whether it lives and he may strike
/// it; `arts::gun_rundum_gathers`): up to three others alive that he may strike, nearer him than
/// 1,000 and in his line of sight ([`in_sight`]), in the list's order.
fn gathers(
    (here, level, target): (Vec3, crate::terrain::Level, Entity),
    listed: impl IntoIterator<Item = (Entity, Vec3, u32, bool)>,
    heights: &crate::terrain::Heights,
) -> Vec<Entity> {
    let listed: Vec<(Entity, Vec3, Candidate)> = listed
        .into_iter()
        .enumerate()
        .map(|(index, (entity, at, reach, hostile))| {
            let candidate = Candidate {
                id: u32::try_from(index).unwrap_or(u32::MAX),
                distance: whole_distance(at, here),
                reach,
                hostile,
            };
            (entity, at, candidate)
        })
        .collect();
    let of = |id: u32| usize::try_from(id).ok().and_then(|index| listed.get(index));
    let target = listed
        .iter()
        .find(|(entity, ..)| *entity == target)
        .map(|(_, _, candidate)| candidate.id);
    let candidates: Vec<Candidate> = listed.iter().map(|&(_, _, candidate)| candidate).collect();
    let sees = |candidate: &Candidate| {
        of(candidate.id).is_some_and(|&(_, at, _)| in_sight(here, at, level, heights))
    };
    arts::gun_rundum_gathers(target, &candidates, sees)
        .into_iter()
        .filter_map(|id| of(id).map(|&(entity, ..)| entity))
        .collect()
}

/// The points about which a `Rundum` looks for a creature in turn, and whether the radius of each
/// is truncated (docs/re/arts.md, "A `Rundum`'s run", #3120): the effects' bone of the item in the
/// right hand (`right`), else the hand's own (`hand`), else its place `here`, truncated; then the
/// bone of the item in the left hand (`left`), whole, where it has one.
fn rundum_points(
    (right, hand): (Option<Vec3>, impl FnOnce() -> Option<Vec3>),
    left: Option<Vec3>,
    here: Vec3,
) -> impl Iterator<Item = (Vec3, bool)> {
    let right = right.or_else(hand).unwrap_or(here);
    std::iter::once((right, true)).chain(left.map(|left| (left, false)))
}

/// Whether an art of the level in play `level` may knock back the creature of `mind` at all
/// (`0x4FC9D0` above 0, `arts::knockback_chance`), as the club arrow asks.
pub(crate) fn may_knock(level: u32, mind: &minds::Mind) -> bool {
    arts::knockback_chance(
        level,
        (mind.creature, mind.level),
        mind.flags & BOSS != 0,
        mind.big,
    ) > 0.0
}

/// Whether a kick's roll of 0 to 99, an art's of the level in play `level`, knocks back the
/// creature of `mind` (`0x4FC9D0`, `arts::knockback_chance`).
pub(crate) fn knocks_back(dice: &mut Rng, level: u32, mind: &Mind) -> bool {
    let chance = arts::knockback_chance(
        level,
        (mind.creature, mind.level),
        mind.flags & BOSS != 0,
        mind.big,
    );
    to_f32(dice.below(100)) < chance
}

/// The item in a hero's hand that its hurled blades strike with (`0x5038F0`): the one in its
/// right hand, else its left's (`+0x1D8`, `+0x1D4`), and its size (`items.pak +0x14`, 50 for 0,
/// `0x428CE0`); a hero without one strikes nothing with them.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Blade {
    pub right: bool,
    pub size: f32,
}

/// The bones whose point the hurled blades strike from, by the hand of the hero's [`Blade`]: the
/// hero's own socket of the item in that hand, else the hand.
fn blade_bones(blade: Blade) -> [&'static str; 2] {
    if blade.right {
        ["Bone_weapon_01", HAND_BONE]
    } else {
        ["Bone_weapon_02", "Bip01 L Hand"]
    }
}

/// The hurled blades' strikes (the Seraphim's 1054, `0x5038F0`), at each of the world's ticks
/// as its executor runs: the point of the hero's socket of its [`Blade`] (`0x44CA30` of
/// `Bone_weapon_01` or `Bone_weapon_02`), and the living creature nearest it that it touches by
/// the two sizes (`arts::blade_touches`, the creature's `arrows::Size`), where it is hostile and
/// not the one struck last, takes a strike of the art's level and factor and becomes the last.
/// The original takes the first object of its sectors' lists that the point touches
/// (`0x600340`), of any kind, and strikes nothing that tick where that is the last struck or one
/// it may not strike (`0x549000`); the nearest creature stands for the first.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn blades(
    tick: Res<WorldTick>,
    mut heroes: Query<(Entity, &mut Playing, Option<&Blade>, Option<&Making>), Without<Dies>>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &Mind,
            &Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<Dies>, Without<Body>),
    >,
    (children, bones, worn): (Query<&Children>, Query<(&Name, &GlobalTransform)>, Worn),
    (mut hits, mut sparks): (MessageWriter<fight::Hit>, MessageWriter<Spark>),
) {
    if tick.seconds.is_none() {
        return;
    }
    for (hero, mut playing, blade, making) in &mut heroes {
        if playing.style != Style::Blades {
            continue;
        }
        let Some(blade) = blade.copied() else {
            continue;
        };
        let Some(point) = bone(hero, &blade_bones(blade), &children, (&bones, &worn)) else {
            continue;
        };
        let touched = creatures
            .iter()
            .filter(|&(.., health, _)| health.current > 0)
            .map(|(entity, at, mind, _, size)| {
                let distance = (at.translation() - point).xz().length();
                let size = size.map_or(opensacred_rules::arrows::SIZE, |size| size.0);
                (entity, mind.hostile(), distance, size)
            })
            .filter(|&(.., distance, size)| arts::blade_touches(distance, size, blade.size))
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((entity, true, ..)) = touched
            && Some(entity) != playing.last
        {
            hits.write(fight::Hit {
                fixed: None,
                sure: false,
                poison: false,
                ward: false,
                hurled: false,
                object: false,
                mount: false,
                attacker: hero,
                target: entity,
                share: playing.factor,
                ranged: false,
                art: Some(playing.level),
            });
            if let Some(making) = making {
                sparks.write(Spark {
                    art: making.art,
                    by: hero,
                    on: entity,
                });
            }
            playing.last = Some(entity);
        }
    }
}

/// A hero's or a creature's flight of its jump (`0x44DED0`, `0x44DFE0`; the flowers' leap,
/// `creature_casts`, #2231): from `from` to `to` on the ground in
/// `time` seconds, `flown` of them gone, its height on the arc of `arts::arc` of the height
/// `peak` at its middle above the ground under it; and whether it `stops` at a cell of its way
/// that it may not pass (`arts::flight_stops`, by the flier's type).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Leap {
    pub from: Vec3,
    pub to: Vec3,
    pub time: f32,
    pub flown: f32,
    pub peak: f32,
    pub stops: bool,
}

/// The art whose hit sparks a flight that a cell stops makes at its flier, the Dark Elf's
/// `Hardhit` (the event's `+0x3C` `0x42A`, `0x44E68D`).
const FLIGHT_STOP_SPARKS: u16 = 1066;

/// Flies each hero and each creature in its jump (`0x44DFE0`): along the line to its landing in
/// its flight's time, on its arc, and lands it there; one that stops ends its flight at the first
/// of the parts of its way that it tests each step (`arts::flight_samples`) whose cell it may not
/// pass (`terrain::Heights::passes`, `0x636A60`), on the ground where the last it passed put it,
/// with the Dark Elf's yellow hit sparks at its neck whatever its class (864 by the FX event
/// `0x59` of the bone 3 and the art 1066, `0x44E662`; #3203).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their components"
)]
pub(crate) fn leap(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<crate::terrain::Heights>,
    mut leapers: Query<(
        Entity,
        &mut Transform,
        Option<&Body>,
        Option<&crate::crowd::InTheWay>,
        &mut Leap,
    )>,
    mut sparks: MessageWriter<crate::hit_sparks::Spark>,
) {
    for (hero, mut transform, body, way, mut leap) in &mut leapers {
        let level = body.map_or_else(|| way.and_then(|way| way.level), Body::level);
        leap.flown += time.delta_secs();
        let t = if leap.time > 0.0 {
            (leap.flown / leap.time).min(1.0)
        } else {
            1.0
        };
        if leap.stops && t < 1.0 {
            let mut passed = None;
            let stopped = arts::flight_samples(t).any(|part| {
                let at = leap.from.lerp(leap.to, part);
                let passes = heights.passes(crate::paths::cell_of(at.x, at.z), level);
                if passes {
                    passed = Some(at);
                }
                !passes
            });
            if stopped {
                let at = passed.unwrap_or(transform.translation);
                let y = heights.standing(at.x, at.z, level).unwrap_or(at.y);
                transform.translation = Vec3::new(at.x, y, at.z);
                commands.entity(hero).remove::<Leap>();
                sparks.write(crate::hit_sparks::Spark {
                    art: FLIGHT_STOP_SPARKS,
                    by: hero,
                    on: hero,
                });
                debug!("making: {hero}'s flight ends at a cell it may not pass");
                continue;
            }
        }
        let ground = leap.from.lerp(leap.to, t);
        let y = heights
            .standing(ground.x, ground.z, level)
            .unwrap_or(ground.y);
        let up = crate::iso::height(arts::arc(leap.peak, t) * crate::iso::PIXELS_PER_MODEL_HEIGHT);
        transform.translation = Vec3::new(ground.x, y + up, ground.z);
        if t >= 1.0 {
            commands.entity(hero).remove::<Leap>();
        }
    }
}

/// A creature that a kick knocks back (`0x594D12`, the flag `0x20000` of `+0x200`): it slides
/// away from its kicker `by`, from its place `from` where it is gone, at
/// `arts::KNOCKBACK_SPEED` until it has gone `left` (`+0x24D`), doing nothing else; its walk
/// and strike are ended as it is knocked. One thrown (`throw`, the Dwarf's 1083) flies its flight
/// instead, striking what it meets, and is held as a stunned creature is.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Knocked {
    pub by: Entity,
    pub from: Vec3,
    pub left: f32,
    /// How long it is held at least beyond the event's own 3 s (the roar's,
    /// `opensacred_rules::minds::Mind::knocked`), 0 for the others; a thrown one's whole hold.
    pub held: f32,
    pub throw: Option<arts::Flight>,
    /// Whether it is held not at all, the cannon's (`opensacred_rules::minds::Mind::knocked_unheld`,
    /// #3430).
    pub unheld: bool,
}

/// A creature's flags word's bit of a boss (`+0x1F4` bit `0x80000`).
const BOSS: u32 = 0x80000;

/// Plays the hit clip of the creature `entity` that a knock of `strength` reaches, its slot 175 at
/// three times the rate, where the strength is above 10 (`0x593770`;
/// `opensacred_rules::arts::knock_plays_hit_clip`, #3141): the kicks', the roar's, the cannon's
/// and the blast's.
pub(crate) fn hit_clip(commands: &mut Commands, entity: Entity, strength: f32) {
    if arts::knock_plays_hit_clip(strength) {
        commands.entity(entity).insert(crate::npcs::ScriptClip::hit(
            arts::HIT_CLIP,
            arts::HIT_CLIP_RATE,
        ));
    }
}

/// Slides each knocked-back creature away from its kicker at every frame (`0x594D12`): a step of
/// `arts::KNOCKBACK_SPEED` times the frame's time, at most `arts::KNOCKBACK_STEP` of it, taken
/// from what is left; the step moves it where it may stand on its level, else it stays; with
/// nothing left it is free again.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn knock(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<crate::terrain::Heights>,
    places: Query<&GlobalTransform>,
    mut knocked: Query<(
        Entity,
        &mut Transform,
        &mut Knocked,
        &crate::crowd::InTheWay,
        Option<&crate::arrows::Size>,
    )>,
    others: Query<(Entity, &Transform, &Mind, &Health), (Without<Knocked>, Without<Dies>)>,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs().min(arts::KNOCKBACK_STEP);
    for (entity, mut transform, mut knock, way, size) in &mut knocked {
        if let Some(mut flight) = knock.throw {
            // A thrown creature flies its flight; a cell it may not enter stops it, the ground
            // ends its flight (`0x44DFE0`).
            let ground = crate::backbreaker::fly(
                &mut transform,
                &mut flight,
                &heights,
                way.level,
                time.delta_secs(),
            );
            // The first creature it meets, alive, not big, not flying, nearer than the two
            // sizes: struck by its thrower, his ordinary strike, which lands without a roll as his
            // art has ended (`0x56B130`), thrown on along its way, held 2 s; it keeps 0.9 of its
            // speed (`0x44DFE0`'s blows).
            let own = size.map_or(crate::arrows::size(0), |size| size.0);
            let here = transform.translation.xz();
            if let Some((other, at)) = others
                .iter()
                .filter(|&(e, _, mind, health)| {
                    e != entity && e != knock.by && !mind.big && health.current > 0
                })
                .map(|(e, at, ..)| (e, at.translation))
                .find(|(_, at)| (at.xz() - here).length() < own + crate::arrows::size(0))
            {
                let to_it = (at.xz() - here).into();
                debug!("making: {entity} thrown knocks {other} over");
                hits.write(fight::Hit {
                    attacker: knock.by,
                    target: other,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: None,
                    sure: true,
                    poison: false,
                    ward: false,
                    hurled: false,
                    object: false,
                    mount: false,
                });
                // Its dying clip at the rate 1.0 (`0x44DFE0`'s blows, #3141).
                commands
                    .entity(other)
                    .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                    .insert((
                        Knocked {
                            by: knock.by,
                            from: knock.from,
                            left: 0.0,
                            held: arts::BOWL_HOLD,
                            throw: Some(arts::bowled(flight.velocity, to_it)),
                            unheld: false,
                        },
                        crate::npcs::ScriptClip::fall(arts::BOWLED_CLIP),
                    ));
                // Its burning light for its hold's 2 s (the blows' object 816, #3142).
                crate::holding_sights::burn(&mut commands, other, arts::BOWL_HOLD);
                flight.velocity = flight.velocity.map(|v| v * arts::BOWL_KEEPS);
            }
            if flight.landed() {
                transform.translation.y = ground;
                commands.entity(entity).remove::<Knocked>();
            } else {
                knock.throw = Some(flight);
            }
            continue;
        }
        if knock.left <= 0.0 {
            commands.entity(entity).remove::<Knocked>();
            continue;
        }
        let from = places
            .get(knock.by)
            .map_or(knock.from, GlobalTransform::translation);
        let away = (transform.translation - from)
            .with_y(0.0)
            .normalize_or_zero();
        let step = arts::KNOCKBACK_SPEED * dt;
        knock.left -= step;
        let to = transform.translation + away * step;
        let cell = crate::paths::cell_of(to.x, to.z);
        if heights.walkable_on((cell.0, cell.1), way.level) {
            transform.translation.x = to.x;
            transform.translation.z = to.z;
        }
    }
}

/// A creature knocked back leaves its fight and is held until its timer fires with its slide
/// over (`0x531AF7`, `opensacred_rules::minds::Mind::knocked`), at each knockback.
fn held(knocked: On<Insert<Knocked>>, mut minds: Query<(&mut Mind, &Knocked)>) {
    if let Ok((mut mind, knock)) = minds.get_mut(knocked.entity) {
        if knock.throw.is_some() {
            mind.stunned(knock.held);
        } else if knock.unheld {
            mind.knocked_unheld();
        } else {
            mind.knocked(knock.held);
        }
    }
}

/// The sounds of the Dwarf's guns' objects at him, each once as it starts: the flames' 1016
/// `DWARF_FLAMMENWERFER` (`0x516C36`, `0x7B3EFB`), the mortar's grenade's 1018
/// `DWARF_MOERSER_ABSCHUSS` (`0x516A4A`, `0x7B577E`) and the cannon's flash's 1017
/// `DWARF_KANONE_ABSCHUSS` (`0x5172F0`, `0x7B4AD8`); their sights are `dwarf_fires`'.
const FLAMER_SOUND: u32 = 2113;
const MORTAR_SOUND: u32 = 2114;
const CANNON_SOUND: u32 = 2116;
/// How long the Gladiator's bare-handed `Attacke` holds its creature at least, in seconds (the
/// float 3.0 at `0x890768`, `0x502711`).
const BARE_ATTACKE_HOLD: f32 = 3.0;

/// A gun's object's sound at `at`.
fn gun_sound(points: &mut MessageWriter<crate::sfx::PlayAt>, sound: u32, at: Vec3) {
    points.write(crate::sfx::PlayAt {
        sound: crate::sfx::Of::Id(sound),
        point: at,
        rate: (0.5, crate::sfx::Roll::None),
        factor: 1.0,
    });
}

/// A whole number of world units as a float.
fn to_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

/// The hero's making of its arts.
pub struct MakingPlugin;

/// The Seraphim's whirling blow, whose bit `0x800000` spins her heading while it runs.
const WHIRLING_BLOW: u16 = 1020;
/// How fast it spins her heading, in degrees a second (`0x890B54`).
const WHIRL_SPIN: f32 = 1_000.0;

/// Spins the heading of a hero who makes the whirling blow 1020 by [`WHIRL_SPIN`] a second of the
/// frame's time while it runs, as its bit `0x800000` turns her at each frame of her animation's
/// step (`0x594C8F`; docs/re/arts.md, "The Seraphim's", #3435).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's query carries its filters"
)]
fn spin(
    time: Res<Time>,
    mut heroes: Query<(&mut Transform, &Making), (With<Playing>, With<Body>)>,
) {
    for (mut transform, making) in &mut heroes {
        if making.art == WHIRLING_BLOW {
            transform.rotate_y(WHIRL_SPIN.to_radians() * time.delta_secs());
        }
    }
}

impl Plugin for MakingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selected>()
            .insert_resource(Dice(crate::seed::dice(2)))
            .add_systems(
                Update,
                (
                    aim.in_set(Control)
                        .in_set(crate::frame::OnWorld)
                        .before(hero::steer),
                    make.in_set(Made)
                        .after(hero::steer)
                        .before(hero::take_orders),
                    blades.after(make),
                    leap.after(make),
                    spin.after(make),
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            )
            .add_systems(Update, knock.in_set(Unpaused).in_set(Step::HeroMove))
            .add_observer(held);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_knock_above_10_plays_the_hit_clip_and_a_weaker_none() {
        let mut world = World::new();
        let strong = world.spawn_empty().id();
        let weak = world.spawn_empty().id();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, &world);
            hit_clip(&mut commands, strong, 1_000.0);
            hit_clip(&mut commands, weak, 10.0);
        }
        queue.apply(&mut world);
        assert!(world.get::<crate::npcs::ScriptClip>(strong).is_some());
        assert!(world.get::<crate::npcs::ScriptClip>(weak).is_none());
    }

    #[test]
    fn a_rundum_looks_about_the_right_blade_then_the_left_one() {
        let (right, hand, left, here) = (Vec3::X, Vec3::Y, Vec3::Z, Vec3::ONE);
        let points = |r, h: Option<Vec3>, l| rundum_points((r, || h), l, here).collect::<Vec<_>>();
        assert_eq!(
            points(Some(right), Some(hand), Some(left)),
            [(right, true), (left, false)]
        );
        // No weapon's bone in the right hand: the hand's own; the left's alone without its hand.
        assert_eq!(points(None, Some(hand), None), [(hand, true)]);
        assert_eq!(
            points(None, None, Some(left)),
            [(here, true), (left, false)]
        );
    }

    /// Sector (0, 0) walked on, but for the column of cells 3, open at the row `gap` if any.
    fn column_walled(gap: Option<usize>) -> crate::terrain::Heights {
        use opensacred_data::world::{Cell, SIDE, Sector};
        let cell = |ground| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        let cells = (0..SIDE * SIDE)
            .map(|k| {
                let wall = k % SIDE == 3 && Some(k / SIDE) != gap;
                cell(if wall { 0x41 } else { 0x40 })
            })
            .collect();
        let mut heights = crate::terrain::Heights::default();
        heights.add(&Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        });
        heights
    }

    #[test]
    fn an_art_on_an_enemy_wants_a_line_of_sight_or_a_way() {
        let at = |x: f32, y: f32| crate::iso::ground(x + 0.5, y + 0.5);
        let walled = column_walled(None);
        // On the same side, the line.
        assert!(enemy_reached((at(5.0, 5.0), None), at(9.0, 5.0), &walled));
        // Beyond the wall, neither a line nor a way (#3361).
        assert!(!enemy_reached((at(1.0, 5.0), None), at(6.0, 5.0), &walled));
        // A gap down the wall: no line, but a way.
        let gapped = column_walled(Some(20));
        assert!(enemy_reached((at(1.0, 5.0), None), at(6.0, 5.0), &gapped));
    }

    #[test]
    fn the_dwarfs_gun_rundum_gathers_the_hostiles_in_his_sight() {
        let at = |x: f32, y: f32| crate::iso::ground(x + 0.5, y + 0.5);
        let walled = column_walled(None);
        let mut world = World::new();
        let [target, behind, dead, far, near, nearer, next] =
            std::array::from_fn(|_| world.spawn_empty().id());
        let here = at(5.0, 5.0);
        // Nearest first: its target, one behind the wall, one dead, one 1,000 off or more, and
        // three in his sight, of which the first two fill the slots left.
        let listed = [
            (target, at(6.0, 5.0), 60, true),
            (behind, at(1.0, 5.0), 60, true),
            (dead, at(7.0, 5.0), 60, false),
            (near, at(8.0, 5.0), 60, true),
            (nearer, at(9.0, 5.0), 60, true),
            (next, at(10.0, 5.0), 60, true),
            (far, here + Vec3::X * 1_000.0, 60, true),
        ];
        assert_eq!(
            gathers((here, None, target), listed, &walled),
            [near, nearer, next]
        );
        // The wall gapped, the one behind it is in sight.
        let gapped = column_walled(Some(5));
        assert_eq!(
            gathers((here, None, target), listed, &gapped),
            [behind, near, nearer]
        );
        // Only the target listed: none.
        assert!(gathers((here, None, target), listed.into_iter().take(1), &walled).is_empty());
    }

    #[test]
    fn a_flight_that_a_wall_stops_sparks_at_its_flier() {
        use opensacred_data::world::{Cell, SIDE, Sector};
        use std::time::Duration;
        #[derive(Resource, Default)]
        struct Seen(Vec<crate::hit_sparks::Spark>);
        fn seen(mut sparks: MessageReader<crate::hit_sparks::Spark>, mut seen: ResMut<Seen>) {
            seen.0.extend(sparks.read().copied());
        }
        // The ground of sector (0, 0) walked on, but for the column of cells 3.
        let cell = |ground| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        let cells = (0..SIDE * SIDE)
            .map(|k| cell(if k % SIDE == 3 { 0x41 } else { 0x40 }))
            .collect();
        let mut heights = crate::terrain::Heights::default();
        heights.add(&Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(50),
            ))
            .insert_resource(heights)
            .init_resource::<Seen>()
            .add_message::<crate::hit_sparks::Spark>()
            .add_systems(Update, (leap, seen).chain());
        let at = |cells: f32| Vec3::new(cells * crate::iso::CELL, 0.0, 30.0);
        // A hero's flight from the column 1 to the 5, over the wall, stops there with the sparks;
        // one within the open ground lands with none.
        for (to, sparked) in [(5.5, true), (2.5, false)] {
            app.world_mut().resource_mut::<Seen>().0.clear();
            let flier = app
                .world_mut()
                .spawn((
                    Transform::from_translation(at(1.5)),
                    Leap {
                        from: at(1.5),
                        to: at(to),
                        time: 0.5,
                        flown: 0.0,
                        peak: 0.0,
                        stops: true,
                    },
                ))
                .id();
            for _ in 0..15 {
                app.update();
            }
            let expected = if sparked {
                vec![crate::hit_sparks::Spark {
                    art: FLIGHT_STOP_SPARKS,
                    by: flier,
                    on: flier,
                }]
            } else {
                Vec::new()
            };
            assert_eq!(app.world().resource::<Seen>().0, expected);
            // Stopped short of the wall, or landed on its point.
            let x = app.world().get::<Transform>(flier).map(|t| t.translation.x);
            let landed = |x: f32| (x - to * crate::iso::CELL).abs() < 1.0;
            assert!(x.is_some_and(|x| x < 3.0 * crate::iso::CELL && landed(x) != sparked));
        }
    }

    #[test]
    fn a_wearers_bone_is_its_own_not_its_worn_items() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let at = |y: f32| GlobalTransform::from_translation(Vec3::Y * y);
        let wearer = world.spawn(GlobalTransform::IDENTITY).id();
        world.spawn((Name::new(HAND_BONE), at(60.0), ChildOf(wearer)));
        // The worn item's node of the same name, at its rest pose, its last child, met first.
        let piece = world
            .spawn((
                crate::npcs::WornBy(wearer),
                GlobalTransform::IDENTITY,
                ChildOf(wearer),
            ))
            .id();
        world.spawn((Name::new(HAND_BONE), at(-36.0), ChildOf(piece)));
        let found = world
            .run_system_once(
                move |children: Query<&Children>,
                      bones: Query<(&Name, &GlobalTransform)>,
                      worn: Worn| {
                    bone(wearer, &[HAND_BONE], &children, (&bones, &worn))
                },
            )
            .ok()
            .flatten();
        assert_eq!(found, Some(Vec3::Y * 60.0));
    }

    #[test]
    fn the_heroes_arts_play_their_models_clips_with_their_hit_points() {
        let slots = Slots::of([
            (211, "SERA_SPECIAL_MULTI_1H"),
            (153, "SERA_ATTACK_SPECIAL10"),
        ]);
        let points = HashMap::from([(
            "SERA_SPECIAL_MULTI_1H".to_owned(),
            vec![0.137, 0.228, 0.316, 0.454, 0.577, 0.726, 0.747, 0.0],
        )]);
        let clips = clips(1, 1, Some(&slots), &points);
        let attacke = clips.iter().find(|c| c.style == Style::Attacke).unwrap();
        assert_eq!(attacke.clip.as_deref(), Some("SERA_SPECIAL_MULTI_1H"));
        assert_eq!((attacke.points.len(), attacke.rate), (7, 0.73));
        let rundum = clips.iter().find(|c| c.style == Style::Rundum).unwrap();
        assert!(rundum.points.is_empty());
        // Her `Hardhit` plays one of the attack's clips.
        let hardhit = clips.iter().find(|c| c.style == Style::Hardhit).unwrap();
        assert_eq!(hardhit.clip, None);
        // The Mage, whose weapon arts' executor is not read, has none of those; his spells play
        // the clip of their slot whatever he holds, at their own rate (#872).
        assert!(super::clips(3, 1, Some(&slots), &points).is_empty());
        let mage = Slots::of([(155, "MAGE_CAST_MAGIC02"), (161, "MAGE_CAST_MAGIC08")]);
        let casts = super::clips(3, 4, Some(&mage), &points);
        let fireball = arts::style(spells::FIREBALL).unwrap();
        let cast = casts.iter().find(|c| c.style == fireball).unwrap();
        assert_eq!(cast.clip.as_deref(), Some("MAGE_CAST_MAGIC02"));
        assert!(cast.fixed && cast.rate == 1.0);
        assert_eq!(
            casts.iter().map(|c| c.style).collect::<Vec<_>>(),
            [Style::Cast(155), Style::Cast(161)]
        );
    }

    #[test]
    fn the_mortar_wants_its_point_on_the_makers_floor_and_a_straight_way_in_a_dungeon() {
        let cell = crate::iso::CELL;
        let here = Vec3::new(0.5, 0.0, 0.5) * cell;
        let point = Vec3::new(6.5, 0.0, 0.5) * cell;
        // A wall across the line at the cell 3.
        let walkable = |(x, _): crate::paths::Cell| x != 3;
        let floor = Some((1, 2, 0));
        // Outside a dungeon the shell flies over the wall.
        assert!(mortar_reaches((here, None), (point, None), false, walkable));
        // In one it wants a straight way.
        assert!(!mortar_reaches((here, None), (point, None), true, walkable));
        assert!(mortar_reaches((here, None), (point, None), true, |_| true));
        // Its point on another floor than the maker's, never.
        assert!(!mortar_reaches((here, None), (point, floor), false, |_| {
            true
        }));
        assert!(!mortar_reaches((here, floor), (point, None), true, |_| {
            true
        }));
        assert!(mortar_reaches((here, floor), (point, floor), true, |_| {
            true
        }));
    }

    #[test]
    fn a_point_wants_a_straight_way_a_place_to_stand_and_the_makers_floor() {
        let cell = crate::iso::CELL;
        let here = Vec3::new(0.5, 0.0, 0.5) * cell;
        let point = Vec3::new(6.5, 0.0, 0.5) * cell;
        let floor = Some((1, 2, 0));
        let open = |_| true;
        assert!(point_reached((here, None), (point, None), (open, open)));
        // A wall across the line at the cell 3, wherever.
        assert!(!point_reached(
            (here, None),
            (point, None),
            (|(x, _)| x != 3, open)
        ));
        // The water there, which the line flies over and nothing stands on (#3358).
        assert!(point_reached(
            (here, None),
            (point, None),
            (open, |(x, _)| x != 3)
        ));
        // The point's own cell not to be stood on.
        assert!(!point_reached(
            (here, None),
            (point, None),
            (open, |(x, _)| x != 6)
        ));
        // Another floor than the maker's.
        assert!(!point_reached((here, None), (point, floor), (open, open)));
        assert!(point_reached((here, floor), (point, floor), (open, open)));
    }
}
