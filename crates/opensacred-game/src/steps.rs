//! The footsteps of the creatures other than the hero (#1171; docs/re/sound.md, "The other
//! creatures' footsteps"). A creature that walks or runs steps at its clip's motion's moments
//! (`tables/motions.ron`'s `floats_46`: the right foot's `[1]` and the left's `[0]`, and the
//! forelegs' `[3]` and `[2]` where not 0), each once a loop of the clip (`0x5941A0`); each step
//! plays at the creature, at the volume of its place, the sound its type takes (`0x68EB90`): the
//! humans' `footsteps_otherhuman` by the ground as the hero's, the animals', the big ones', the
//! metal clothes', the skeletons', the horses', the bigger ones', the tatzelwurms' and the
//! spiders', the wings of the fliers, at a rate of the sample's of 0.9 to 1.1 (0.6 to 0.8 for the
//! giant spiders); and the Big Demon's step jolts the screen (`0x639080`, the weather's event of
//! kind 12 of 18).
//!
//! The types' sounds are the original's cascade worked out over its types' names
//! (`0x8EC328`): the two tables of name prefixes that `0x697860` fills, the lists of types, and the
//! names' parts. The fliers are the types of the `Creature.pak` flag 1 that it sends through its
//! wings while in the air (`+0x1F4` bit `0x200000`; **hypothesis**: always in the air).

use std::collections::HashMap;

use bevy::animation::graph::AnimationNodeType;
use bevy::prelude::*;
use opensacred_rules::weather::Event;

use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::npcs::{Dies, Npc, Walk};
use crate::sfx::{self, Of, Roll};
use crate::terrain::Heights;

/// The profiles of the steps (`sndProfiles.pak`).
const HUMAN: u32 = 104;
const ANIMALS: u32 = 106;
const LIGHT: u32 = 103;
const BIG: u32 = 107;
const METAL: u32 = 109;
const SKELETONS: u32 = 108;
const HORSE: u32 = 105;
const BIGGER: u32 = 113;
const TATZEL: u32 = 112;
const SPIDERS: u32 = 114;
/// The wings, by their sound ids: the small, the big and the others.
const SMALL_WINGS: [u32; 2] = [1194, 1199];
const BIG_WINGS: [u32; 2] = [1197, 1198];
const WINGS: [u32; 2] = [1195, 1196];
/// The Big Demon, whose steps jolt the screen, and the giant spiders, whose steps are slower.
const BIG_DEMON: u32 = 53;
const GIANT_SPIDERS: [u32; 2] = [55, 237];
/// The jolt a Big Demon's step sends (`0x639080`).
const JOLT: u32 = 18;

/// The sound of a type's steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// A profile's group by the ground under the creature.
    Ground(u32, Ground),
    /// Two wings' sounds, one at even odds.
    Wings([u32; 2]),
}

/// How a step's group follows the ground (its cell's nature, the high nibble of its ground byte).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    /// As the hero's (`0x68EDDA`'s switch).
    Hero,
    /// The horses': 2 on water and swamp, 5 on roads, stones and the dungeon, else 1.
    Horse,
    /// The big ones': 2 on water and swamp, else 1.
    Big,
    /// One group whatever the ground.
    Only(u8),
}

