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

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use opensacred_data::tables::WeaponArt;
use opensacred_rules::arts::{
    self, ArtClip, Candidate, Creature, Style, attacke_target, due, hardhit_point, held_factor,
    made_from, rundum_radius, rundum_target,
};
use opensacred_rules::{minds, spells};

use crate::ai::Mind;
use crate::arts::HeroArts;
use crate::fight::{self, Combat, Experience, Health};
use crate::frame::{Control, Step, Unpaused, WorldTick};
use crate::hero::{self, ArtClipName, Body, Clips, GameSpeed};
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
}

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
        Style::Cast(157),
        Style::Cast(161),
        Style::Cast(163),
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
            vec![arts::kick_point(kind, class)]
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
/// effects of `arts::boon`, `ward`, `form` or `glance` (by the factor `glance`), or the Dark
/// Elf's 1044's by the first table's record 47 (`arts::soulcatcher`).
fn boon_of(arts: &HeroArts, record: &WeaponArt, level: u32, glance: f64) -> Option<arts::Boon> {
    arts::boon(record, level)
        .or_else(|| arts::ward(record, level))
        .or_else(|| arts::form(record, level))
        .or_else(|| arts::glance(record, level, glance))
        .or_else(|| {
            let spell = arts.tables.spells.iter().find(|spell| spell.id == 47)?;
            (record.id == 1044).then(|| arts::soulcatcher(spell, level))
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

/// At a new game, the hero's first art whose action is read is the right button's.
fn select_first(arts: Option<Res<HeroArts>>, mut selected: ResMut<Selected>) {
    // A hero that holds the selected art keeps it; a new game's hero, which does not, takes its
    // first (#678).
    let Some(arts) = arts else {
        return;
    };
    if selected.0.is_some_and(|art| arts.arts.get(art).is_some()) {
        return;
    }
    let first = arts
        .arts
        .entries
        .iter()
        .find(|e| arts::style(e.art).is_some())
        .map(|e| e.art);
    if selected.0 != first {
        selected.0 = first;
    }
}

/// Whether the hero may make `art` now: it holds it, it is ready, its action is read, and no
/// effect holds on it that refuses it (`effects`, `arts::refused_while`, `0x51D840`).
pub fn may_make(
    arts: &HeroArts,
    art: u16,
    effects: Option<&crate::effects::Effects>,
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
    Ok(())
}

/// On a press of the right button on a hostile creature alive, the hero makes the selected art
/// on it (a launch on an enemy, the type 1), its attack ending; with the art not ready it walks
/// to the creature's place instead, as the original's handler of the order takes it (about
/// `0x52FE40`, docs/re/arts.md, "The right button", #654; with Ctrl held the original's posts
/// another event, not followed). A press of the left button ends an order not yet under way.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn aim(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<crate::hud::Pointer>,
    camera: Single<(&Camera, &GlobalTransform)>,
    selected: Res<Selected>,
    arts: Option<Res<HeroArts>>,
    (creatures, corpses): (
        Query<(Entity, &GlobalTransform, &Mind, &Health), Without<Dies>>,
        Query<(Entity, &GlobalTransform), With<crate::npcs::Corpse>>,
    ),
    mut heroes: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Body,
            Has<Playing>,
            Option<&crate::effects::Effects>,
        ),
        Without<Dies>,
    >,
) {
    if buttons.just_pressed(MouseButton::Left) {
        for (hero, _, _, playing, _) in &heroes {
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
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let picked = creatures
        .iter()
        .filter(|(_, _, mind, health)| mind.hostile() && health.current > 0)
        .map(|(entity, at, ..)| {
            let at = at.translation();
            (entity, at, crate::attack::from_line(ray, at))
        })
        .filter(|&(.., distance)| distance <= crate::attack::PICKED_WITHIN)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    // The dead creature under the cursor, an art of the launch type 4's (`0x51D840`, case 4).
    let dead = corpses
        .iter()
        .map(|(entity, at)| {
            let at = at.translation();
            (entity, at, crate::attack::from_line(ray, at))
        })
        .filter(|&(.., distance)| distance <= crate::attack::PICKED_WITHIN)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let selected = art;
    for (hero, at, mut body, playing, effects) in &mut heroes {
        if playing {
            continue;
        }
        // The art of the hero's form for the selected one (`0x552370`).
        let art = arts::right_button(selected, arts.kind, |key| {
            effects.is_some_and(|effects| effects.holds_any(&[key]))
        });
        let launch = launch(&arts, art);
        let spell = art < 1000;
        // The right button on the hero himself makes him the target, nearer the cursor's line
        // than a hostile creature (seen: the Gladiator's 1015, of the launch type 2, made on
        // himself so, and refused on a hostile creature or on the ground).
        let on_hero = crate::attack::from_line(ray, at.translation());
        let on_hero = on_hero <= crate::attack::PICKED_WITHIN
            && picked.is_none_or(|(.., distance)| on_hero < distance);
        // Where the cursor's line meets the hero's height: the point of an art of the launch
        // type 3 (`0x51D840`'s action's point).
        let along = (at.translation().y - ray.origin.y) / ray.direction.y;
        let ground = ray.origin + ray.direction.as_vec3() * along;
        let (target, place) = match (launch, on_hero, picked) {
            (Some(3), ..) => (hero, ground),
            (Some(4), ..) => match dead {
                Some((corpse, place, _)) => (corpse, place),
                None => {
                    info!("making: {art} wants a dead creature under the cursor");
                    continue;
                }
            },
            // A spell of the launch type 5 takes its caster for its target, whatever the cursor
            // is on (`0x51D840`).
            (Some(5), ..) if spell => (hero, at.translation()),
            (Some(2), true, _) => (hero, at.translation()),
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
            (_, _, Some((entity, place, _))) => (entity, place),
            // A spell of the launch type 0 admits the ground under the cursor (`0x51D840`).
            (Some(0), _, None) if spell => (hero, ground),
            _ => continue,
        };
        match may_make(&arts, art, effects) {
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
                    });
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

/// The world point of the first of `names` among `root`'s descendants.
fn bone(
    root: Entity,
    names: &[&str],
    children: &Query<&Children>,
    bones: &Query<(&Name, &GlobalTransform)>,
) -> Option<Vec3> {
    let mut found: Option<(usize, Vec3)> = None;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok((name, at)) = bones.get(entity)
            && let Some(rank) = names.iter().position(|n| *n == name.as_str())
            && found.is_none_or(|(best, _)| rank < best)
        {
            found = Some((rank, at.translation()));
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    found.map(|(_, at)| at)
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
/// ends with none or after its last; a `Rundum` before 0.7 of its clip about its weapon's or its
/// hand's bone, on the nearest creature within its radius but the one it struck last, until its
/// clip ends; a `Hardhit` once at its hit point, on its target. The first strike, or a
/// `Rundum`'s start, starts the regeneration.
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
    animations: Res<Assets<AnimationClip>>,
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
        ),
        Without<Dies>,
    >,
    creatures: Query<(Entity, &GlobalTransform, &Mind, &Health), (Without<Dies>, Without<Body>)>,
    children: Query<&Children>,
    bones: Query<(&Name, &GlobalTransform)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    (mut hits, mut cries, ranks, leaps, sizes, mut launches, mut acts, corpses): (
        MessageWriter<fight::Hit>,
        MessageWriter<crate::voices::BattleCry>,
        Query<&fight::Ranked>,
        Query<(), With<Leap>>,
        Query<&crate::arrows::Size>,
        MessageWriter<crate::bolts::Launch>,
        MessageWriter<crate::spells::Acts>,
        Query<(&GlobalTransform, &crate::npcs::Corpse)>,
    ),
    nearest: Res<crate::crowd::Nearest>,
    heights: Res<crate::terrain::Heights>,
) {
    let (Some(clips), Some(arts)) = (clips, arts.as_mut()) else {
        return;
    };
    let by_day = clock.is_none_or(|clock| clock.phase() == crate::daynight::Phase::Day);
    for (hero, mut transform, mut body, making, playing, combat, size, holding, frenzy) in
        &mut heroes
    {
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
            if let Some(Style::Cast(slot)) = arts::style(making.art) {
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
                if spells::turns(making.art) && toward.length() > f32::EPSILON {
                    transform.rotation = hero::facing(toward);
                }
                let level = arts.arts.level_in_play(
                    making.art,
                    Creature {
                        kind: arts.kind,
                        level: experience.level,
                        player: true,
                    },
                );
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
                let played = speed.0 * cast / 100.0;
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
            // A boon is made where the hero stands, on itself; any other art on its target alive,
            // within its distance.
            if making.target != hero && !raises {
                let Some((target, target_reach)) = creatures
                    .get(making.target)
                    .ok()
                    .filter(|(_, _, mind, health)| mind.hostile() && health.current > 0)
                    .map(|(_, at, mind, _)| (at.translation(), reach(mind)))
                else {
                    end(&mut commands);
                    continue;
                };
                let distance = arts::order_distance(making.art, record.distance);
                let Some((at, within)) = made_from(distance, to_f32(target_reach)) else {
                    info!("making: {}'s distance is not made yet", making.art);
                    end(&mut commands);
                    continue;
                };
                let distance = (target - here).with_y(0.0).length();
                if at > 0.0 && distance > at + within {
                    body.send(target);
                    continue;
                }
                transform.rotation = hero::facing((target - here).with_y(0.0));
            }
            body.stop();
            let (Some(style), Some(level)) = (
                arts::style(making.art),
                arts.arts.level_in_play(
                    making.art,
                    Creature {
                        kind: arts.kind,
                        level: experience.level,
                        player: true,
                    },
                ),
            ) else {
                end(&mut commands);
                continue;
            };
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
                arts.start(making.art);
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
            let Some(action) = clips.arts.iter().find(|a| a.style == style) else {
                info!(
                    "making: the hero has no clip of {} for its model and what it holds",
                    making.art
                );
                end(&mut commands);
                continue;
            };
            // The art's clip, or one of the attack's at random for the Seraphim's `Hardhit`.
            let (node, clip, mut points) = match (action.node, &action.clip) {
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
                // Its point at its reach at most, the farthest of its line where she may stand
                // with a straight way to it (`0x50E6C0`).
                let aimed = making.point.unwrap_or(point);
                let reach = arts::dash_reach(record, level);
                let [x, z] = arts::panther_point(here.xz().into(), aimed.xz().into(), reach, false);
                let level = body.level();
                let walkable = |cell| heights.walkable_on(cell, level);
                let far = Vec3::new(x, here.y, z);
                let along = (far - here).normalize_or_zero();
                // From its point back toward her a cell at a time.
                let reached = std::iter::successors(Some((far - here).length()), |&left| {
                    (left > 0.0).then(|| (left - crate::iso::CELL).max(0.0))
                })
                .map(|left| here + along * left)
                .find(|to| {
                    walkable(crate::paths::cell_of(to.x, to.z))
                        && crate::paths::clear(here.xz(), to.xz(), walkable)
                });
                let Some(reached) = reached else {
                    info!("making: the dash has nowhere to go");
                    end(&mut commands);
                    continue;
                };
                point = reached;
            }
            if style == Style::Jump {
                // A jump wants a straight way to its point and a place to stand there, and plays
                // at a rate by its length (`0x5038F0` at `0x505B6E`).
                let level = body.level();
                let walkable = |cell| heights.walkable_on(cell, level);
                let to = crate::paths::cell_of(point.x, point.z);
                if !crate::paths::clear(here.xz(), point.xz(), walkable) || !walkable(to) {
                    info!("making: no straight way for the jump to its target");
                    end(&mut commands);
                    continue;
                }
                rate = if arts.kind == arts::VAMPIRESS_BY_NIGHT {
                    arts::CLAW_JUMP_RATE
                } else {
                    arts::jump_rate(whole_distance(here, point))
                };
            }
            let played = if action.fixed {
                speed.0 * rate
            } else {
                speed.0 * clip_speed * rate
            };
            transitions.play(&mut player, node, BLEND).set_speed(played);
            let factor = if style == Style::Backbreaker {
                arts::backbreaker_factor(level)
            } else {
                held_factor(arts::factor(record, level), action.halved)
            };
            // The backbreaker takes its target up and holds it (`0x4FF0B0`).
            if style == Style::Backbreaker
                && let Ok((_, _, _, health)) = creatures.get(making.target)
            {
                let seconds = arts::backbreaker_hold(experience.level, health.level);
                let mut held = commands.entity(making.target);
                held.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
                held.entry::<Mind>()
                    .and_modify(move |mut mind| mind.stunned(seconds));
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
                let boon = boon_of(arts, record, level, 1.0);
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
            // A `Rundum`, the whirling blow, the hurled blades and the backbreaker start their
            // regeneration as they start.
            if matches!(
                style,
                Style::Rundum
                    | Style::Whirl
                    | Style::Blades
                    | Style::Backbreaker
                    | Style::Dominate
                    | Style::Kiss
            ) {
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
            commands.entity(hero).insert(Playing {
                style,
                node,
                length,
                seconds: length / played.max(f32::EPSILON),
                point,
                points,
                struck: 0,
                target: making.target,
                last: None,
                level,
                factor,
            });
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
            attacker: hero,
            target,
            share: playing.factor,
            ranged: false,
            art: Some(playing.level),
        };
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
                if let Some(record) = arts.tables.weapon_arts.iter().find(|a| a.id == making.art) {
                    let neck = bone(hero, &[crate::arrows::NECK], &children, &bones)
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
                crate::spells::throw_trap(
                    &mut commands,
                    hero,
                    (making.art, playing.level),
                    multipliers,
                    (here, playing.point, body.level()),
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
                // Below 0.6 left of its clip its target, of none of the classes 9 and 10 nor a
                // big type and below her bound, takes the effect 14 for 12 + 3·L s, which turns
                // its sides over, so that it fights its old friends (seen); then it ends
                // (`0x50E6C0`).
                if !due(1.0 - arts::MASTER_BITES, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                let boss = ranks
                    .get(playing.target)
                    .is_ok_and(|rank| rank.0 == opensacred_rules::fight::Rank::Boss);
                if let (Some(record), Ok((_, _, mind, health))) =
                    (record, creatures.get(playing.target))
                    && mind.hostile()
                    && health.current > 0
                    && arts::master_bites(
                        mind.class,
                        mind.big,
                        health.level,
                        boss,
                        experience.level,
                        playing.level,
                    )
                {
                    let seconds = arts::master_bite_seconds(record, playing.level);
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
                    && corpses.contains(playing.target)
                {
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
                        && crate::paths::clear(here.xz(), Vec2::new(x, z), |cell| {
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
                // a fixed physical damage of its drain, and she takes as much health; then it ends
                // (`0x50E6C0`).
                if !due(1.0 - arts::BITES, done) {
                    if finished {
                        end(&mut commands);
                    }
                    continue;
                }
                let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                if let (Some(record), Ok((_, _, mind, health))) =
                    (record, creatures.get(playing.target))
                    && mind.hostile()
                    && health.current > 0
                {
                    let drain = arts::bite_drain(record, playing.level);
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
                    commands
                        .entity(hero)
                        .entry::<Health>()
                        .and_modify(move |mut health| {
                            health.current = (health.current + drain).min(health.most);
                        });
                    info!("making: {} drains {drain}", making.art);
                }
                end(&mut commands);
            }
            Style::Backbreaker => {
                // At its point it lets its target go with its strike; the target stays held by a
                // roll, else its timer comes back within 0.5 s (`0x4FF0B0`).
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
                    hits.write(hit(playing.target, &playing));
                    let boss = ranks
                        .get(playing.target)
                        .is_ok_and(|rank| rank.0 == opensacred_rules::fight::Rank::Boss);
                    let roll = dice.0.below(100);
                    if !arts::backbreaker_holds(playing.level, health.level, boss, roll) {
                        commands
                            .entity(playing.target)
                            .entry::<Mind>()
                            .and_modify(|mut mind| mind.stunned(arts::LET_GO));
                    }
                }
                end(&mut commands);
            }
            Style::Dash => {
                // Below 0.8 left of its clip she is put at its point, starts its regeneration and
                // ends (`0x50E6C0`).
                if due(1.0 - arts::DASHES, done) {
                    body.teleport(&mut transform, playing.point);
                    arts.start(making.art);
                    end(&mut commands);
                } else if finished {
                    end(&mut commands);
                }
            }
            Style::Changeling => {
                // Below 0.5 left of its clip the Vampiress changes her form: 1022 into her
                // night's for its time and its regeneration started, 1023 back to her day's and
                // 1022's regeneration started anew (seen), its own by day (`0x50E6C0`).
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
                        arts::changeling_seconds(record, playing.level, by_day)
                    });
                commands.trigger(crate::forms::Change { night, seconds });
                arts.start(1022);
                if !night && by_day {
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
                // At its first run each hostile creature within its radius of the hero, by a
                // kick's roll, is knocked back and held; then its regeneration starts and its
                // clip plays on (`0x4FF0B0` at `0x4FFB75`).
                if playing.struck == 0 {
                    playing.struck = 1;
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
                    commands
                        .entity(playing.target)
                        .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                        .insert(Knocked {
                            by: hero,
                            from: here,
                            left: 0.0,
                            held: hold,
                            throw: Some(arts::Flight::new(along.into(), arts::THROW_RISE)),
                        });
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
                            let burned: Vec<Entity> =
                                hostile_near(from, from + facing * arts::FLAMES_LENGTH)
                                    .into_iter()
                                    .map(|(entity, _)| entity)
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
                        let struck = hostile_near(from, from + direction * reach);
                        info!("making: {} shoots {} creatures", making.art, struck.len());
                        for (entity, mind) in struck {
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
                            });
                            if !mind.big && knocks_back(&mut dice.0, playing.level, mind) {
                                commands
                                    .entity(entity)
                                    .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                                    .insert(Knocked {
                                        by: hero,
                                        from: here,
                                        left: arts::CANNON_KNOCK,
                                        held: 0.0,
                                        throw: None,
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
                            boon_of(arts, record, playing.level, glance)
                        });
                    // The Seraphim's 1018 and 1019 end as they give their effects (`0x5038F0`).
                    let ends = boon.as_ref().is_some_and(|boon| boon.ends);
                    if let Some(boon) = boon {
                        info!("making: {} gives the hero {:?}", making.art, boon.effects);
                        commands
                            .entity(hero)
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
                playing.target = chosen;
                playing.struck += 1;
                if playing.struck >= playing.points.len() {
                    end(&mut commands);
                }
            }
            Style::Kick => {
                // One strike at its point, which may knock its target back; its clip plays on
                // to its end (#650).
                if playing.struck == 0 && due(playing.points.first().copied().unwrap_or(0.6), done)
                {
                    playing.struck = 1;
                    if let Ok((_, _, mind, health)) = creatures.get(playing.target)
                        && health.current > 0
                    {
                        hits.write(hit(playing.target, &playing));
                        let knocks = knocks_back(&mut dice.0, playing.level, mind);
                        let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                        if knocks
                            && let Some(left) = record.and_then(|record| {
                                arts::knockback(record, playing.level, mind.level)
                            })
                        {
                            commands
                                .entity(playing.target)
                                .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                                .insert(Knocked {
                                    by: hero,
                                    from: here,
                                    left: to_f32(left),
                                    held: 0.0,
                                    throw: None,
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
                    if creatures.get(playing.target).is_ok() {
                        hits.write(hit(playing.target, &playing));
                    }
                    arts.start(making.art);
                    end(&mut commands);
                } else if finished {
                    end(&mut commands);
                }
            }
            Style::Rundum | Style::Whirl => {
                if finished {
                    end(&mut commands);
                    continue;
                }
                // It looks for the creatures about its bones as its executor runs, on the
                // world's tick (**hypothesis**, #617), the Gladiator's from 0.3 of its clip.
                if !arts::rundum_looks(arts.kind, done) || tick.seconds.is_none() {
                    continue;
                }
                let Some(record) = arts.tables.weapon_arts.iter().find(|a| a.id == making.art)
                else {
                    continue;
                };
                let radius = |reach| rundum_radius(record, playing.level, reach);
                let point = bone(hero, &WEAPON_BONES, &children, &bones)
                    .or_else(|| bone(hero, &[HAND_BONE], &children, &bones))
                    .unwrap_or(here);
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
                    playing.last = Some(chosen);
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
                            });
                    }
                }
            }
        }
    }
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
    mut heroes: Query<(Entity, &mut Playing, Option<&Blade>), Without<Dies>>,
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
    children: Query<&Children>,
    bones: Query<(&Name, &GlobalTransform)>,
    mut hits: MessageWriter<fight::Hit>,
) {
    if tick.seconds.is_none() {
        return;
    }
    for (hero, mut playing, blade) in &mut heroes {
        if playing.style != Style::Blades {
            continue;
        }
        let Some(blade) = blade.copied() else {
            continue;
        };
        let Some(point) = bone(hero, &blade_bones(blade), &children, &bones) else {
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
                attacker: hero,
                target: entity,
                share: playing.factor,
                ranged: false,
                art: Some(playing.level),
            });
            playing.last = Some(entity);
        }
    }
}

/// A hero's flight of its jump (`0x44DED0`, `0x44DFE0`): from `from` to `to` on the ground in
/// `time` seconds, `flown` of them gone, its height on the arc of `arts::arc` of the height
/// `peak` at its middle above the ground under it.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Leap {
    pub from: Vec3,
    pub to: Vec3,
    pub time: f32,
    pub flown: f32,
    pub peak: f32,
}

/// Flies each hero in its jump (`0x44DFE0`): along the line to its landing in its flight's time,
/// on its arc, and lands it there.
fn leap(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<crate::terrain::Heights>,
    mut heroes: Query<(Entity, &mut Transform, &Body, &mut Leap)>,
) {
    for (hero, mut transform, body, mut leap) in &mut heroes {
        leap.flown += time.delta_secs();
        let t = if leap.time > 0.0 {
            (leap.flown / leap.time).min(1.0)
        } else {
            1.0
        };
        let ground = leap.from.lerp(leap.to, t);
        let y = heights
            .standing(ground.x, ground.z, body.level())
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
}

/// A creature's flags word's bit of a boss (`+0x1F4` bit `0x80000`).
const BOSS: u32 = 0x80000;

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
            let [dx, dz] = flight.step(time.delta_secs());
            let to = transform.translation + Vec3::new(dx, 0.0, dz);
            if heights.walkable_on(crate::paths::cell_of(to.x, to.z), way.level) {
                transform.translation.x = to.x;
                transform.translation.z = to.z;
            } else {
                flight.velocity = [0.0, 0.0];
            }
            let ground = heights
                .standing(transform.translation.x, transform.translation.z, way.level)
                .unwrap_or(transform.translation.y);
            transform.translation.y = ground
                + crate::iso::height(flight.height.max(0.0) * crate::iso::PIXELS_PER_MODEL_HEIGHT);
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
                });
                commands
                    .entity(other)
                    .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                    .insert(Knocked {
                        by: knock.by,
                        from: knock.from,
                        left: 0.0,
                        held: arts::BOWL_HOLD,
                        throw: Some(arts::bowled(flight.velocity, to_it)),
                    });
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
        } else {
            mind.knocked(knock.held);
        }
    }
}

/// A whole number of world units as a float.
fn to_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

/// The hero's making of its arts.
pub struct MakingPlugin;

impl Plugin for MakingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selected>()
            .insert_resource(Dice(Rng::of(0, 0, 2)))
            .add_systems(Update, select_first)
            .add_systems(
                Update,
                (
                    aim.in_set(Control).before(hero::steer),
                    make.after(hero::steer).before(hero::take_orders),
                    blades.after(make),
                    leap.after(make),
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
}
