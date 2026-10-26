//! The icons over the creatures of a dialogue or a trade (#1039, docs/re/netscript.md,
//! "Dialogues"): a creature whose dialogue a script stamped, or a trader, shows over its head the
//! icon that its dialogue's entry or its trade gives (`0x499E90`, `opensacred_rules::quests::icon`),
//! as `cCreature_render` draws it (`0x599F9B`, `0x4090D0`): a square of the icon's image facing
//! the camera above the head's bone, and the same image added over it in a grey that pulses.
//! The tints that `0x4090D0` gives the square's vertices do not show in the original's frames
//! (seen 2026-10-26: the novice's icon 0, yellow by the code, and a dimmed 10 alike): the square
//! is its image at about a third of its values, the pulse added over it (**hypothesis**: the
//! device's lighting takes the vertices' colours' place, and lights the square without normals by
//! its ambient alone).

use std::collections::HashMap;
use std::path::Path;

use bevy::math::Affine3A;
use bevy::prelude::*;
use opensacred_rules::quests;

use crate::blended::{self, Layer, Sorted};
use crate::frame::Step;
use crate::game::AppState;
use crate::npcs::{Npcs, Talk, Trades};
use crate::{hud, iso};

/// The square, in the models' units, over the head's bone: from −10 to 10 across the screen and
/// from 20 to 40 up it (`0x4090D0`).
const SIDE: f32 = 20.0;
const RAISED: f32 = 30.0;
/// The bone the square stands over, and, for a model without it, how high over its feet the head
/// stands, in world units (**estimate**).
const HEAD: &str = "Bip01 Head";
const HEAD_HEIGHT: f32 = 110.0;
/// The pulse's grey, `(sin(t · 0.005) + 1) · 127` of 255 at `t` milliseconds and 100 more for
/// each of the creature's ids (`0x4097B4`, `0x409B38`), its period 1.26 s.
const PULSE_RATE: f32 = 0.005;
const PULSE_PHASE: f32 = 100.0;
/// The pulse's greys, as many shared materials of each image, the last 254 of 255.
const PULSE_STEPS: u8 = 32;

/// The share of its image's values that the square shows under the pulse (**estimate**: the
/// darkest of the original's frames by day, 2026-10-26).
const SHOWN: f32 = 1.0 / 3.0;

/// The dialogues' and the trades' icons and their images; `0x4090D0` tints 0 to 5 and the
/// trades' yellow, 10 to 15 a dark grey (`0xFF3F3F3F`).
const DIALOGUES: [(i32, &str); 12] = [
    (0, "npc_dialog_01"),
    (1, "npc_dialog_02"),
    (4, "npc_dialog_05"),
    (5, "npc_dialog_07"),
    (10, "npc_dialog_01"),
    (11, "npc_dialog_02"),
    (14, "npc_dialog_05"),
    (15, "npc_dialog_07"),
    (0x20, "npc_dialog_smith"),
    (0x21, "npc_dialog_trader"),
    (0x22, "npc_dialog_combo"),
    (0x23, "npc_dialog_trader"),
];
/// The townsfolk's icons from `0x40` and from `0x80`, untinted.
const TOWNSFOLK: [&str; 27] = [
    "sim_talk",
    "sim_talk_romance",
    "sim_talk_music",
    "sim_talk_weather",
    "sim_talk_politics",
    "sim_talk_money",
    "sim_talk_war",
    "sim_talk_gamble",
    "sim_listen",
    "sim_fight",
    "sim_guard",
    "sim_quest",
    "sim_sleep",
    "sim_panic",
    "sim_door_where",
    "sim_door_panic",
    "sim_walk",
    "sim_nervous",
    "sim_romance",
    "sim_animal_where",
    "sim_visit",
    "sim_going_home",
    "sim_bbq",
    "sim_door_locked",
    "sim_door_wrong",
    "sim_farm",
    "sim_follow",
];
const HURTS: [&str; 4] = ["sim_physical", "sim_fire", "sim_magic", "sim_poison"];
/// The icons that show nothing.
const NONE: [i32; 3] = [8, 13, 126];
/// The image of any other icon, which `0x4090D0` tints red.
const OTHER: &str = "npc_dialog_02";

