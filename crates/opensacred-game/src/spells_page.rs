//! The inventory's spells page (#1528; docs/re/arts.md, "The spells page"): the original's
//! `cUI_SpellTab` (`UI_TABPAGE_SPELLS`, `0x6C1720`), the second of the inventory's tabs, at the
//! window's corner (0, 388): each class's fixed picture of its arts (`0x6C2E60`), its three
//! backgrounds and its arts' icons of 64 × 64 at their places, an art the hero holds by its vivid
//! image and one it does not by its own grey picture, each under the frame 174 (`0x6C25A0`); and
//! over an art newly taught or raised (its entry's `TAUGHT` bit) the flash of `FX_BUTTONFLASH`, the
//! bits cleared as the page hides (`0x5809B0`).
//!
//! A left press on a held art takes it onto the cursor, the cursor empty, and a press with an art
//! on the cursor lets it go; a right press on a held art puts it into the bar's selected slot
//! (`0x6C1F30`, `0x6C1930`, `0x6C1AE0`; `art_slots`); the Vampiress's night arts by day are
//! refused.
//!
//! The keys F1 to F6, on their release (#1536): while the page shows, one over a held art binds
//! the key to it, in the page's list and in the hero's entries (`0x580D00`), and the same key over
//! the art that holds it clears it in the list alone, which the page's next show reads back from
//! the entries (`0x6C1E60`, `0x580C80`); elsewhere the key's art goes into the bar's selected
//! slot as the right press puts one, F1 to F6 while the page shows and F1 to F4 alone while it is
//! hidden (`0x6C1F30`). The page writes `F%d` on an art of a key (`0x6C25A0`).
//!
//! The hovered art shows its tooltip with its next level at its icon's (x + 70, y + 32)
//! (`0x6C25A0`, `tooltip::art`, #1696).
//!
//! Not followed yet: the master's window's view of the page (#1540).

use std::collections::HashMap;

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::ShaderRef;
use bevy::text::FontSize;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::UiMaterialKey;

use crate::art_slots::{ArtSlots, HeldArt};
use crate::arts::HeroArts;
use crate::bar::{self, UiImages};
use crate::frame::Unpaused;
use crate::game::{AppState, Data, FONT};
use crate::inventory::{Page, Shown};

/// The page's corner on the original's screen (the inventory's window).
const CORNER: Vec2 = Vec2::new(0.0, 388.0);
/// The page's number among the inventory's tabs.
pub(crate) const PAGE: u8 = 1;
/// An icon's side, the frame over it (`gui_main_05` (192, 192)), the backgrounds' places and tint
/// (`0x6C2BA0`: `0xAFFFFFFF`, the Wood Elf's `0xCFFFFFFF`).
const ICON: f32 = 64.0;
const FRAME: u32 = 174;
const BACKGROUNDS: [f32; 3] = [0.0, 256.0, 512.0];
const TINT: f32 = 175.0 / 255.0;
const WOOD_ELF_TINT: f32 = 207.0 / 255.0;
const WOOD_ELF: u32 = 5;
/// The flash (`0x6C2D50`): its texture's 4 × 4 frames, 13 a second.
pub(crate) const FLASH: &str = "fx_buttonflash";
const FLASH_FRAMES: f32 = 13.0;
/// The arts that a press refuses by day (`0x4D1D60`): the Vampiress's night arts.
const NIGHT_ARTS: [u16; 3] = [1025, 1027, 1029];
const NIGHT_ARTS_FROM: u16 = 1030;
const NIGHT_ARTS_TO: u16 = 1041;
/// The Vampiress's two types, her first eight arts by day and by night, and the place from which
/// her later arts are grey by day.
const VAMPIRESS: [u32; 2] = [6, 7];
const BY_NIGHT: [u16; 8] = [1023, 1025, 1027, 1029, 1038, 1039, 1040, 1041];
const GREY_BY_DAY_FROM: usize = 8;

