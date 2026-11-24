//! The hero's attack (#283; docs/re/fight.md, "The hero's attack"): a left click on a hostile
//! creature sends the hero at it (`0x6172C0`, the AI's states 1 and 4): it comes into the band of
//! its reach and strikes once, again while the button stays held or for each click during a
//! strike (#705), until the creature dies or the player sends it elsewhere; with Ctrl held it
//! strikes once, at once, where it stands, and a click on the ground strikes the air there. Its
//! hits resolve as the creatures' (`fight`). Where it stands, when it strikes and when
//! its hits land are the rules' (`opensacred_rules::minds`). With a bow it shoots arrows
//! (`arrows`), and with the Seraphim's effects of 1018 and 1019 bolts (`bolts`).

use std::time::Duration;

use bevy::prelude::*;
use opensacred_rules::arts::{BFG, Bolt, RAILGUN};
use opensacred_rules::minds;

use crate::ai::Mind;
use crate::fight::{self, Health};
use crate::frame::{Control, Step, Unpaused, WorldTick};
use crate::hero::{self, Body, Clips, GameSpeed};
use crate::npcs::Dies;
use crate::spawning::Rng;

/// The blend into a strike's clip.
const BLEND: Duration = Duration::from_millis(100);

/// Whether the left button's press went to an attack: then it does not steer the hero while it
/// is held (`hero::steer`).
#[derive(Resource, Default)]
pub struct Aim(pub bool);

/// The hero's attack: its target, or none for a strike in the air at `point`; whether it strikes
/// at once, where it stands, once; whether it ends with its strike, the player having sent the
/// hero elsewhere; whether the button that sent it is still held; whether a click still asks
/// for a strike that has not begun; whether its first strike spoke its `ATTACK` line; where its
/// AI stands; the combat art it is a shot of (a special arrow), if any; and whether its shot goes
/// on once whatever the button and its target, the flag `+0x18C` that a Ctrl click on the ground
/// in mid-shot sets (`0x4DC51A`, `0x4DF109`; #3577).
#[derive(Component)]
pub struct Attack {
    target: Option<Entity>,
    point: Vec3,
    at_once: bool,
    ends: bool,
    held: bool,
    asked: bool,
    spoke: bool,
    phase: Phase,
    special: Option<crate::arrows::Special>,
    goes_on: bool,
}

/// Where the hero's attack stands in the AI's state 4 (`0x4E0300`), which steps at the world's
/// ticks alone (#617): the original's hero, sent by a click at a target in its band, took the
/// state 4 at the click's tick, its sub-state 2 at the next and struck at the one after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Sent by a click, an order that the next tick takes (`0x60F130`, after the hero's update).
    Ordered,
    /// Deciding (the sub-state 0, or 6 after a strike that did not chain).
    Deciding,
    /// Coming on to the band (the sub-state 1), its move ending in it.
    ComesOn,
    /// In the band, turned to its target (the sub-state 2): it strikes at the next tick.
    InBand,
}

impl Attack {
    /// The creature it attacks, if any.
    pub fn target(&self) -> Option<Entity> {
        self.target
    }

    /// The hero's attack on `target`, as a click on it orders it, the button not held (the
    /// console's `attack`).
    pub fn ordered(target: Entity) -> Self {
        Self {
            target: Some(target),
            point: Vec3::ZERO,
            at_once: false,
            ends: false,
            held: false,
            asked: true,
            spoke: false,
            phase: Phase::Ordered,
            special: None,
            goes_on: false,
        }
    }

    /// The hero's shot of the combat art `special` at `target`, at once, where it stands, once,
    /// at the next tick (the action 5 that the Wood Elf's executor sets for her special arrows,
    /// `0x50AC00`).
    pub fn shot(target: Entity, special: crate::arrows::Special) -> Self {
        Self {
            at_once: true,
            phase: Phase::InBand,
            special: Some(special),
            ..Self::ordered(target)
        }
    }
}