/// The image of the icon `icon` (`0x4090D0`'s switch): the dialogues', the trades' and the
/// townsfolk's; none for 8, 13 and 126; any other the second dialogues' image.
pub fn image(icon: i32) -> Option<&'static str> {
    if NONE.contains(&icon) {
        return None;
    }
    if let Some(&(_, name)) = DIALOGUES.iter().find(|(value, _)| *value == icon) {
        return Some(name);
    }
    let from = |first: i32, names: &[&'static str]| {
        usize::try_from(icon - first)
            .ok()
            .and_then(|index| names.get(index).copied())
    };
    Some(
        from(0x40, &TOWNSFOLK)
            .or_else(|| from(0x80, &HURTS))
            .unwrap_or(OTHER),
    )
}

/// The icons' textures by their names, found among the assets; none where they are missing.
#[derive(Resource, Default)]
pub struct Files(HashMap<&'static str, String>);

impl Files {
    /// The textures of `assets` of every icon's image.
    pub fn find(assets: &Path) -> Self {
        let mut names: Vec<&'static str> = DIALOGUES
            .iter()
            .map(|&(_, name)| name)
            .chain(TOWNSFOLK)
            .chain(HURTS)
            .collect();
        names.sort_unstable();
        names.dedup();
        let found = hud::find_textures(assets, &names);
        Self(
            names
                .into_iter()
                .zip(found)
                .filter_map(|(name, file)| Some((name, file?)))
                .collect(),
        )
    }
}

/// The icon a creature of the dialogue `talk` and the trades `trades` shows, none without
/// either (`cCreature_render`'s test of `+0x14` bit `0x80000`, which the dialogues' and the
/// trades' records set and `DelDlg` clears): its talk the trader's stamp where the tag `0x64`
/// made it a trader, its entry's icon where the table holds its dialogue.
fn icon_of(talk: Option<&Talk>, trades: Option<&Trades>, npcs: Option<&Npcs>) -> Option<i32> {
    let trades = trades.map_or(0, |trades| trades.0);
    if talk.is_none() && trades == 0 {
        return None;
    }
    let stamp = if trades & quests::TRADER != 0 {
        quests::TRADER_TALK
    } else {
        0
    };
    let entry = talk.and_then(|talk| npcs?.icon(&talk.0));
    Some(quests::icon(stamp, trades, entry))
}

/// The square's mesh, one unit on a side.
#[derive(Resource)]
struct Square(Handle<Mesh>);

/// The squares' materials, shared by the icons: of each image's file, the image's, among the
/// objects, and the pulse's of each of its greys ([`PULSE_STEPS`]), which the pulse's square takes
/// in turn, among the glows, so that it is drawn over the image.
#[derive(Resource, Default)]
struct Looks(HashMap<(String, Option<u8>), Handle<Sorted>>);

impl Looks {
    /// The material of the image of `file`, or of its pulse's grey `step`.
    fn of(
        &mut self,
        file: &str,
        step: Option<u8>,
        assets: &AssetServer,
        materials: &mut Assets<Sorted>,
    ) -> Handle<Sorted> {
        self.0
            .entry((file.to_owned(), step))
            .or_insert_with(|| {
                let (base_color, alpha_mode) = match step {
                    None => (Color::srgb(SHOWN, SHOWN, SHOWN), AlphaMode::Blend),
                    Some(step) => {
                        let grey = f32::from(step) / f32::from(PULSE_STEPS - 1) * 254.0 / 255.0;
                        (Color::srgb(grey, grey, grey), AlphaMode::Add)
                    }
                };
                materials.add(blended::sorted(
                    StandardMaterial {
                        base_color,
                        base_color_texture: Some(assets.load(file.to_owned())),
                        unlit: true,
                        cull_mode: None,
                        alpha_mode,
                        ..default()
                    },
                    if step.is_some() {
                        Layer::Glows
                    } else {
                        Layer::Objects
                    },
                ))
            })
            .clone()
    }
}

/// A creature's icon while it shows: its value, its image's file, its two squares, the image and
/// the pulse, the pulse's grey, and the head's bone once found.
#[derive(Component)]
struct Icon {
    icon: i32,
    file: String,
    squares: [Entity; 2],
    step: u8,
    head: Option<Entity>,
}

fn make_square(mut commands: Commands, meshes: Option<ResMut<Assets<Mesh>>>) {
    if let Some(mut meshes) = meshes {
        commands.insert_resource(Square(meshes.add(Rectangle::new(1.0, 1.0))));
    }
}

/// The pulse's grey at `milliseconds` of a creature of the phase `phase`, as a step of
/// [`PULSE_STEPS`].
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the step is rounded from 0 to the last"
)]
fn pulse_step(milliseconds: f32, phase: f32) -> u8 {
    let wave = (((milliseconds + phase) * PULSE_RATE).sin() + 1.0) / 2.0;
    (wave * f32::from(PULSE_STEPS - 1)).round() as u8
}