/// A class's arts at their places on the page, each its id and its icon's corner.
type Places = &'static [(u16, (u16, u16))];

/// The first of a class's three backgrounds and its arts at their places on the page (`0x6C2E60`),
/// by its creature type; the Vampiress's by day.
fn layout(kind: u32) -> Option<(u32, Places)> {
    Some(match kind {
        1 => (
            1487,
            &[
                (21, (44, 20)),
                (1019, (116, 20)),
                (20, (188, 20)),
                (19, (20, 100)),
                (17, (116, 100)),
                (18, (212, 100)),
                (15, (44, 180)),
                (13, (116, 180)),
                (22, (188, 180)),
                (1070, (364, 20)),
                (1071, (436, 20)),
                (1072, (508, 20)),
                (1020, (340, 100)),
                (1054, (436, 100)),
                (1017, (532, 100)),
                (1069, (388, 180)),
                (1018, (484, 180)),
            ],
        ),
        2 => (
            1490,
            &[
                (1003, (16, 16)),
                (1013, (16, 96)),
                (1012, (16, 176)),
                (1010, (108, 96)),
                (1014, (192, 16)),
                (1015, (192, 96)),
                (1016, (192, 176)),
                (1058, (352, 128)),
                (1059, (444, 160)),
                (1057, (536, 128)),
                (1060, (444, 80)),
            ],
        ),
        3 => (
            1493,
            &[
                (5, (8, 20)),
                (24, (60, 70)),
                (23, (8, 120)),
                (11, (60, 170)),
                (2, (136, 20)),
                (25, (188, 70)),
                (16, (136, 120)),
                (26, (188, 170)),
                (28, (264, 20)),
                (29, (316, 70)),
                (1, (264, 120)),
                (33, (316, 170)),
                (27, (392, 20)),
                (34, (444, 70)),
                (35, (392, 120)),
                (4, (444, 170)),
                (7, (520, 20)),
                (30, (572, 70)),
                (31, (520, 120)),
                (32, (572, 170)),
            ],
        ),
        4 => (
            1496,
            &[
                (1042, (32, 16)),
                (1043, (32, 96)),
                (1044, (32, 176)),
                (1045, (96, 16)),
                (1046, (124, 96)),
                (1068, (160, 176)),
                (1065, (176, 16)),
                (1066, (204, 96)),
                (1067, (240, 176)),
                (1051, (464, 16)),
                (1052, (428, 96)),
                (1053, (400, 176)),
                (1048, (544, 16)),
                (1049, (508, 96)),
                (1047, (480, 176)),
                (1050, (548, 176)),
            ],
        ),
        5 => (
            1499,
            &[
                (1006, (16, 44)),
                (36, (16, 124)),
                (1007, (94, 92)),
                (38, (94, 172)),
                (1008, (172, 92)),
                (39, (172, 172)),
                (1004, (250, 60)),
                (40, (250, 140)),
                (1005, (328, 44)),
                (41, (328, 124)),
                (1061, (406, 28)),
                (42, (406, 108)),
                (1062, (484, 28)),
                (37, (484, 108)),
                (1063, (562, 44)),
                (3, (562, 124)),
            ],
        ),
        6 | 7 => (
            1502,
            &[
                (1022, (72, 16)),
                (1024, (152, 16)),
                (1026, (20, 64)),
                (1028, (200, 64)),
                (1073, (112, 96)),
                (1074, (36, 136)),
                (1075, (186, 136)),
                (1076, (112, 176)),
                (1030, (424, 16)),
                (1031, (504, 16)),
                (1032, (372, 64)),
                (1033, (552, 64)),
                (1034, (464, 96)),
                (1035, (388, 136)),
                (1036, (538, 136)),
                (1037, (464, 176)),
            ],
        ),
        8 => (
            1508,
            &[
                (1089, (30, 20)),
                (1083, (116, 20)),
                (1084, (196, 20)),
                (1078, (30, 100)),
                (1079, (116, 100)),
                (1080, (30, 180)),
                (1082, (116, 180)),
                (55, (398, 20)),
                (58, (484, 20)),
                (59, (564, 20)),
                (56, (484, 100)),
                (57, (564, 100)),
                (1085, (398, 180)),
                (1086, (484, 180)),
                (1087, (564, 180)),
            ],
        ),
        9 => (
            1505,
            &[
                (67, (40, 104)),
                (60, (112, 74)),
                (66, (184, 48)),
                (62, (256, 34)),
                (65, (328, 54)),
                (63, (400, 60)),
                (64, (472, 34)),
                (1097, (136, 146)),
                (1100, (208, 162)),
                (1098, (280, 170)),
                (1099, (352, 162)),
                (1094, (424, 146)),
            ],
        ),
        _ => return None,
    })
}

