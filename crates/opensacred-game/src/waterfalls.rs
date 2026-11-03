//! The waterfalls' loop (#1170; docs/re/sound.md, "The sources"): a static of the six types
//! that the sound manager's list of drawn statics names (`0x69F883`, by `0x6A0B30`'s comparison
//! of the type's name) loops `WATERFALL_LOOP` (208) while it stands within 1,150 of the listener
//! in the original's screen pixels, and stops beyond (`0x697410`); the loop plays at the sample's
//! own rate, at the volume 1, panned and fading by the static's place (`0x6934E0`, seen panned
//! to the waterfall's side). The list holds at most 16 statics (`0x6843CC`), so at most 16 loops.
//!
//! Not followed: the list's other statics (those of a sound profile, which take its places too),
//! and that only a static drawn on the screen comes into it.

use std::collections::HashSet;

use anyhow::Result;
use bevy::prelude::*;
use opensacred_data::world::{Sector, StaticObject};

use crate::frame::Playing;
use crate::game::AppState;

/// The types whose names the manager compares (`0x9DFCE0` to `0x9DFC78`).
const NAMES: [&str; 6] = [
    "WAFA_FRONT_MITTE",
    "WAFA_LINKS_MITTE",
    "WAFA_RECHTS_MITTE",
    "WAFA_FRONT_SPRITZER",
    "WAFA_LINKS_UNTEN_RE",
    "WAFA_RECHTS_SPRITZER",
];
/// `WATERFALL_LOOP`.
const LOOP: u32 = 208;
/// How near the listener a waterfall sounds, in the original's screen pixels (`0x47E`).
const NEAR: i32 = 1_150;
/// The list's places (`0x6843CC`).
const PLACES: usize = 16;

/// The waterfalls' types, by their names in `items.ron`.
#[derive(Resource, Default)]
pub struct Types(HashSet<u32>);

impl Types {
    /// Reads the types of [`NAMES`] from `items.ron`; two types share a name.
    pub fn read(tables: &crate::tables::Tables) -> Result<Self> {
        Ok(Self(
            tables
                .items()?
                .iter()
                .filter(|item| NAMES.contains(&item.name.as_str()))
                .map(|item| item.slot)
                .collect(),
        ))
    }
}

/// A waterfall's static, at its place on the ground of its sector.
#[derive(Component)]
pub struct Fall;

/// Where the waterfalls among `sector`'s static objects stand, on the ground (at height 0).
pub fn sources(sector: &Sector, types: &Types) -> Vec<Vec3> {
    fn walk(objects: &[StaticObject], types: &Types, out: &mut Vec<Vec3>) {
        for object in objects {
            if types.0.contains(&object.kind) {
                // `48 (cx − cy)` and `24 (cx + cy)` back to the cell.
                let (across, down) = (object.x as f32 / 48.0, object.y as f32 / 24.0);
                let (cx, cy) = ((across + down) / 2.0, (down - across) / 2.0);
                out.push(crate::iso::ground(cx, cy));
            }
            walk(&object.children, types, out);
        }
    }
    let mut out = Vec::new();
    walk(&sector.statics, types, &mut out);
    out
}

/// The loops that play, each at its waterfall.
#[derive(Resource, Default)]
struct Roars(Vec<(Entity, Handle<bevy_kira_audio::AudioInstance>)>);

/// Each waterfall within [`NEAR`] of the hero loops, up to [`PLACES`]; a loop stops when its
/// waterfall goes or lies further; each plays at its waterfall's place.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn roar(
    mut roars: ResMut<Roars>,
    falls: Query<(Entity, &GlobalTransform), With<Fall>>,
    heroes: Query<&GlobalTransform, With<crate::hero::Body>>,
    mut sfx: ResMut<crate::sfx::Sfx>,
    heard: Res<crate::sfx::Listener>,
    assets: Res<AssetServer>,
    audio: Res<bevy_kira_audio::Audio>,
    mut instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>,
) {
    use bevy_kira_audio::prelude::{AudioControl, AudioTween, PlaybackState};
    let Some(hero) = heroes.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let near = |at: &GlobalTransform| crate::sfx::screen_distance(at.translation(), hero) < NEAR;
    roars.0.retain(|(fall, handle)| {
        // A loop whose instance is not made yet is starting.
        let playing = instances.get(handle).is_none_or(|instance| {
            matches!(
                instance.state(),
                PlaybackState::Playing { .. } | PlaybackState::Queued
            )
        });
        let keep = playing && falls.get(*fall).is_ok_and(|(_, at)| near(at));
        if !keep && let Some(mut instance) = instances.get_mut(handle) {
            instance.stop(AudioTween::default());
        }
        keep
    });
    for (fall, at) in &falls {
        if roars.0.len() >= PLACES {
            break;
        }
        if !near(at) || roars.0.iter().any(|(roaring, _)| *roaring == fall) {
            continue;
        }
        let Some((id, file)) = sfx.with_id(LOOP) else {
            return;
        };
        let sample = sfx.sample(id, file, &assets);
        let handle = audio
            .play(sample)
            .looped()
            .with_volume(crate::sfx::decibels(0.0))
            .handle();
        debug!("waterfalls: the loop starts at {fall}");
        roars.0.push((fall, handle));
    }
    for (fall, handle) in &roars.0 {
        let (Ok((_, at)), Some(mut instance)) = (falls.get(*fall), instances.get_mut(handle))
        else {
            continue;
        };
        let (volume, pan) = crate::sfx::placed_at(at.translation(), Some(hero), *heard);
        instance.set_decibels(crate::sfx::decibels(volume), AudioTween::default());
        instance.set_panning(pan, AudioTween::default());
    }
}

/// Stops the loops as the game ends.
fn end(mut roars: ResMut<Roars>, mut instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>) {
    for (_, handle) in roars.0.drain(..) {
        if let Some(mut instance) = instances.get_mut(&handle) {
            instance.stop(bevy_kira_audio::prelude::AudioTween::default());
        }
    }
}

/// The waterfalls' loops.
pub struct WaterfallsPlugin;

impl Plugin for WaterfallsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Roars>()
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(
                Update,
                roar.in_set(Playing).run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_waterfall_stands_where_its_static_is_drawn() {
        let types = Types([15551].into_iter().collect());
        let object = |kind, x, y| StaticObject {
            cell: 0,
            kind,
            flags: 0,
            x,
            y,
            record: 0,
            type_flags: 0,
            mask: 0,
            sprite: 0,
            miniature: None,
            shadow: None,
            floor: None,
            children: Vec::new(),
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: Vec::new(),
            // The cell (3, 1): `48 (3 − 1)` and `24 (3 + 1)`; another type is no waterfall.
            statics: vec![object(15551, 96, 96), object(15530, 96, 96)],
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let falls = sources(&sector, &types);
        assert_eq!(falls, [crate::iso::ground(3.0, 1.0)]);
    }
}