/// The sound of the steps of the creature type `kind` (`0x68EB90`), none for a type that makes
/// none (the heroes' types among them, as other creatures).
pub fn sound(kind: u32) -> Option<Sound> {
    let ground = |profile| Some(Sound::Ground(profile, Ground::Only(1)));
    match kind {
        4..=5
        | 78..=79
        | 82
        | 104
        | 133..=134
        | 182..=184
        | 216..=219
        | 222..=231
        | 238
        | 249
        | 257..=260
        | 262..=323
        | 326..=327
        | 361
        | 363
        | 368
        | 597..=598
        | 640..=663
        | 665..=688
        | 691..=705 => Some(Sound::Ground(HUMAN, Ground::Hero)),
        211..=212
        | 349
        | 512
        | 516
        | 520..=528
        | 530..=533
        | 536..=538
        | 540..=541
        | 543
        | 545
        | 567..=571
        | 573..=588
        | 591..=593
        | 599..=600 => ground(ANIMALS),
        33..=36
        | 47
        | 50
        | 57..=59
        | 69..=70
        | 73
        | 93
        | 95..=98
        | 106..=107
        | 119..=124
        | 189
        | 192
        | 197
        | 204
        | 210
        | 213
        | 329
        | 334
        | 339..=343
        | 346
        | 350
        | 711..=712 => Some(Sound::Ground(LIGHT, Ground::Only(4))),
        37
        | 39..=43
        | 45
        | 49
        | 51
        | 60..=62
        | 68
        | 77
        | 84
        | 89..=92
        | 137
        | 187
        | 196
        | 202
        | 205..=208
        | 253..=254
        | 330..=332
        | 344
        | 359..=360
        | 365..=367 => Some(Sound::Ground(BIG, Ground::Big)),
        38
        | 67
        | 76
        | 85..=87
        | 118
        | 127..=130
        | 142..=143
        | 171..=181
        | 185
        | 188
        | 214..=215
        | 245
        | 250..=252
        | 356..=357
        | 689..=690 => ground(METAL),
        32 | 48 | 80..=81 | 112..=117 | 131 | 355 | 708..=710 | 713..=714 => ground(SKELETONS),
        72 | 550..=558 => Some(Sound::Ground(HORSE, Ground::Horse)),
        52..=53 | 74 | 88 | 94 | 100..=103 => ground(BIGGER),
        55..=56 | 194 | 209 | 237 | 246 | 347..=348 => ground(TATZEL),
        71 | 75 | 99 | 109 | 234..=236 | 601 => ground(SPIDERS),
        54 => Some(Sound::Wings(BIG_WINGS)),
        190 | 193 | 335..=336 => Some(Sound::Wings(WINGS)),
        513 | 589..=590 => Some(Sound::Wings(SMALL_WINGS)),
        _ => None,
    }
}

/// The group of a step of `ground` on a cell of the nature `nature`.
fn group(ground: Ground, nature: u8) -> u8 {
    match ground {
        Ground::Hero => crate::sound::group_of(nature),
        Ground::Horse => match nature {
            0x5 | 0x9 | 0xA => 2,
            0xB..=0xD => 5,
            _ => 1,
        },
        Ground::Big => match nature {
            0x5 | 0x9 | 0xA => 2,
            _ => 1,
        },
        Ground::Only(group) => group,
    }
}

/// Whether a clip at `now` of its loop has passed `moment` since it was at `last`, once a loop.
fn passed(last: f32, now: f32, moment: f32) -> bool {
    if now >= last {
        last < moment && moment <= now
    } else {
        moment > last || moment <= now
    }
}

/// The moments of the steps of each motion by its clip's name, upper-cased without its `.GRN`.
#[derive(Resource, Default)]
struct Moments(HashMap<String, Vec<f32>>);

/// Where a walking creature's clip was at the last frame, of its loop.
#[derive(Component)]
struct Stepped(f32);

fn begin(mut commands: Commands, data: Option<Res<Data>>) {
    let mut moments = Moments::default();
    if let Some(motions) = data.as_ref().and_then(|data| data.0.tables.motions().ok()) {
        for motion in motions {
            let [left, right, left_hand, right_hand] = motion.floats_46;
            let mut at = vec![right, left];
            if left_hand != 0.0 || right_hand != 0.0 {
                at.extend([right_hand, left_hand]);
            }
            let name = motion.name.to_ascii_uppercase();
            let name = name.strip_suffix(".GRN").unwrap_or(&name).to_owned();
            if motion.floats_46.iter().any(|&m| m != 0.0) {
                moments.0.insert(name, at);
            }
        }
    }
    commands.insert_resource(moments);
}