/// A class's arts on the page with their places, the Vampiress's first eight her night's by night
/// (`0x6C2E60(type, night)`).
fn arts_of(kind: u32, night: bool) -> Option<(u32, Vec<(u16, Vec2)>)> {
    let (backgrounds, arts) = layout(kind)?;
    let arts = arts
        .iter()
        .enumerate()
        .map(|(i, &(art, (x, y)))| {
            let art = match BY_NIGHT.get(i) {
                Some(&night_art) if night && VAMPIRESS.contains(&kind) => night_art,
                _ => art,
            };
            (art, Vec2::new(f32::from(x), f32::from(y)))
        })
        .collect();
    Some((backgrounds, arts))
}

/// How an art shows on the page (`0x6C25A0`): held, by its vivid image; held but one of the
/// Vampiress's later arts by day, by its grey image; not held, by its own picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Vivid,
    Grey,
    Own,
}

/// The look of the art at the place `i` of the page of a hero of the type `kind`.
fn look(held: bool, kind: u32, night: bool, i: usize) -> Look {
    if !held {
        Look::Own
    } else if VAMPIRESS.contains(&kind) && !night && i >= GREY_BY_DAY_FROM {
        Look::Grey
    } else {
        Look::Vivid
    }
}

/// Whether a press refuses the art `art` at the hour `night` (`0x4D1D60`): a night art by day.
fn refused(art: u16, night: bool) -> bool {
    !night && (NIGHT_ARTS.contains(&art) || (NIGHT_ARTS_FROM..=NIGHT_ARTS_TO).contains(&art))
}

/// The art held by the hero under the point `at` of the original's screen on the page of a hero of
/// the type `kind`.
fn held_under(arts: &HeroArts, night: bool, at: Vec2) -> Option<u16> {
    let (_, places) = arts_of(arts.kind, night)?;
    places
        .into_iter()
        .find(|&(_, corner)| {
            let corner = CORNER + corner;
            (corner.x..=corner.x + ICON).contains(&at.x)
                && (corner.y..=corner.y + ICON).contains(&at.y)
        })
        .map(|(art, _)| art)
        .filter(|&art| arts.arts.get(art).is_some())
}

/// The art of the page under the point `at` of the original's screen, held or not, and its icon's
/// corner, while the page shows (`0x6C25A0`, its tooltip's, #1696).
pub(crate) fn hovered(
    (shown, page): (&Shown, &Page),
    arts: &HeroArts,
    night: bool,
    at: Vec2,
) -> Option<(u16, Vec2)> {
    if !shown.0 || page.0 != PAGE {
        return None;
    }
    let (_, places) = arts_of(arts.kind, night)?;
    places.into_iter().find_map(|(art, corner)| {
        let corner = CORNER + corner;
        Rect::from_corners(corner, corner + Vec2::splat(ICON))
            .contains(at)
            .then_some((art, corner))
    })
}