/// Gives each creature its icon as its dialogue, its entry's icon and its trades make it, and
/// takes away one that no longer shows.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    files: Option<Res<Files>>,
    square: Option<Res<Square>>,
    npcs: Option<Res<Npcs>>,
    (assets, mut looks): (Res<AssetServer>, ResMut<Looks>),
    materials: Option<ResMut<Assets<Sorted>>>,
    creatures: Query<
        (Entity, Option<&Talk>, Option<&Trades>, Option<&Icon>),
        Or<(With<Talk>, With<Trades>, With<Icon>)>,
    >,
) {
    let (Some(files), Some(square), Some(mut materials)) = (files, square, materials) else {
        return;
    };
    for (creature, talk, trades, shown) in &creatures {
        let wanted = icon_of(talk, trades, npcs.as_deref())
            .and_then(|icon| Some((icon, image(icon)?)))
            .and_then(|(icon, name)| Some((icon, files.0.get(name)?)));
        if shown.map(|shown| shown.icon) == wanted.map(|(icon, ..)| icon) {
            continue;
        }
        if let Some(shown) = shown {
            for square in shown.squares {
                commands.entity(square).despawn();
            }
            commands.entity(creature).remove::<Icon>();
        }
        let Some((icon, file)) = wanted else {
            continue;
        };
        let image = looks.of(file, None, &assets, &mut materials);
        let pulse = looks.of(file, Some(0), &assets, &mut materials);
        let squares = [image, pulse].map(|material| {
            commands
                .spawn((
                    Mesh3d(square.0.clone()),
                    MeshMaterial3d(material),
                    Transform::default(),
                    Visibility::Hidden,
                    ChildOf(creature),
                ))
                .id()
        });
        commands.entity(creature).insert(Icon {
            icon,
            file: file.clone(),
            squares,
            step: 0,
            head: None,
        });
    }
}