/// Plays the steps of the creatures that walk or run, and the Big Demon's jolts.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn step(
    mut commands: Commands,
    moments: Res<Moments>,
    walkers: Query<
        (
            Entity,
            &Npc,
            &Walk,
            &GlobalTransform,
            &crate::ai::Mind,
            Option<&Stepped>,
        ),
        Without<Dies>,
    >,
    players: Query<(&AnimationPlayer, &AnimationGraphHandle)>,
    (graphs, clips): (Res<Assets<AnimationGraph>>, Res<Assets<AnimationClip>>),
    heights: Res<Heights>,
    (mut sfx, mut sounds, mut weather): (
        ResMut<sfx::Sfx>,
        MessageWriter<sfx::Play>,
        Option<ResMut<crate::weather::Weather>>,
    ),
) {
    for (entity, npc, walk, at, mind, stepped) in &walkers {
        let Some((player, node, name, _)) = npc.walking(walk) else {
            continue;
        };
        let Some(at_moments) = moments.0.get(&name.to_ascii_uppercase()) else {
            continue;
        };
        let Ok((player, graph)) = players.get(player) else {
            continue;
        };
        let duration = graphs
            .get(graph)
            .and_then(|graph| graph.get(node))
            .and_then(|node| match &node.node_type {
                AnimationNodeType::Clip(clip) => clips.get(clip),
                _ => None,
            })
            .map_or(0.0, AnimationClip::duration);
        let Some(active) = player.animation(node) else {
            continue;
        };
        if duration <= 0.0 {
            continue;
        }
        let now = (active.seek_time() / duration).rem_euclid(1.0);
        commands.entity(entity).insert(Stepped(now));
        let Some(&Stepped(last)) = stepped else {
            continue;
        };
        let kind = mind.thinks.creature;
        for _ in at_moments
            .iter()
            .filter(|&&moment| passed(last, now, moment))
        {
            if kind == BIG_DEMON
                && let Some(weather) = weather.as_mut()
            {
                weather.set(Event::Jolt(JOLT));
            }
            let Some(sound) = sound(kind) else {
                continue;
            };
            let base = if GIANT_SPIDERS.contains(&kind) {
                0.1
            } else {
                0.4
            };
            let of = match sound {
                Sound::Ground(profile, ground) => {
                    let place = at.translation();
                    let nature = heights.step_nature(place.x, place.z).unwrap_or(0);
                    Of::Profile(profile, group(ground, nature))
                }
                Sound::Wings(pair) => Of::Id(pair[usize::try_from(sfx.dice.below(2)).unwrap_or(0)]),
            };
            sounds.write(sfx::Play {
                sound: of,
                at: Some(entity),
                rate: (base, Roll::Share(0.2)),
            });
        }
    }
}

/// The footsteps of the creatures other than the hero.
pub struct StepsPlugin;

impl Plugin for StepsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(
                Update,
                step.run_if(resource_exists::<Moments>)
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_steps_by_its_sound() {
        assert_eq!(sound(300), Some(Sound::Ground(HUMAN, Ground::Hero)));
        assert_eq!(sound(53), Some(Sound::Ground(BIGGER, Ground::Only(1))));
        assert_eq!(sound(54), Some(Sound::Wings(BIG_WINGS)));
        assert_eq!(sound(551), Some(Sound::Ground(HORSE, Ground::Horse)));
        assert_eq!(sound(1), None);
        assert_eq!(group(Ground::Horse, 0xC), 5);
        assert_eq!(group(Ground::Big, 0x5), 2);
    }

    #[test]
    fn a_moment_is_passed_once_a_loop() {
        assert!(passed(0.1, 0.3, 0.2));
        assert!(!passed(0.2, 0.3, 0.2));
        assert!(passed(0.9, 0.1, 0.95));
        assert!(passed(0.9, 0.1, 0.05));
        assert!(!passed(0.9, 0.1, 0.5));
    }
}