/// A press on the page (`0x6C1F30`): the left with an art on the cursor lets it go; with the
/// cursor empty, the held art under it onto the cursor (`0x6C1930`); the right on a held art puts
/// it into the bar's selected slot (`0x6C1AE0`, the taskbar's messages 3 and 4).
fn pick(
    buttons: Res<ButtonInput<MouseButton>>,
    (shown, page): (Res<Shown>, Res<Page>),
    (pointer, ui_scale, windows): (Res<crate::hud::Pointer>, Res<UiScale>, Query<&Window>),
    (arts, forms, pack): (
        Option<Res<HeroArts>>,
        Option<Res<crate::forms::Forms>>,
        Res<crate::pickup::Pack>,
    ),
    (mut held, mut slots): (ResMut<HeldArt>, ResMut<ArtSlots>),
) {
    let (left, right) = (
        buttons.just_pressed(MouseButton::Left),
        buttons.just_pressed(MouseButton::Right),
    );
    if !shown.0 || page.0 != PAGE || !(left || right) {
        return;
    }
    let (Some(arts), Some(at)) = (
        arts,
        crate::inventory::screen_point(&pointer, &ui_scale, &windows),
    ) else {
        return;
    };
    let on_page = Rect::from_corners(CORNER, CORNER + Vec2::new(640.0, 256.0)).contains(at);
    if !on_page {
        return;
    }
    if left && held.0.is_some() {
        held.0 = None;
        return;
    }
    let night = forms.is_some_and(|forms| forms.by_night());
    let Some(art) = held_under(&arts, night, at).filter(|&art| !refused(art, night)) else {
        return;
    };
    if left && pack.held.is_none() {
        held.0 = Some(art);
    } else if right {
        let slot = slots.selected;
        if slots.arts[slot] != Some(art) {
            slots.arts[slot] = Some(art);
        }
    }
}

/// The arts' keys that reach the page while it is hidden: F1 to F4, its test stopping at `0x112`.
const HIDDEN_KEYS: u8 = 4;
/// Where a key's name stands on its art's icon, and its font: the interface's font 1, white
/// (`0x6C25A0`, `0x649BE0`), AntiquaSSK at the size of the foe's panel's (`foe_panel`).
const KEY_AT: Vec2 = Vec2::new(44.0, 42.0);
const KEY_FONT: f32 = 19.5;

/// The page's list of its arts' keys (`+0x2CC`, each entry's word `+0x0C`): the layout it was
/// made for, the hero's type and its form by night, and the arts of a key with their keys' numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Keyed {
    kind: u32,
    night: bool,
    keys: Vec<(u16, u8)>,
}

