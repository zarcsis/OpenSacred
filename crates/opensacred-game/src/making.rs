//! The hero makes its combat arts (#509; docs/re/arts.md, "An art in a strike", "The arts'
//! clips"): a right click on a hostile creature makes the selected art on it, as the original's
//! does, and the console's `arts make` on the nearest. The order (`0x4FD0F0`) needs the art
//! ready and of a style whose action is read; the hero comes on to within 10 of the reach of the
//! two creatures, stops, turns to the target and plays its class's clip of the art for what it
//! holds (`opensacred_rules::arts::action`) at its attack's speed; the strikes are the rules'
//! (`arts::attacke_target`, `arts::rundum_target`, `fight::art_strike`), the first starting the
//! regeneration of every art of its kind (`arts::HeroArts::start`). The spells and the other
//! classes' arts wait for their reading.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use opensacred_rules::arts::{
    self, ArtClip, Candidate, Creature, Style, attacke_target, due, hardhit_point, held_factor,
    made_from, rundum_radius, rundum_target,
};
use opensacred_rules::minds;

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

/// The hero's order to make an art on a creature (`0x4FD0F0`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Making {
    pub art: u16,
    pub target: Entity,
}

/// An art's action under way: its style, its clip's node and length, its hit points and how many
/// strikes it has sent, its target, the creature a `Rundum` struck last, and its strikes' level
/// and factor.
#[derive(Component, Debug, Clone)]
pub struct Playing {
    style: Style,
    node: AnimationNodeIndex,
    length: f32,
    points: Vec<f32>,
    struck: usize,
    target: Entity,
    last: Option<Entity>,
    level: u32,
    factor: f32,
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
        Style::Smite,
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
        // A kick strikes at its class's point of its clip, not at the clip's hit points.
        let points = if style == Style::Kick {
            vec![arts::kick_point(kind, class)]
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
    creatures: Query<(Entity, &GlobalTransform, &Mind, &Health), Without<Dies>>,
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
    let launch = arts
        .tables
        .weapon_arts
        .iter()
        .find(|a| a.id == art)
        .map(|a| a.launch);
    for (hero, at, mut body, playing, effects) in &mut heroes {
        if playing {
            continue;
        }
        // The right button on the hero himself makes him the target, nearer the cursor's line
        // than a hostile creature (seen: the Gladiator's 1015, of the launch type 2, made on
        // himself so, and refused on a hostile creature or on the ground).
        let on_hero = crate::attack::from_line(ray, at.translation());
        let on_hero = on_hero <= crate::attack::PICKED_WITHIN
            && picked.is_none_or(|(.., distance)| on_hero < distance);
        let (target, place) = match (launch, on_hero, picked) {
            (Some(2), true, _) => (hero, at.translation()),
            (Some(2), false, _) => {
                info!("making: {art} wants a friend, or the hero himself, under the cursor");
                continue;
            }
            (_, true, _) if arts::style(art) == Some(Style::Boon) => (hero, at.translation()),
            (_, _, Some((entity, place, _))) => (entity, place),
            _ => continue,
        };
        match may_make(&arts, art, effects) {
            Ok(()) => {
                // A boon's target is its maker (the launch type 5, `0x51D840`).
                let target = if arts::style(art) == Some(Style::Boon) {
                    hero
                } else {
                    target
                };
                info!("making: the hero makes {art} on {target}");
                commands
                    .entity(hero)
                    .remove::<(crate::attack::Attack, crate::attack::Striking)>()
                    .insert(Making { art, target });
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

/// A whole distance on the ground, as the original's integer square root gives it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a distance on the ground, whole and positive, far below the integers' range"
)]
fn whole_distance(a: Vec3, b: Vec3) -> u32 {
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
    tick: Res<WorldTick>,
    speed: Res<GameSpeed>,
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
        ),
        Without<Dies>,
    >,
    creatures: Query<(Entity, &GlobalTransform, &Mind, &Health), (Without<Dies>, Without<Body>)>,
    children: Query<&Children>,
    bones: Query<(&Name, &GlobalTransform)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut hits: MessageWriter<fight::Hit>,
    mut cries: MessageWriter<crate::voices::BattleCry>,
    nearest: Res<crate::crowd::Nearest>,
) {
    let (Some(clips), Some(arts)) = (clips, arts.as_mut()) else {
        return;
    };
    for (hero, mut transform, mut body, making, playing, combat, size) in &mut heroes {
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
            let Some(record) = arts.tables.weapon_arts.iter().find(|a| a.id == making.art) else {
                end(&mut commands);
                continue;
            };
            // A boon is made where the hero stands, on itself; any other art on its target alive,
            // within its distance.
            if making.target != hero {
                let Some((target, target_reach)) = creatures
                    .get(making.target)
                    .ok()
                    .filter(|(_, _, mind, health)| mind.hostile() && health.current > 0)
                    .map(|(_, at, mind, _)| (at.translation(), reach(mind)))
                else {
                    end(&mut commands);
                    continue;
                };
                let Some((at, within)) = made_from(record.distance, to_f32(target_reach)) else {
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
            let Some(action) = clips.arts.iter().find(|a| a.style == style) else {
                info!("making: the hero's model has no clip of {}", making.art);
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
            let clip_speed = combat.map_or(1.0, |combat| combat.clip_speed());
            transitions
                .play(&mut player, node, BLEND)
                .set_speed(if action.fixed {
                    speed.0 * action.rate
                } else {
                    speed.0 * clip_speed * action.rate
                });
            let factor = held_factor(arts::factor(record, level), action.halved);
            // A smite strikes at its own point of its clip.
            if style == Style::Smite {
                points = vec![1.0 - arts::SMITE_LEFT];
            }
            // A boon gives its effects at its own point of its clip.
            if style == Style::Boon {
                points = arts::boon(record, level)
                    .map(|boon| vec![boon.point])
                    .unwrap_or_default();
            }
            if style == Style::Rundum {
                arts.start(making.art);
            }
            info!(
                "making: the hero makes {} ({style:?}) at level {level}, its factor {factor}",
                making.art
            );
            // The hero's battle cry (`0x67A7A0`'s event 5, `0x693FE0`, #713).
            cries.write(crate::voices::BattleCry { hero });
            commands.entity(hero).insert(Playing {
                style,
                node,
                length,
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
            attacker: hero,
            target,
            share: playing.factor,
            ranged: false,
            art: Some(playing.level),
        };
        match playing.style {
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
            Style::Boon => {
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
                        .and_then(|record| arts::boon(record, playing.level));
                    if let Some(boon) = boon {
                        info!("making: {} gives the hero {:?}", making.art, boon.effects);
                        commands
                            .entity(hero)
                            .entry::<crate::effects::Effects>()
                            .or_default()
                            .and_modify(move |mut effects| {
                                for (effect, seconds) in boon.effects {
                                    effects.add(effect, seconds);
                                }
                            });
                    }
                    arts.start(making.art);
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
                        let chance = arts::knockback_chance(
                            playing.level,
                            (mind.creature, mind.level),
                            mind.flags & BOSS != 0,
                            mind.big,
                        );
                        let roll = to_f32(dice.0.below(100));
                        let record = arts.tables.weapon_arts.iter().find(|a| a.id == making.art);
                        if roll < chance
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
            Style::Rundum => {
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
                }
            }
        }
    }
}

/// A creature that a kick knocks back (`0x594D12`, the flag `0x20000` of `+0x200`): it slides
/// away from its kicker `by`, from its place `from` where it is gone, at
/// `arts::KNOCKBACK_SPEED` until it has gone `left` (`+0x24D`), doing nothing else; its walk
/// and strike are ended as it is knocked.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Knocked {
    pub by: Entity,
    pub from: Vec3,
    pub left: f32,
}

/// A creature's flags word's bit of a boss (`+0x1F4` bit `0x80000`).
const BOSS: u32 = 0x80000;

/// Slides each knocked-back creature away from its kicker at every frame (`0x594D12`): a step of
/// `arts::KNOCKBACK_SPEED` times the frame's time, at most `arts::KNOCKBACK_STEP` of it, taken
/// from what is left; the step moves it where it may stand on its level, else it stays; with
/// nothing left it is free again.
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
    )>,
) {
    let dt = time.delta_secs().min(arts::KNOCKBACK_STEP);
    for (entity, mut transform, mut knock, way) in &mut knocked {
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
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            )
            .add_systems(Update, knock.in_set(Unpaused).in_set(Step::HeroMove));
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
        // The Mage, whose executor is not read, has none.
        assert!(super::clips(3, 1, Some(&slots), &points).is_empty());
    }
}