/// The strike the hero plays (the attack action 3, `0x4E4CF0`): its clip's node, its length,
/// its motion's own factor of its speed (`motions::Factors`), the fractions at which it hits, how
/// many hits it has sent, and the bolt it shoots, if it is a shot of bolts (the action 8,
/// `0x4DCB80`).
#[derive(Component)]
pub struct Striking {
    node: AnimationNodeIndex,
    length: f32,
    factor: f32,
    fractions: [f32; 2],
    struck: u8,
    bolt: Option<Bolt>,
}

/// The hero's chain `+0x210`, the factor of its shots' rate, which its shot that goes on raises by
/// 0.1 up to 1.5 and keeps for its later shots, a new click's, a walk's, a cast's and a save's
/// alike, and which a strike's decision sets back to 1.0 (`opensacred_rules::minds::
/// held_shot_chain`, `0x4DFC8D`, `0x4E1443`; seen, #3568); 1.0 where the hero has none.
#[derive(Component, Clone, Copy, Debug)]
pub struct ShotChain(pub f32);

/// The dice of the hero's strikes.
#[derive(Resource)]
struct Dice(Rng);

/// On a press of the left button, the hero's attack: the hostile creature alive that the cursor
/// picks (`under_cursor`, `0x626C50`) becomes its target, which with Alt held an item under the
/// cursor comes before; with Ctrl held it strikes at once, at the creature or in the air at the
/// cursor; a press elsewhere ends the attack once
/// its strike ends, and steers the hero. On the button's release the attack ends with its strike,
/// unless a click still asks for one.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
pub(crate) fn aim(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: crate::controls::Keys,
    pointer: Res<crate::hud::Pointer>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    mut aim: ResMut<Aim>,
    // A creature of a dialogue is spoken to, hostile or not (`0x53A170`, `crate::talk`).
    (creatures, under): (
        Query<
            (&Mind, &Health),
            (
                Without<Dies>,
                Without<crate::npcs::Talk>,
                // A ridden horse gives way to its rider (docs/re/cursor.md).
                Without<crate::riding::Ridden>,
            ),
        >,
        Res<crate::under_cursor::UnderCursor>,
    ),
    mut heroes: Query<
        (
            Entity,
            &Transform,
            Option<&mut Attack>,
            Has<crate::making::Playing>,
        ),
        (With<Body>, Without<Dies>),
    >,
) {
    if !buttons.pressed(MouseButton::Left) {
        aim.0 = false;
        for (_, _, attack, _) in &mut heroes {
            if let Some(mut attack) = attack.filter(|attack| attack.held) {
                attack.held = false;
            }
        }
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let picked = under.0.filter(|&entity| {
        creatures
            .get(entity)
            .is_ok_and(|(mind, health)| mind.hostile() && health.current > 0)
    });
    let at_once = keys.pressed(crate::controls::Action::StrikeAtOnce);
    for (hero, transform, attack, playing) in &mut heroes {
        // Where the cursor's line of sight meets the hero's height: the air it strikes.
        let along = (transform.translation.y - ray.origin.y) / ray.direction.y;
        let point = ray.origin + ray.direction.as_vec3() * along;
        aim.0 = picked.is_some() || at_once;
        let next = Attack {
            target: picked,
            point,
            at_once,
            ends: false,
            held: true,
            asked: true,
            spoke: false,
            phase: Phase::Ordered,
            special: None,
            goes_on: false,
        };
        match (aim.0, attack) {
            // A click on the same target goes on with its attack, which spoke at its first
            // strike (the original's sub-state 1 anew, `0x4E4CF0`).
            (true, Some(mut attack)) => {
                let spoke = attack.spoke && attack.target == next.target;
                *attack = Attack { spoke, ..next };
            }
            (true, None) => {
                let mut hero = commands.entity(hero);
                // An art under way keeps its order, as the button leaves it (`making::aim`):
                // `making::make` alone ends its clip, and the attack waits for that (`engage`,
                // #1215).
                if !playing {
                    hero.remove::<crate::making::Making>();
                }
                hero.insert(next);
            }
            (false, Some(mut attack)) => attack.ends = true,
            (false, None) => {}
        }
        if let Some(target) = picked {
            debug!("attack: the hero attacks {target}");
        }
    }
}

/// The hero's attack (the AI's state 4 for a hero, docs/re/fight.md), deciding at the world's
/// ticks ([`Phase`], #617): the click's order taken at a tick, it decides at the next; farther
/// than its band from its target it runs to it, after it as it moves, its move ending in the band
/// at any frame; in the band, or at once, it stops and turns to the target, and at the next tick
/// strikes: one of its attack clips at random, once, at the game's speed, which hits at the first
/// tick past the clip's fractions (0.5 for a first of 0; a second where there is one) when the
/// target is then within 1.5 times the reach, its strikes at an even pace. It strikes again while
/// the button that sent it is held or a click asks for another (the original's attack action keeps
/// its first half while the button is held, #705), until the target dies; a strike at once or in
/// the air is one. With the button held, the next strike starts as soon as this one's hits are
/// struck and its clip has the turn to the next left (`minds::strike_gives_way`, #707), at any
/// frame as the clip queued at the hit starts; a click waits for the clip's end, which a tick
/// sees, and strikes then; a strike that does not chain ends there and its AI decides again at
/// the next tick.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn engage(
    mut commands: Commands,
    tick: Res<WorldTick>,
    mut dice: ResMut<Dice>,
    clips: Option<Res<Clips>>,
    speed: Res<GameSpeed>,
    (animations, factors): (Res<Assets<AnimationClip>>, Res<crate::motions::Factors>),
    mut heroes: Query<
        (
            Entity,
            &mut Transform,
            &mut Body,
            &mut Attack,
            Option<&mut Striking>,
            Option<&fight::Combat>,
            Option<&crate::hero::Size>,
            Option<&fight::Holding>,
            Option<&fight::Frenzy>,
            Option<&crate::effects::Effects>,
            Has<crate::making::Playing>,
            Option<&crate::riding::Seat>,
            Option<&mut ShotChain>,
        ),
        (
            Without<Dies>,
            Without<crate::fight::Downed>,
            Without<crate::fight::Pushed>,
        ),
    >,
    targets: Query<
        (
            &GlobalTransform,
            &Health,
            Option<&crate::ai::Mind>,
            Option<&crate::crowd::InTheWay>,
        ),
        Without<Dies>,
    >,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    (mut hits, mut shots, mut bolts): (
        MessageWriter<fight::Hit>,
        MessageWriter<crate::arrows::Loose>,
        MessageWriter<crate::bolts::Fire>,
    ),
    mut lines: MessageWriter<crate::voices::Speak>,
    (mut blows, mut fires): (
        MessageWriter<crate::sfx::Blow>,
        MessageWriter<crate::sfx::Gunfire>,
    ),
    (difficulty, arts, the_hero): (
        Option<Res<crate::game::Difficulty>>,
        Option<Res<crate::arts::HeroArts>>,
        Option<Res<crate::hero::Hero>>,
    ),
) {
    let Some(clips) = clips else {
        return;
    };
    let difficulty = difficulty.map_or_else(Default::default, |difficulty| difficulty.0);
    // The hold of set 60's cages: the plant cage's value 0 at their level (`0x4E07F4`, #2547).
    let bind_seconds = arts.as_deref().and_then(|arts| {
        let cage = arts
            .tables
            .spells
            .iter()
            .find(|spell| spell.id == opensacred_rules::spells::CAGE)?;
        let seconds = opensacred_rules::spells::value(cage, 0, minds::BIND_LEVEL);
        Some(u16::try_from(seconds).map_or(f32::MAX, f32::from))
    });
    for (
        hero,
        mut transform,
        mut body,
        mut attack,
        striking,
        combat,
        size,
        holding,
        frenzy,
        effects,
        playing,
        seat,
        mut chain,
    ) in &mut heroes
    {
        // Ordered while an art plays, it waits for the art's clip to end (`aim`, #1215).
        if playing {
            continue;
        }
        let target = attack
            .target
            .and_then(|target| targets.get(target).ok())
            .filter(|(_, health, ..)| health.current > 0);
        // With the Seraphim's effect 17, or 16 beyond 200, it shoots bolts (`0x53A170` at
        // `0x53AACE`).
        let bolt = effects.and_then(|effects| {
            let distance = target.map(|(at, ..)| {
                (at.translation() - transform.translation)
                    .with_y(0.0)
                    .length()
            });
            Bolt::of(effects.holds(BFG), effects.holds(RAILGUN), distance)
        });
        // With a bow or a crossbow in hand it shoots (the AI's state 8 for a hero, #784).
        let ranged = holding.is_some_and(|holding| holding.0.ranged) || bolt.is_some();
        // The class of what it holds and its hits' share of a strike (`0x4EA1F0`, `0x4EA3F0`).
        let class = holding.map_or(0, |holding| holding.0.class);
        let share = holding.map_or(1.0, |holding| holding.0.share());
        // From the saddle it strikes as its horse's size reaches (#2302), not turning to its
        // target, by its own clip (docs/re/horses.md, "Fighting from horseback").
        let riding = seat.is_some_and(|seat| seat.ridden().is_some());
        // The reach of the hero and its target, by their sizes (`0x4DBC60`, #642).
        let reach = minds::reach(
            if riding {
                opensacred_rules::horses::SIZE
            } else {
                size.map_or(minds::SIZE, |size| size.0)
            },
            target
                .and_then(|(_, _, mind, _)| mind)
                .map_or(minds::SIZE, |mind| mind.size()),
        );
        // The level it stands on, which the hero's way to it is tested on (`0x4F8560`).
        let onto = target.and_then(|(.., way)| way).map(|way| way.level);
        // A ranged fighter or a mage, which set 60's cages take (`0x4E07D2`).
        let bindable = target
            .and_then(|(_, _, mind, _)| mind)
            .is_some_and(|mind| minds::bindable(mind.ranged, mind.creature, mind.spell));
        let target = target.map(|(at, ..)| at.translation());
        let here = transform.translation;
        let Some((mut player, mut transitions)) =
            clips.player.and_then(|player| players.get_mut(player).ok())
        else {
            continue;
        };
        // At its attack speed (`0x4E4CF0`): its strikes' `+0x210` stays 1, held or clicked (#704).
        // Quickened by the Dwarf's frenzy (`0x584FE0`).
        let attack_speed = combat.map_or(100.0, |combat| {
            opensacred_rules::arts::quickened(combat.speed, frenzy.map_or(0.0, |frenzy| frenzy.0))
        }) / 100.0;
        // A shot at its attack speed times its chain, which a gun's takes none of (`0x4DCB80`,
        // `0x4DCD0F`; [`ShotChain`], #3568).
        let gun = holding.is_some_and(|holding| holding.0.gun);
        let kept = chain.as_ref().map_or(1.0, |chain| chain.0);
        let chained = attack_speed * if ranged && !gun { kept } else { 1.0 };
        // A shot of bolts at its own rate by the attack speed (`0x4DCB80`).
        let rate = |bolt: Option<Bolt>| bolt.map_or(chained, |bolt| bolt.rate(chained));
        let ticked = tick.seconds.is_some();
        let mut blend = BLEND;
        // Whether a strike starts at this frame whatever the AI's step: the next of a held button
        // or of a click.
        let mut now = false;
        if let Some(mut striking) = striking {
            let Some(active) = player.animation(striking.node) else {
                commands.entity(hero).remove::<Striking>();
                continue;
            };
            let done = active.seek_time() / striking.length;
            // The action tests its hits at its steps, the world's ticks, alone (`0x4EA1F0`); with
            // two weapons it hits twice whatever its clip, each hit of half the share (`0x4EA3F0`,
            // #2748).
            if ticked && minds::hit_due(striking.fractions, striking.struck, done, class) {
                striking.struck += 1;
                if let Some(bolt) = striking.bolt {
                    // Its bolts leave at the clip's first hit toward its target, or the point it
                    // strikes (`0x4DDD2A`, `0x4DE162`, `0x4DE5EC`), which the bolts fly and hit.
                    if striking.struck == 1 {
                        bolts.write(crate::bolts::Fire {
                            shooter: hero,
                            toward: target.unwrap_or(attack.point),
                            bolt,
                            only: None,
                        });
                    }
                } else if ranged {
                    // Its shot leaves at the clip's first hit, an arrow from its left hand toward
                    // its target (`0x4DD90E`, `0x4EB700`), which the arrows fly and hit, with its
                    // bow's sound as a creature's (`0x4DCB80`, #1475); one of a Ctrl click on the
                    // ground toward the click's point, which names no creature (seen, #3577).
                    if striking.struck == 1 {
                        blows.write(crate::sfx::Blow::Shot { shooter: hero });
                        shots.write(crate::arrows::Loose {
                            shooter: hero,
                            target: attack.target.unwrap_or(Entity::PLACEHOLDER),
                            at: attack.target.is_none().then_some(attack.point),
                            special: attack.special,
                        });
                    }
                } else if let (Some(entity), Some(at)) = (attack.target, target)
                    && minds::hero_hits((at - here).with_y(0.0).length(), reach)
                {
                    // The swing at a hit within reach (`0x4EA6FD`, #712).
                    blows.write(crate::sfx::Blow::Swing {
                        striker: hero,
                        air: None,
                    });
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
                        share,
                        ranged: false,
                        art: None,
                    });
                }
            }
            // A Ctrl click on the ground in mid-shot with a ranged weapon in hand: at the tick
            // after it the decision finds the shot playing, raises the chain by 0.1 up to 1.7 and
            // sets the flag by which the shot goes on once, at the click's point (the state 8,
            // `0x52EEEC`–`0x52EF16`, `0x4DC51A`; seen, #3568, #3577).
            if ticked
                && attack.phase == Phase::Ordered
                && attack.at_once
                && attack.target.is_none()
                && striking.bolt.is_none()
                && holding.is_some_and(|holding| holding.0.ranged)
            {
                let raised = minds::ground_shot_chain(kept);
                match chain.as_mut() {
                    Some(chain) => chain.0 = raised,
                    None => {
                        commands.entity(hero).insert(ShotChain(raised));
                    }
                }
                debug!("attack: a Ctrl click on the ground raises the hero's chain to {raised:.1}");
                attack.goes_on = true;
                attack.phase = Phase::Deciding;
                continue;
            }
            // Its shot, the button held and its target alive, or its flag set whatever both, no
            // art's arrow and its bolts' effect still on it, goes on: at the first tick with less
            // than 0.1 s of its clip left, the clip plays again an eighth in at its attack speed
            // times its chain as it was, the chain raised by 0.1 up to 1.5, the flag cleared, and
            // looses anew; a shot of the effect 17 or of a gun from its start at the speed it had
            // (`0x4DFC8D`, `0x4DF306`, `0x4DF109`; seen, #3563, #3568). A plain click while it
            // plays goes on with nothing: its own shot follows the shot's end.
            if ranged
                && ticked
                && striking.struck > 0
                && attack.special.is_none()
                && (attack.goes_on || (attack.held && target.is_some()))
                && !attack.ends
                && (striking.bolt.is_none() || bolt.is_some())
                && minds::held_shot_goes_on(
                    (striking.length - active.seek_time()).max(0.0) / active.speed().max(0.001),
                )
            {
                let whole = striking.bolt == Some(Bolt::Railgun) || gun;
                let played = if whole {
                    active.speed()
                } else {
                    speed.0 * attack_speed * kept * striking.factor
                };
                if let Some(active) = player.animation_mut(striking.node) {
                    active.replay();
                    active.seek_to(minds::held_shot_restart(striking.length, whole));
                    active.set_speed(played);
                }
                let raised = minds::held_shot_chain(kept);
                match chain.as_mut() {
                    Some(chain) => chain.0 = raised,
                    None => {
                        commands.entity(hero).insert(ShotChain(raised));
                    }
                }
                debug!("attack: the hero's shot goes on at {played:.3}, its chain {raised:.1}");
                attack.goes_on = false;
                striking.struck = 0;
                if striking.bolt.is_none() {
                    fires.write(crate::sfx::Gunfire { shooter: hero });
                }
                continue;
            }
            // With the button held, the next strike starts before this one's clip ends, the one
            // fading into the other over what is left (`0x5491F0`, #707); a shot goes on instead.
            let rate = rate(striking.bolt);
            let left = (striking.length - active.seek_time()).max(0.0)
                / (speed.0 * rate * striking.factor);
            if attack.held
                && !ranged
                && !attack.at_once
                && !attack.ends
                && target.is_some()
                && minds::strike_gives_way(striking.fractions, striking.struck, (left, rate), class)
            {
                commands.entity(hero).remove::<Striking>();
                blend = Duration::from_secs_f32(left.max(0.001));
                now = true;
            } else if !(ticked && active.is_finished()) {
                continue;
            } else {
                commands.entity(hero).remove::<Striking>();
                if minds::attack_ends(
                    attack.at_once,
                    attack.ends,
                    target.is_some(),
                    attack.held || attack.asked,
                ) {
                    commands.entity(hero).remove::<Attack>();
                    continue;
                }
                if !attack.asked {
                    // Not chained, the action ends and the AI decides again (the sub-state 6).
                    attack.phase = Phase::Deciding;
                    continue;
                }
                now = true;
            }
        }
        if !now && !ticked {
            // Between the ticks only its move to the band ends there (`0x4F4770`).
            if attack.phase == Phase::ComesOn
                && !attack.ends
                && let Some(at) = target
            {
                let distance = (at - here).with_y(0.0).length();
                let comes_on = if ranged {
                    minds::hero_comes_on_to_shoot(false, distance, true)
                } else {
                    minds::hero_comes_on(false, distance, reach)
                };
                if !comes_on {
                    body.stop();
                }
            }
            continue;
        }
        let aimed = match (attack.target, target) {
            (Some(_), Some(at)) => at,
            (None, _) if attack.at_once => attack.point,
            _ => {
                commands.entity(hero).remove::<Attack>();
                continue;
            }
        };
        if attack.ends {
            commands.entity(hero).remove::<Attack>();
            continue;
        }
        if !now && attack.phase == Phase::Ordered {
            // The click's order taken at this tick, it decides at the next. In melee, the hero of
            // set 60 first cages a ranged fighter or a mage beyond 200 by `R(0, 99)` below
            // `⌊30·D⌋`: ten cages of the plant cage at the level 2 (the AI's sub-state 0,
            // `0x4E06FD`–`0x4E09D1`; #2547).
            if !ranged
                && holding.is_some_and(|holding| holding.0.sets.binds())
                && let (Some(entity), Some(at), Some(seconds)) =
                    (attack.target, target, bind_seconds)
            {
                let distance = (at - here).with_y(0.0).length();
                let roll = if distance > minds::BINDS_BEYOND {
                    dice.0.below(100)
                } else {
                    u32::MAX
                };
                if minds::binds(distance, bindable, roll, difficulty) {
                    debug!("attack: the hero's set 60 cages {entity} for {seconds} s");
                    crate::plant_cages::cage(
                        &mut commands,
                        entity,
                        minds::BIND_CAGES,
                        (true, seconds),
                    );
                }
            }
            attack.phase = Phase::Deciding;
            continue;
        }
        let distance = (aimed - here).with_y(0.0).length();
        let comes_on = if ranged {
            minds::hero_comes_on_to_shoot(attack.at_once, distance, attack.phase == Phase::ComesOn)
        } else {
            minds::hero_comes_on(attack.at_once, distance, reach)
        };
        if comes_on {
            body.send_to(aimed, onto);
            attack.phase = Phase::ComesOn;
            continue;
        }
        body.stop();
        if !riding {
            transform.rotation = hero::facing((aimed - here).with_y(0.0));
        }
        if !now && attack.phase != Phase::InBand {
            // In the band, turned to its target, it strikes at the next tick.
            attack.phase = Phase::InBand;
            continue;
        }
        // A shot of bolts plays its own clip, the railgun's slot 164 or the BFG's 145, where the
        // model has it (`0x4DCB80`, #1371); else one of its attack clips at random.
        let shot = bolt.and_then(|bolt| match bolt {
            Bolt::Railgun => clips.bolts[0].as_ref(),
            Bolt::Bfg => clips.bolts[1].as_ref(),
        });
        if let (Some(bolt), Some(_)) = (bolt, shot) {
            debug!("attack: the hero shoots its {bolt:?} bolts by their own clip");
        }
        let count = u32::try_from(clips.attacks.len()).unwrap_or(0);
        let strike = if riding {
            clips.riding_attack.as_ref()
        } else {
            usize::try_from(dice.0.below(count))
                .ok()
                .and_then(|index| clips.attacks.get(index))
        };
        let Some((node, clip, fractions)) = shot.or(strike) else {
            commands.entity(hero).remove::<Attack>();
            continue;
        };
        // A strike's decision sets the chain back to 1.0 before the strike starts (`0x4E1443`;
        // seen, #3568).
        if !ranged && let Some(chain) = chain.as_mut() {
            chain.0 = 1.0;
        }
        let length = animations
            .get(clip)
            .map_or(1.0, AnimationClip::duration)
            .max(0.01);
        // The Dwarf's sword strike that begins the attack at its share (`0x4E5229`, #3353).
        let share = the_hero.as_ref().map_or(1.0, |the_hero| {
            minds::strike_clip_share(the_hero.creature, class, !attack.spoke)
        });
        // Times its motion's own factor, the bows' and the crossbows' of some classes
        // (`0x413610`, #3567).
        let factor = factors.of(clip);
        transitions
            .play(&mut player, *node, blend)
            .set_speed(speed.0 * rate(bolt) * factor * if bolt.is_some() { 1.0 } else { share });
        attack.asked = false;
        attack.phase = Phase::Deciding;
        // The attack's first strike speaks (`0x4E52D1`, the action's sub-state 0); the strikes it
        // chains do not (#711). A strike in the air swings there, once, by its attack speed
        // (`0x4E52B7`, #3346); a shot at a point sounds as its arrow leaves, as any shot's.
        if !attack.spoke {
            attack.spoke = true;
            if attack.target.is_none() && !ranged {
                blows.write(crate::sfx::Blow::Swing {
                    striker: hero,
                    air: Some(attack_speed * 100.0),
                });
            }
            lines.write(crate::voices::Speak {
                speaker: hero,
                group: crate::voices::ATTACK,
            });
        }
        // A shot's clip that starts fires a gun held (`0x4DD1FE`, `0x4DF442`; #1963).
        if ranged && bolt.is_none() {
            fires.write(crate::sfx::Gunfire { shooter: hero });
        }
        commands.entity(hero).insert(Striking {
            node: *node,
            length,
            factor,
            fractions: *fractions,
            struck: 0,
            bolt,
        });
    }
}

/// The hero's attack.
pub struct AttackPlugin;

impl Plugin for AttackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Aim>()
            .insert_resource(Dice(crate::seed::dice(1)))
            .add_systems(
                Update,
                (
                    aim.in_set(Control)
                        .in_set(crate::frame::OnWorld)
                        .before(hero::steer),
                    engage.after(hero::steer).before(hero::take_orders),
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            );
    }
}