impl Keyed {
    /// The list made for the hero's layout with the keys of its entries (`0x6C2500`, `0x580C80`):
    /// each art of the page that the hero holds with a shortcut's slot.
    fn read(arts: &HeroArts, night: bool) -> Self {
        let keys = arts_of(arts.kind, night)
            .map(|(_, places)| {
                places
                    .into_iter()
                    .filter_map(|(art, _)| {
                        let slot = arts.arts.get(art)?.slot();
                        (slot > 0).then_some((art, slot))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            kind: arts.kind,
            night,
            keys,
        }
    }

    /// The key's number of the art `art`.
    fn of(&self, art: u16) -> Option<u8> {
        self.keys
            .iter()
            .find(|&&(a, _)| a == art)
            .map(|&(_, key)| key)
    }
}

/// The page's list, none until it is first made: as the page shows, or at a key.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ArtKeys(Option<Keyed>);

/// What a key over the page does (`0x6C1F30`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Keying {
    /// The key bound to the art, in the list and in the hero's entries.
    Bound(u16),
    /// The key taken off the art in the list alone.
    Cleared(u16),
    /// The key's art into the bar's selected slot.
    Selected(u16),
    Nothing,
}

/// The key of the number `key` on the list `list`, over the held art `over` while the page shows
/// (`0x6C1F30`): bound to it, or cleared where it holds the key, the list changed; else the art of
/// the key, if any, for the selected slot.
fn keyed(list: &mut Keyed, key: u8, over: Option<u16>) -> Keying {
    if let Some(art) = over {
        if list.of(art) == Some(key) {
            list.keys.retain(|&(a, _)| a != art);
            return Keying::Cleared(art);
        }
        list.keys.retain(|&(a, k)| a != art && k != key);
        list.keys.push((art, key));
        return Keying::Bound(art);
    }
    list.keys
        .iter()
        .find(|&&(_, k)| k == key)
        .map_or(Keying::Nothing, |&(art, _)| Keying::Selected(art))
}

/// The keys F1 to F6 on their release (`0x6C1F30`, the event of a key's code): over a held art
/// while the page shows, bound to it or cleared; else the key's art into the bar's selected slot,
/// as a right press puts one (`0x6C1AE0`), F5 and F6 only while the page shows.
fn press(
    (keys, console): (crate::controls::Keys, Option<Res<crate::console::Console>>),
    (shown, page): (Res<Shown>, Res<Page>),
    (pointer, ui_scale, windows): (Res<crate::hud::Pointer>, Res<UiScale>, Query<&Window>),
    (arts, forms): (Option<ResMut<HeroArts>>, Option<Res<crate::forms::Forms>>),
    (mut list, mut slots): (ResMut<ArtKeys>, ResMut<ArtSlots>),
) {
    // The keys F1 to F6 by their numbers (`0x65DD10`: the codes `0x10F` to `0x114`, less
    // `0x10E`).
    let Some(key) = (1..)
        .zip(crate::controls::ART_KEYS)
        .find(|&(_, action)| keys.just_released(action))
        .map(|(key, _)| key)
    else {
        return;
    };
    if console.is_some_and(|console| console.is_open()) {
        return;
    }
    let on = shown.0 && page.0 == PAGE;
    let Some(mut arts) = arts.filter(|_| on || key <= HIDDEN_KEYS) else {
        return;
    };
    let night = forms.is_some_and(|forms| forms.by_night());
    let over = crate::inventory::screen_point(&pointer, &ui_scale, &windows)
        .filter(|_| on)
        .and_then(|at| held_under(&arts, night, at));
    let keying = keyed(
        list.0.get_or_insert_with(|| Keyed::read(&arts, night)),
        key,
        over,
    );
    match keying {
        Keying::Bound(art) => {
            arts.arts.bind(art, key);
            info!("spells page: F{key} bound to {art}");
        }
        Keying::Cleared(art) => info!("spells page: F{key} taken off {art}"),
        Keying::Selected(art) if !refused(art, night) => {
            let slot = slots.selected;
            if slots.arts[slot] != Some(art) {
                slots.arts[slot] = Some(art);
                info!("spells page: F{key} puts {art} into the slot {}", slot + 1);
            }
        }
        Keying::Selected(_) | Keying::Nothing => {}
    }
}

/// A new game's list not made yet.
fn reset(mut list: ResMut<ArtKeys>) {
    list.0 = None;
}

/// The files of the arts' own pictures, found once.
#[derive(Resource, Default)]
struct Pictures(HashMap<u16, String>);

/// The page's container.
#[derive(Component)]
struct PageRoot;

const SHADER: &str = "embedded://opensacred_game/flash.wesl";

/// The flash's frame: its part of the texture (corner and size, in the texture's units) added
/// onto the screen (`0x6C2D50`, `ONE` and `ONE`); one for every art that flashes, all in step.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct Flash {
    #[uniform(0)]
    part: Vec4,
    #[texture(1)]
    #[sampler(2)]
    picture: Handle<Image>,
}

impl UiMaterial for Flash {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        // The light added, the screen's alpha kept.
        let target = descriptor
            .fragment
            .as_mut()
            .and_then(|fragment| fragment.targets.first_mut())
            .and_then(Option::as_mut);
        if let Some(target) = target {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            });
        }
    }
}

/// The page's flash, made with the page.
#[derive(Resource)]
struct Flashing(Handle<Flash>);