/// Stands each icon over its creature's head, facing the camera, and pulses it.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn place(
    time: Res<Time<Real>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut icons: Query<(Entity, &mut Icon)>,
    children: Query<&Children>,
    names: Query<&Name>,
    places: Query<&GlobalTransform>,
    mut squares: Query<(&mut Transform, &mut Visibility, &mut MeshMaterial3d<Sorted>)>,
    (assets, mut looks): (Res<AssetServer>, ResMut<Looks>),
    materials: Option<ResMut<Assets<Sorted>>>,
) {
    let (Some(view), Some(mut materials)) = (cameras.iter().next(), materials) else {
        return;
    };
    let turned = view.rotation();
    let up = turned * Vec3::Y;
    let milliseconds = time.elapsed_secs_wrapped() * 1_000.0;
    for (creature, mut icon) in &mut icons {
        let Ok(at) = places.get(creature) else {
            continue;
        };
        if icon.head.is_none() {
            icon.head = children
                .iter_descendants(creature)
                .find(|&e| names.get(e).is_ok_and(|name| name.as_str() == HEAD));
        }
        let head = icon
            .head
            .and_then(|head| places.get(head).ok())
            .map_or(at.translation() + Vec3::Y * HEAD_HEIGHT, |head| {
                head.translation()
            });
        let centre = head + up * RAISED * iso::MODEL_SCALE;
        let inverse = at.affine().inverse();
        let world = Affine3A::from_scale_rotation_translation(
            Vec3::splat(SIDE * iso::MODEL_SCALE),
            turned,
            centre,
        );
        let local = Transform::from_matrix(Mat4::from(inverse * world));
        for square in icon.squares {
            let Ok((mut transform, mut visibility, _)) = squares.get_mut(square) else {
                continue;
            };
            *transform = local;
            visibility.set_if_neq(Visibility::Inherited);
        }
        // The pulse takes the shared material of its grey, its phase its creature's own.
        let id = u16::try_from(creature.to_bits() % 10_000).unwrap_or(0);
        let step = pulse_step(milliseconds, f32::from(id) * PULSE_PHASE);
        if step != icon.step
            && let Ok((_, _, mut material)) = squares.get_mut(icon.squares[1])
        {
            material.0 = looks.of(&icon.file, Some(step), &assets, &mut materials);
            icon.step = step;
        }
    }
}

/// The icons over the creatures.
pub struct IconsPlugin;

impl Plugin for IconsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .add_systems(Startup, make_square)
            .add_systems(
                Update,
                (show, place)
                    .chain()
                    .in_set(Step::Npcs)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_takes_its_image_by_the_originals_switch() {
        // The novice's "?!", a dimmed one, a smith's, a townsperson's and a hurt's.
        assert_eq!(image(0), Some("npc_dialog_01"));
        assert_eq!(image(11), Some("npc_dialog_02"));
        assert_eq!(image(0x20), Some("npc_dialog_smith"));
        assert_eq!(image(0x40), Some("sim_talk"));
        assert_eq!(image(0x5A), Some("sim_follow"));
        assert_eq!(image(0x83), Some("sim_poison"));
        // None for 8, 13 and 126; any other a "!", the popups' images and −1 among them.
        assert_eq!(image(8), None);
        assert_eq!(image(126), None);
        assert_eq!(image(-1), Some(OTHER));
        assert_eq!(image(1043), Some(OTHER));
        assert_eq!(image(0x5B), Some(OTHER));
    }

    #[test]
    fn the_pulse_goes_through_its_greys_in_about_a_second_and_a_quarter() {
        assert_eq!(pulse_step(0.0, 0.0), 16);
        // A quarter of 2π / 0.005 ms later, at its brightest; three quarters, at its darkest.
        assert_eq!(pulse_step(314.159, 0.0), PULSE_STEPS - 1);
        assert_eq!(pulse_step(942.478, 0.0), 0);
        assert_eq!(pulse_step(0.0, 314.159), PULSE_STEPS - 1);
    }

    #[test]
    fn a_creature_shows_an_icon_by_its_dialogue_or_its_trades() {
        let talk = Talk("auftrag10".into());
        assert_eq!(icon_of(None, None, None), None);
        // A dialogue that the table does not hold shows its entry's default.
        assert_eq!(icon_of(Some(&talk), None, None), Some(0));
        assert_eq!(
            icon_of(None, Some(&Trades(quests::MERCHANT)), None),
            Some(0x21)
        );
        // The trader of the tag `0x64` shows its stamp's.
        assert_eq!(
            icon_of(Some(&talk), Some(&Trades(quests::TRADER)), None),
            Some(4)
        );
    }
}