/// Finds the files of the arts' own pictures (their records' `+0x0C`) once the arts are read.
fn begin(mut commands: Commands, data: Option<Res<Data>>, arts: Res<HeroArts>) {
    let Some(data) = data else {
        return;
    };
    let tables = &arts.tables;
    let named: Vec<(u16, String)> = tables
        .spells
        .iter()
        .map(|spell| (spell.id, spell.icon.clone()))
        .chain(
            tables
                .weapon_arts
                .iter()
                .map(|art| (art.id, art.icon.clone())),
        )
        .map(|(id, icon)| (id, picture_name(&icon)))
        .collect();
    let mut names: Vec<&str> = named.iter().map(|(_, name)| name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let found: HashMap<&str, String> = names
        .iter()
        .copied()
        .zip(crate::hud::find_textures(&data.0.assets, &names))
        .filter_map(|(name, file)| Some((name, file?)))
        .collect();
    let own = named
        .iter()
        .filter_map(|(id, name)| Some((*id, found.get(name.as_str())?.clone())))
        .collect();
    commands.insert_resource(Pictures(own));
}

/// The three images of the art `art` (its record's `+0x30`, `+0x34`, `+0x38`): grey, pale and
/// vivid.
pub(crate) fn sprites(arts: &HeroArts, art: u16) -> Option<[u32; 3]> {
    let tables = &arts.tables;
    tables
        .spells
        .iter()
        .find(|spell| spell.id == art)
        .map(|spell| spell.sprites)
        .or_else(|| {
            tables
                .weapon_arts
                .iter()
                .find(|record| record.id == art)
                .map(|record| record.sprites)
        })
}

/// A texture's name as the files keep it: lower case, without its extension.
fn picture_name(file: &str) -> String {
    let lower = file.to_lowercase();
    lower.strip_suffix(".tga").unwrap_or(&lower).to_owned()
}

/// Makes the page as it shows and as the hero's arts or its form change, and takes it away as it
/// hides, the arts' `TAUGHT` bits then cleared.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn show(
    mut commands: Commands,
    (shown, page): (Res<Shown>, Res<Page>),
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    (mut arts, pictures, forms): (
        Option<ResMut<HeroArts>>,
        Option<Res<Pictures>>,
        Option<Res<crate::forms::Forms>>,
    ),
    roots: Query<Entity, With<PageRoot>>,
    (mut flashes, mut list): (ResMut<Assets<Flash>>, ResMut<ArtKeys>),
    mut was: Local<Option<(u32, bool, Vec<(u16, u8)>, Vec<(u16, u8)>)>>,
) {
    let on = shown.0 && page.0 == PAGE;
    let night = forms.is_some_and(|forms| forms.by_night());
    if on && let Some(arts) = arts.as_deref() {
        let made = list.0.as_ref().map(|keyed| (keyed.kind, keyed.night));
        if was.is_none() {
            // The page's show makes its list and reads the keys (`0x6C1E60`).
            list.0 = Some(Keyed::read(arts, night));
        } else if made != Some((arts.kind, night)) {
            // Its draw makes the list anew for another layout, every key 0 (`0x6C2500`,
            // `0x6C2E60`).
            list.0 = Some(Keyed {
                kind: arts.kind,
                night,
                keys: Vec::new(),
            });
        }
    }
    let keys = list
        .0
        .as_ref()
        .map(|keyed| keyed.keys.clone())
        .unwrap_or_default();
    let now = arts.as_deref().filter(|_| on).map(|arts| {
        let held = arts
            .arts
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.art,
                    u8::from(entry.flags & opensacred_rules::arts::TAUGHT != 0),
                )
            })
            .collect();
        (arts.kind, night, held, keys.clone())
    });
    if *was == now {
        return;
    }
    // The page hidden: the arts' marks of new cleared (`0x5809B0`).
    if now.is_none()
        && was.is_some()
        && let Some(arts) = arts.as_deref_mut()
    {
        for entry in &mut arts.arts.entries {
            entry.flags &= !opensacred_rules::arts::TAUGHT;
        }
    }
    *was = now.clone();
    for root in &roots {
        commands.entity(root).despawn();
    }
    let (Some((kind, night, _, keys)), Some(arts)) = (now, arts.as_deref()) else {
        commands.remove_resource::<Flashing>();
        return;
    };
    let flash = images.file(FLASH).map(|file| {
        let flash = flashes.add(Flash {
            part: Vec4::new(0.0, 0.0, 0.25, 0.25),
            picture: crate::hud::image(&assets, file.to_owned()),
        });
        commands.insert_resource(Flashing(flash.clone()));
        flash
    });
    let Some((backgrounds, places)) = arts_of(kind, night) else {
        return;
    };
    let root = commands
        .spawn((PageRoot, crate::backpack::screen(), GlobalZIndex(-1)))
        .id();
    let tint = if kind == WOOD_ELF {
        WOOD_ELF_TINT
    } else {
        TINT
    };
    for (index, x) in (backgrounds..).zip(BACKGROUNDS) {
        let (mut image, node) = bar::piece(
            &images,
            &assets,
            index,
            CORNER + Vec2::new(x, 0.0),
            Vec2::ZERO,
        );
        image.color = Color::srgba(1.0, 1.0, 1.0, tint);
        let child = commands.spawn((image, node, Pickable::IGNORE)).id();
        commands.entity(root).add_child(child);
    }
    let sized = |(mut image, mut node): (ImageNode, Node)| {
        node.width = Val::Px(ICON);
        node.height = Val::Px(ICON);
        image.image_mode = NodeImageMode::Stretch;
        (image, node)
    };
    for (i, &(art, at)) in places.iter().enumerate() {
        let at = CORNER + at;
        let entry = arts.arts.get(art);
        let record = sprites(arts, art);
        let icon = match (look(entry.is_some(), kind, night, i), record) {
            (Look::Vivid, Some(sprites)) => {
                Some(bar::piece(&images, &assets, sprites[2], at, Vec2::ZERO))
            }
            (Look::Grey, Some(sprites)) => {
                Some(bar::piece(&images, &assets, sprites[0], at, Vec2::ZERO))
            }
            (Look::Own, _) => pictures
                .as_deref()
                .and_then(|pictures| pictures.0.get(&art))
                .map(|file| {
                    (
                        ImageNode::new(crate::hud::image(&assets, file.clone())),
                        bar::placed(at, Vec2::splat(ICON)),
                    )
                }),
            _ => None,
        };
        if let Some(icon) = icon {
            let child = commands.spawn((sized(icon), Pickable::IGNORE)).id();
            commands.entity(root).add_child(child);
        }
        let frame = commands
            .spawn((
                sized(bar::piece(&images, &assets, FRAME, at, Vec2::ZERO)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(frame);
        let new = entry.is_some_and(|entry| entry.flags & opensacred_rules::arts::TAUGHT != 0);
        if let (true, Look::Vivid, Some(flash)) =
            (new, look(entry.is_some(), kind, night, i), &flash)
        {
            let child = commands
                .spawn((
                    MaterialNode(flash.clone()),
                    bar::placed(at, Vec2::splat(ICON)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(root).add_child(child);
        }
        // The key's name over the art (`0x6C25A0`).
        if let Some(&(_, key)) = keys.iter().find(|&&(a, _)| a == art) {
            let child = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(at.x + KEY_AT.x),
                        top: Val::Px(at.y + KEY_AT.y),
                        ..default()
                    },
                    Text::new(format!("F{key}")),
                    TextFont {
                        font: assets.load::<Font>(FONT).into(),
                        font_size: FontSize::Px(KEY_FONT),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::no_wrap(),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(root).add_child(child);
        }
    }
}

/// The flash's frame at `seconds` of the clock, of its texture's 4 × 4: `⌊13t⌋ mod 16`.
fn flash_frame(seconds: f32) -> (u8, u8) {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a frame's number, taken modulo 16"
    )]
    let frame = ((seconds * FLASH_FRAMES).floor().max(0.0) as u64 % 16) as u8;
    (frame % 4, frame / 4)
}

/// Steps the flashes through their frames (`0x6C2D50`).
fn flash(
    time: Res<Time<Real>>,
    flashing: Option<Res<Flashing>>,
    mut flashes: ResMut<Assets<Flash>>,
) {
    let Some(flashing) = flashing else {
        return;
    };
    let (column, row) = flash_frame(time.elapsed_secs());
    if let Some(mut flash) = flashes.get_mut(&flashing.0) {
        flash.part = Vec4::new(f32::from(column) / 4.0, f32::from(row) / 4.0, 0.25, 0.25);
    }
}

/// The inventory's spells page.
pub struct SpellsPagePlugin;

impl Plugin for SpellsPagePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "flash.wesl");
        app.add_plugins(UiMaterialPlugin::<Flash>::default())
            .init_resource::<ArtKeys>()
            .add_systems(OnEnter(AppState::InGame), reset)
            .add_systems(
                Update,
                begin
                    .run_if(resource_exists::<HeroArts>)
                    .run_if(not(resource_exists::<Pictures>)),
            )
            .add_systems(
                Update,
                (pick, press.in_set(Unpaused), show, flash)
                    .chain()
                    .after(crate::inventory::Opened)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_class_has_its_picture_of_arts() {
        let counts =
            [1, 2, 3, 4, 5, 6, 7, 8, 9].map(|kind| layout(kind).map(|(_, arts)| arts.len()));
        assert_eq!(counts, [17, 11, 20, 16, 16, 16, 16, 15, 12].map(Some));
        assert!(layout(10).is_none());
        // The Vampiress's first eight by night, the rest as by day.
        let Some((_, night)) = arts_of(6, true) else {
            panic!("the Vampiress has her page");
        };
        assert_eq!(night[0].0, 1023);
        assert_eq!(night[7].0, 1041);
        assert_eq!(night[8].0, 1030);
        assert_eq!(look(true, 6, false, 8), Look::Grey);
        assert_eq!(look(true, 6, true, 8), Look::Vivid);
        assert_eq!(look(false, 1, false, 0), Look::Own);
    }

    #[test]
    fn the_night_arts_are_refused_by_day() {
        assert!(refused(1025, false));
        assert!(refused(1035, false));
        assert!(!refused(1035, true));
        assert!(!refused(1022, false));
    }

    #[test]
    fn a_key_binds_moves_clears_and_selects_as_the_original_s() {
        let mut list = Keyed {
            kind: 1,
            night: false,
            keys: vec![(1070, 2)],
        };
        // F1 over 1071 binds it; F2 over it moves F2 from 1070, its F1 going.
        assert_eq!(keyed(&mut list, 1, Some(1071)), Keying::Bound(1071));
        assert_eq!(keyed(&mut list, 2, Some(1071)), Keying::Bound(1071));
        assert_eq!(list.keys, [(1071, 2)]);
        // Elsewhere F2 selects 1071, F1 nothing.
        assert_eq!(keyed(&mut list, 2, None), Keying::Selected(1071));
        assert_eq!(keyed(&mut list, 1, None), Keying::Nothing);
        // F2 over the art that holds it clears it.
        assert_eq!(keyed(&mut list, 2, Some(1071)), Keying::Cleared(1071));
        assert!(list.keys.is_empty());
    }

    #[test]
    fn the_flash_steps_13_frames_a_second_round_its_16() {
        assert_eq!(flash_frame(0.0), (0, 0));
        assert_eq!(flash_frame(5.0 / 13.0 + 1e-3), (1, 1));
        assert_eq!(flash_frame(16.0 / 13.0 + 1e-3), (0, 0));
    }
}
