//! The labels while Alt is held (`cWorldView_drawOverheadLabels`, `0x627180`; docs/re/items.md,
//! "On the ground", and docs/re/interface.md, "The labels on Alt"): one list of the items in
//! view, lying or still in flight (#1409, #1647), the hero, the creatures and the containers
//! (#1425), each its text on a dark translucent panel (`0xCF303030`):
//!
//! - an item its name in its name's colour (`tooltip::name_colour`), for gold its amount and its
//!   type's name;
//! - the hero its name, white; a creature its script's name or its type's, light red where
//!   hostile, grey once dead, else white, but those of a type of the items table's bit 26
//!   (`0x43F5F0`: the animals, the horses, the doors), and a follower its master's;
//! - a container not emptied its type's name in its records' colour (`0xFFEFEFEF`);
//! - a special (the signposts, `npcs::Special`) its script's name or its type's, in the same.
//!
//! Each wants its place 3 left and 1 above its box's top left corner, the box a living creature's
//! of its type's width and height about its feet (`0x428850`), a dead one's, a container's and a
//! special's its model's bounds on the screen ([`model_box`]), an item's the 35 × 35 square about
//! its point (the least box of `0x44CF20`); they are
//! laid out in turn from the top of the screen by the
//! original's search (`cLabelLayout`, `0x6620F0`): up to 128 tries, sideways by 8 and upright by
//! 4 more each pair, up before down, a place touching another not free, a label of no free place
//! left out. The text under the cursor is light yellow. A left click on an item's label takes its
//! item (`0x6270A0`, `pickup::pick`), and with Alt held an item comes before a foe under the
//! cursor (`attack::aim`); a click on a creature's or a container's label is a click on it, its
//! attack, its talk or its opening (`0x631CEB`). The cursor's look over a label is what it names
//! (`cursors`). While the hero's sets know the enemies (set 3 at 4 pieces, `UI_KNOW_ENEMY`), the
//! foe that the cursor picks tells its values under its name ([`foe_values`]).
//!
//! Not followed yet: the mounts' labels (#1822) and the stargate 1180's; the townsfolk's doings'
//! icons under Alt are `icons`' (#2234).

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use opensacred_rules::drops::Kind;

use crate::drops::{Lying, Tables};
use crate::game::{AppState, FONT};
use crate::lines::Texts;
use crate::npcs::{ScriptName, TextName};

/// The labels' panel (`0xCF303030`), their text's size and line and the panel's margin about it,
/// in the original's pixels (**estimate**, seen 2026-10-29: a potion's name 167 pixels wide on a
/// panel 18 high, the labels of items on one spot 22 pixels apart).
const PANEL: Color = Color::srgba_u8(0x30, 0x30, 0x30, 0xCF);
const SIZE: f32 = 18.0;
const LINE: f32 = 16.0;
const MARGIN: Vec2 = Vec2::new(3.0, 1.0);
/// A label's height, its line and the panel's margins, the font's `A` (`0x627600`).
const HEIGHT: f32 = LINE + 2.0 * MARGIN.y;
/// The width of a character of a label not yet laid out, before its text is measured
/// (**estimate**).
const CHARACTER: f32 = 8.0;
/// Where a label wants its panel's top left, from its box's (`0x6272C8`).
const FROM_BOX: Vec2 = Vec2::new(-3.0, -1.0);
/// The bounds of a model that the original takes for wrong and replaces by a box of 10 about its
/// point (`0x44D19x`): wider than 1,500 across, or both its ends of an axis on the ground farther
/// than 400 from the point to one side.
const WIDEST: f32 = 1500.0;
const FARTHEST: f32 = 400.0;
const FALLBACK: f32 = 10.0;

/// The world box of a model of the points `points` about its place `at` (`0x44CF20`, the bounds
/// that its draw grows from its meshes' vertices, `vertices::ModelPoints::bounds_points`): their
/// least and most, unless the original takes them for wrong ([`WIDEST`], [`FARTHEST`]); none
/// for a model of no points, not drawn yet.
fn model_box(points: &[Vec3], at: Vec3) -> Option<(Vec3, Vec3)> {
    let first = *points.first()?;
    let (low, high) = points
        .iter()
        .fold((first, first), |(low, high), &p| (low.min(p), high.max(p)));
    let off = |low: f32, high: f32, at: f32| low - at > FARTHEST || at - high > FARTHEST;
    if high.x - low.x > WIDEST || off(low.x, high.x, at.x) || off(low.z, high.z, at.z) {
        return Some((at - Vec3::splat(FALLBACK), at + Vec3::splat(FALLBACK)));
    }
    Some((low, high))
}

/// The least side of a box, about its middle (`0x44D3BB`).
const LEAST_BOX: f32 = 35.0;
/// A creature's box about its feet on the screen, of its type's half width and height
/// ([`creature_size`]) in the original's pixels at the zoom 1 by the factors across and upright
/// that the original's frame gives (**estimate**, measured 2026-11-02 beside the original at the
/// Seraphim's start, of the default 20 and 170: her label's panel 38 left of her feet and 150
/// above them): from `HEIGHT_DOWN` of its height below its feet to its height above them.
const HEIGHT_DOWN: f32 = 1.0 / 6.0;
const ACROSS: f32 = 1.75;
const UPRIGHT: f32 = 0.876;

/// The half width and the height of a creature of the type `kind`, its box's about its point
/// (`0x428850`, docs/re/interface.md, "The labels on Alt"): the sizes the code gives some types,
/// else 20 and 170.
fn creature_size(kind: u32) -> (f32, f32) {
    let (half, height): (u16, u16) = match kind {
        35 | 56..=59 => (60, 110),
        37 | 41..=43 | 67 => (25, 180),
        39 | 40 | 182 | 185 | 187 | 189 | 250..=252 => (30, 195),
        45 | 60..=62 | 68 => (25, 190),
        49 | 51 | 137 => (30, 230),
        52 | 74 | 88 | 100..=103 | 247 | 336 | 354 => (100, 280),
        53 | 94 | 183 => (30, 430),
        54 | 167 => (40, 230),
        55 => (75, 280),
        69 | 191 | 195 => (30, 70),
        71 | 513..=516 | 560..=563 | 565 | 566 | 589 | 590 | 594..=596 => (20, 20),
        72 | 261 | 523..=528 | 530..=533 | 536..=538 | 540 | 541 | 550..=558 => (60, 160),
        75 => (40, 40),
        190 => (60, 70),
        193 | 335 | 512 | 520..=522 | 543..=545 | 567..=570 => (50, 80),
        1249 => (15, 50),
        _ => (20, 170),
    };
    (f32::from(half), f32::from(height))
}
/// A container's box about its point on the screen at the zoom 1, which the original takes of its
/// model's bounds (`+0xD8` to `+0xEC`): half its width and its height above its point
/// (**estimate**, measured 2026-11-02 beside the original at the Seraphim's start's chest).
const CONTAINER_BOX: Vec2 = Vec2::new(35.0, 57.0);
/// The layout's tries (`0x6620F0`) and its steps by the try (`0x963A2C`), each times half the
/// try's number: left, right, up, down.
const TRIES: usize = 128;
const STEPS: [Vec2; 4] = [
    Vec2::new(-4.0, 0.0),
    Vec2::new(4.0, 0.0),
    Vec2::new(0.0, -2.0),
    Vec2::new(0.0, 2.0),
];
/// How far beyond the screen an object's box still takes a label (the push's margin, `0x62FAD5`).
const BEYOND: f32 = 200.0;
/// The texts' colours: a creature hostile either way to the hero, a dead one, any other, a
/// container's, and the text under the cursor (`0x55D444`, `0x5C13C0`, `0x62796D`).
const HOSTILE: u32 = 0xFFFF_A0A0;
const DEAD: u32 = 0xFFA0_A0A0;
const WHITE: u32 = 0xFFFF_FFFF;
const CONTAINER: u32 = 0xFFEF_EFEF;
const UNDER_CURSOR: u32 = 0xFFFF_FF80;
/// The items table's bit of the types that take no label (`0x43F5F0`, the first word's bit 26).
const NO_LABEL: u32 = 0x0400_0000;

/// The labels shown, each what it names, its rectangle in the window's logical pixels and whether
/// it names an item, which a click on one takes: an item's (`pickup::pick`), and a creature's or a
/// container's as a click on its body (`attack::aim`, `talk`, `containers`; the pick's rectangles
/// replaced by the labels', `0x631CEB`).
#[derive(Resource, Default, Debug)]
pub struct Shown(Vec<(Entity, Rect, bool)>);

impl Shown {
    /// The item whose label lies under the point `at` of the window, if any.
    pub fn under(&self, at: Vec2) -> Option<Entity> {
        self.0
            .iter()
            .find(|&&(_, rect, item)| item && rect.contains(at))
            .map(|&(item, ..)| item)
    }

    /// The creature or the container whose label lies under the point `at`, if any.
    pub fn labelled(&self, at: Vec2) -> Option<Entity> {
        self.0
            .iter()
            .find(|&&(_, rect, item)| !item && rect.contains(at))
            .map(|&(entity, ..)| entity)
    }
}

/// Whether the labels' key is held, Alt by default (`GetAsyncKeyState` of `VK_MENU`, the world
/// view's flag `0x4000`).
pub fn held(keys: &crate::controls::Keys) -> bool {
    keys.pressed(crate::controls::Action::Labels)
}

/// A label's node, by what it names.
#[derive(Component)]
struct Label(Entity);

/// A label wanted: what it names, its panel's wanted top left in the original's pixels, its text
/// and colour, and whether it names an item, which a click on it takes.
struct Wanted {
    entity: Entity,
    corner: Vec2,
    text: String,
    colour: u32,
    item: bool,
}

/// The wanted top left of the label of a box of `box_min` to `box_max` on the screen, its sides
/// held at the least about their middles (`0x44D3BB`, `0x6272C8`).
fn corner_of(box_min: Vec2, box_max: Vec2) -> Vec2 {
    let side = |low: f32, high: f32| {
        if high - low < LEAST_BOX {
            (low + high) / 2.0 - (LEAST_BOX / 2.0).floor()
        } else {
            low
        }
    };
    Vec2::new(side(box_min.x, box_max.x), side(box_min.y, box_max.y)) + FROM_BOX
}

/// The types that take no label, of the items table's bit 26 ([`NO_LABEL`]).
fn unlabelled(data: &crate::game::Data) -> HashSet<u32> {
    data.0.tables.items().map_or_else(
        |_| HashSet::new(),
        |items| {
            items
                .iter()
                .filter(|item| item.flags() & NO_LABEL != 0)
                .map(|item| item.slot)
                .collect()
        },
    )
}

/// The text of an item's label: its name (`cursors::item_name`), or for gold the amount and its
/// pile's name (`0x627180`).
fn text(
    (texts, tables): (&Texts, Option<&Tables>),
    lying: &Lying,
    name: Option<&TextName>,
) -> Option<String> {
    let dropped = &lying.dropped;
    let named =
        crate::cursors::item_name(texts, tables, dropped, name.map(|name| name.0.as_str()))?;
    Some(if dropped.kind == Kind::Gold {
        format!("{} {named}", dropped.amount)
    } else {
        named.to_owned()
    })
}

/// Whether two labels overlap, touching edges counted (`0x6620F0`).
fn overlap(a: Rect, b: Rect) -> bool {
    a.min.x <= b.max.x && b.min.x <= a.max.x && a.min.y <= b.max.y && b.min.y <= a.max.y
}

/// Lays out labels of the sizes `sizes`, each wanted with its top left at its corner of
/// `corners`, in turn from the top of the screen and from the left (**hypothesis**: the original
/// takes its objects' draw order, from the back), each at the first free place of the original's
/// search (`cLabelLayout`, `0x6620F0`): the try `k` moved by [`STEPS`]' `k mod 4` times `⌊k/2⌋`,
/// so 2 up, 2 down, 8 left, 8 right, 6 up and on, a place touching a laid label not free; a label
/// of no free place among the [`TRIES`] is left out. Seen: the labels of items on one spot stand
/// in a column, their left edges together.
fn lay_out(corners: &[Vec2], sizes: &[Vec2]) -> Vec<Option<Rect>> {
    let mut order: Vec<usize> = (0..corners.len().min(sizes.len())).collect();
    order.sort_by(|&a, &b| {
        (corners[a].y, corners[a].x)
            .partial_cmp(&(corners[b].y, corners[b].x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut laid = vec![None; order.len()];
    let mut placed: Vec<Rect> = Vec::with_capacity(order.len());
    for index in order {
        let found = (0..TRIES).find_map(|k| {
            let times = f32::from(u8::try_from(k / 2).unwrap_or(u8::MAX));
            let corner = corners[index] + STEPS[k % 4] * times;
            let rect = Rect::from_corners(corner, corner + sizes[index]);
            (!placed.iter().any(|&other| overlap(other, rect))).then_some(rect)
        });
        if let Some(rect) = found {
            placed.push(rect);
            laid[index] = Some(rect);
        }
    }
    laid
}

/// The creatures that may take a label: their place, their mind, their script's name, their
/// master where they follow one, and whether they die.
type Creatures<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GlobalTransform,
        &'static crate::ai::Mind,
        Option<&'static ScriptName>,
        Option<&'static crate::followers::Follower>,
        Has<crate::npcs::Dies>,
    ),
    Without<crate::hero::Body>,
>;

/// The values that a foe's label tells under its name while the hero's sets know the enemies
/// (`0x627180`, its format `0x95FBA4`): its health; its attack and defence times the difficulty's
/// `aw`, as the getters of a creature give them (`0x56AA10`, `0x56AA90`); each kind's mean
/// damage, `(l·F + m·F)/2` of its multiplier `F` (`0x563FD0` to `0x5641B0`), the halving of a
/// hand without a weapon not followed; and each resistance times its factor; each cut. The
/// values of `combat` hold the multipliers and the factors already.
fn foe_values(health: u32, combat: &opensacred_rules::fight::Combat, aw: f32) -> String {
    let damage: [i64; 4] = std::array::from_fn(|k| {
        let [least, most] = combat.damage[k];
        cut((least + most) * 0.5)
    });
    let resistance: [i64; 4] = combat.resistance.map(cut);
    let [dp, df, dm, dg] = damage;
    let [rp, rf, rm, rg] = resistance;
    format!(
        "HP:{health} AW:{} VW:{}\nDP:{dp} DF:{df} DM:{dm} DG:{dg}\nRP:{rp} RF:{rf} RM:{rm} RG:{rg}",
        cut(combat.attack * aw),
        cut(combat.defence * aw)
    )
}

/// A value cut toward zero, as the original's `__ftol` stores it.
fn cut(value: f32) -> i64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a creature's value, far within an i64"
    )]
    let whole = value.trunc() as i64;
    whole
}

/// The hero's label's text: its name, the player's where given, else its class's.
fn hero_name(texts: &Texts, request: Option<&crate::game::Request>, class: u32) -> Option<String> {
    request
        .and_then(|request| request.0.name.clone())
        .or_else(|| texts.named(&class.to_string()).map(str::to_owned))
}

/// Shows the labels of the items, the hero, the creatures and the containers in view while Alt
/// is held, and takes them away as it is let go.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn label(
    mut commands: Commands,
    keys: crate::controls::Keys,
    (texts, tables, data): (
        Option<Res<Texts>>,
        Option<Res<Tables>>,
        Option<Res<crate::game::Data>>,
    ),
    (assets, ui_scale, pointer): (Res<AssetServer>, Res<UiScale>, Res<crate::hud::Pointer>),
    camera: Single<(&Camera, &GlobalTransform, &Projection), crate::iso::Eye<With<Camera>>>,
    items: Query<(Entity, &GlobalTransform, &Lying, Option<&TextName>)>,
    (creatures, heroes, containers, specials): (
        Creatures,
        Query<(Entity, &GlobalTransform), With<crate::hero::Body>>,
        Query<(Entity, &GlobalTransform, &crate::containers::Container)>,
        Query<(
            Entity,
            &GlobalTransform,
            &crate::npcs::Special,
            Option<&TextName>,
        )>,
    ),
    (points, meshes, corpses): (
        crate::vertices::ModelPoints,
        Res<Assets<Mesh>>,
        Query<
            (
                Entity,
                &GlobalTransform,
                &crate::npcs::Corpse,
                Option<&ScriptName>,
            ),
            Without<crate::ai::Mind>,
        >,
    ),
    (request, arts): (
        Option<Res<crate::game::Request>>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    (foe, holding, fighters, rules): (
        Res<crate::cursors::Foe>,
        Query<&crate::fight::Holding, With<crate::hero::Body>>,
        Query<(&crate::fight::Health, &crate::fight::Combat)>,
        Res<crate::fight::Rules>,
    ),
    mut labels: Query<(
        Entity,
        &Label,
        &mut Node,
        &mut Text,
        &mut TextColor,
        &ComputedNode,
    )>,
    (mut shown, mut skipped): (ResMut<Shown>, Local<Option<HashSet<u32>>>),
) {
    let scale = ui_scale.0.max(0.01);
    let mut wanted: Vec<Wanted> = Vec::new();
    if let Some(texts) = texts.as_deref().filter(|_| held(&keys)) {
        let skipped =
            skipped.get_or_insert_with(|| data.as_deref().map(unlabelled).unwrap_or_default());
        let (camera, view, projection) = *camera;
        // The world's scale on the screen: 2 shows twice the world (`iso::zoom`).
        let zoom = match projection {
            Projection::Orthographic(ortho) => ortho.scale.max(0.01),
            _ => 1.0,
        };
        let size = camera.logical_viewport_size().unwrap_or(Vec2::ZERO);
        let project = |at: Vec3| camera.world_to_viewport(view, at).ok();
        let near = |point: Vec2| {
            point.cmpge(Vec2::splat(-BEYOND)).all() && point.cmplt(size + BEYOND).all()
        };
        // An item's or a container's box: the least square about its point.
        let small = |at: Vec3| {
            let point = project(at).filter(|&point| near(point))? / scale;
            Some(corner_of(point, point))
        };
        // A creature's box: its type's half width across and its height up from below its feet.
        let tall = |at: Vec3, kind: u32| {
            let feet = project(at).filter(|&point| near(point))? / scale;
            let (half, height) = creature_size(kind);
            let half = half * ACROSS / zoom;
            let height = height * UPRIGHT / zoom;
            let low = feet - Vec2::new(half, height);
            let high = feet + Vec2::new(half, height * HEIGHT_DOWN);
            Some(corner_of(low, high))
        };
        // A model's box: the screen's box of its bounds' eight corners (`0x44CF20`, `0x623B60`).
        let modelled = |root: Entity, at: Vec3| {
            project(at).filter(|&point| near(point))?;
            let (low, high) = model_box(&points.bounds_points(root, &meshes), at)?;
            let corners = (0..8).filter_map(|k: u8| {
                let pick = |bit: u8, low: f32, high: f32| if k & bit == 0 { low } else { high };
                project(Vec3::new(
                    pick(1, low.x, high.x),
                    pick(2, low.y, high.y),
                    pick(4, low.z, high.z),
                ))
            });
            let (min, max) = corners.fold(
                (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
                |(min, max), p| (min.min(p), max.max(p)),
            );
            (min.x <= max.x).then(|| corner_of(min / scale, max / scale))
        };
        // A container's box before its model is drawn: a chest's about its point.
        let chest = |at: Vec3| {
            let point = project(at).filter(|&point| near(point))? / scale;
            let low = point - CONTAINER_BOX / zoom;
            let high = point + Vec2::new(CONTAINER_BOX.x, 0.0) / zoom;
            Some(corner_of(low, high))
        };
        for (item, at, lying, name) in &items {
            let Some(corner) = small(at.translation()) else {
                continue;
            };
            let Some(text) = text((texts, tables.as_deref()), lying, name) else {
                continue;
            };
            let colour = tables.as_deref().map_or(WHITE, |tables| {
                crate::tooltip::name_colour(tables, &lying.dropped)
            });
            wanted.push(Wanted {
                entity: item,
                corner,
                text,
                colour,
                item: true,
            });
        }
        let class = arts.as_deref().map_or(1, |arts| arts.kind);
        let hero = heroes.iter().next();
        let hero_text = hero_name(texts, request.as_deref(), class);
        if let (Some((entity, at)), Some(text)) = (hero, hero_text.clone())
            && let Some(corner) = tall(at.translation(), class)
        {
            wanted.push(Wanted {
                entity,
                corner,
                text,
                colour: WHITE,
                item: false,
            });
        }
        let knows_enemies = holding
            .iter()
            .next()
            .is_some_and(|holding| holding.0.sets.knows_enemies());
        for (entity, at, mind, key, follower, dies) in &creatures {
            if skipped.contains(&mind.creature) {
                continue;
            }
            // A dead creature's box of its model as it falls and lies (`+0xB0` bit 2, which its
            // death sets, `0x53FB4C`), a living one's of its type's size.
            let corner = if dies {
                modelled(entity, at.translation()).or_else(|| tall(at.translation(), mind.creature))
            } else {
                tall(at.translation(), mind.creature)
            };
            let Some(corner) = corner else {
                continue;
            };
            // A follower's label is its master's, the hero's (its `+0x1EC`, `0x627303`).
            let master_is_hero = follower
                .is_some_and(|follower| hero.is_some_and(|(hero, _)| hero == follower.master));
            let (text, colour) = if master_is_hero {
                (hero_text.clone(), WHITE)
            } else {
                let text = key
                    .and_then(|key| texts.named(crate::talk::resource(&key.0)))
                    .or_else(|| texts.named(&mind.creature.to_string()))
                    .map(str::to_owned);
                let colour = if dies {
                    DEAD
                } else if mind.hostile() {
                    HOSTILE
                } else {
                    WHITE
                };
                (text, colour)
            };
            let Some(mut text) = text else {
                continue;
            };
            // The living foe that the cursor picks, while the hero's sets know the enemies, tells
            // its values under its name (`0x62737F`, `0x6279E8`).
            if foe.0 == Some(entity)
                && !dies
                && mind.hostile()
                && knows_enemies
                && let Ok((health, combat)) = fighters.get(entity)
            {
                text.push('\n');
                text.push_str(&foe_values(health.current, &combat.0, rules.creatures_aw()));
            }
            wanted.push(Wanted {
                entity,
                corner,
                text,
                colour,
                item: false,
            });
        }
        for (entity, at, container) in &containers {
            if container.emptied || skipped.contains(&container.kind()) {
                continue;
            }
            let Some(corner) =
                modelled(entity, at.translation()).or_else(|| chest(at.translation()))
            else {
                continue;
            };
            let Some(text) = texts.named(&container.kind().to_string()) else {
                continue;
            };
            wanted.push(Wanted {
                entity,
                corner,
                text: text.to_owned(),
                colour: CONTAINER,
                item: false,
            });
        }
        // The dead lying, their minds gone (`npcs::die`), grey about their models as they lie.
        for (entity, at, corpse, key) in &corpses {
            if skipped.contains(&corpse.creature) {
                continue;
            }
            let corner = modelled(entity, at.translation())
                .or_else(|| tall(at.translation(), corpse.creature));
            let Some(corner) = corner else {
                continue;
            };
            let named = key
                .and_then(|key| texts.named(crate::talk::resource(&key.0)))
                .or_else(|| texts.named(&corpse.creature.to_string()));
            let Some(text) = named else {
                continue;
            };
            wanted.push(Wanted {
                entity,
                corner,
                text: text.to_owned(),
                colour: DEAD,
                item: false,
            });
        }
        // The specials, the signposts: their script's name or their type's, in the records'
        // colour, about their models (`0x451300`, `0x5C13C0`).
        for (entity, at, special, name) in &specials {
            if skipped.contains(&special.0) {
                continue;
            }
            let Some(corner) = modelled(entity, at.translation()) else {
                continue;
            };
            let named = name
                .and_then(|name| texts.named(&name.0))
                .or_else(|| texts.named(&special.0.to_string()))
                .filter(|text| !text.is_empty());
            let Some(text) = named else {
                continue;
            };
            wanted.push(Wanted {
                entity,
                corner,
                text: text.to_owned(),
                colour: CONTAINER,
                item: false,
            });
        }
    }
    // The sizes of the labels laid out before, in the original's pixels (the node's inverse scale
    // takes the window's and `UiScale` both); a new one's estimated.
    // Each laid out a line high, a foe's lines of values below its name's line as they draw
    // (`0x627180` measures the text's width alone).
    let measured: HashMap<Entity, Vec2> = labels
        .iter()
        .map(|(_, label, _, _, _, computed)| {
            let size = computed.size() * computed.inverse_scale_factor();
            (label.0, Vec2::new(size.x, size.y.min(HEIGHT)))
        })
        .collect();
    let corners: Vec<Vec2> = wanted.iter().map(|wanted| wanted.corner).collect();
    let sizes: Vec<Vec2> = wanted
        .iter()
        .map(|wanted| {
            measured
                .get(&wanted.entity)
                .copied()
                .filter(|size| size.x > 0.0)
                .unwrap_or_else(|| {
                    let count = wanted.text.chars().count();
                    let characters = f32::from(u16::try_from(count).unwrap_or(0));
                    Vec2::new(characters * CHARACTER + 2.0 * MARGIN.x, HEIGHT)
                })
        })
        .collect();
    let rects = lay_out(&corners, &sizes);
    let mut nodes: HashMap<Entity, Entity> = HashMap::new();
    for (entity, label, ..) in &labels {
        let kept = wanted
            .iter()
            .zip(&rects)
            .any(|(wanted, rect)| wanted.entity == label.0 && rect.is_some());
        if kept {
            nodes.insert(label.0, entity);
        } else {
            commands.entity(entity).despawn();
        }
    }
    let cursor = pointer.0.map(|at| at / scale);
    shown.0.clear();
    for (wanted, rect) in wanted.iter().zip(&rects) {
        let Some(rect) = *rect else {
            continue;
        };
        shown.0.push((
            wanted.entity,
            Rect::from_corners(rect.min * scale, rect.max * scale),
            wanted.item,
        ));
        let under = cursor.is_some_and(|at| at.cmpge(rect.min).all() && at.cmple(rect.max).all());
        let colour = crate::tooltip::argb(if under { UNDER_CURSOR } else { wanted.colour });
        let name = &wanted.text;
        if let Some(&node) = nodes.get(&wanted.entity)
            && let Ok((_, _, mut at, mut words, mut tint, _)) = labels.get_mut(node)
        {
            at.left = Val::Px(rect.min.x);
            at.top = Val::Px(rect.min.y);
            if words.0 != *name {
                name.clone_into(&mut words.0);
            }
            if tint.0 != colour {
                tint.0 = colour;
            }
            continue;
        }
        commands.spawn((
            Label(wanted.entity),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.min.x),
                top: Val::Px(rect.min.y),
                padding: UiRect::axes(Val::Px(MARGIN.x), Val::Px(MARGIN.y)),
                ..default()
            },
            BackgroundColor(PANEL),
            Text::new(name.clone()),
            TextFont {
                font: assets.load(FONT).into(),
                font_size: FontSize::Px(SIZE),
                ..default()
            },
            bevy::text::LineHeight::Px(LINE),
            TextColor(colour),
            GlobalZIndex(9),
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// The items' labels on Alt.
pub struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shown>().add_systems(
            Update,
            label
                .run_if(in_state(AppState::InGame))
                .before(crate::frame::Control),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_foes_values_are_cut_as_the_original_writes_them() {
        // The silver wolf of level 4 seen at the smith of 50,44 on bronze (`aw` 0.65): its `A` and
        // `D`, its damage 3.5 to 5 and resistances 5.218, 1.353, 0, 0.56 times its multipliers
        // and factors, as its combat holds them (HP:57 AW:10 VW:13, DP:5, RP:5 RF:1).
        let combat = opensacred_rules::fight::Combat {
            attack: 16.5,
            defence: 20.4,
            damage: [[4.333, 6.19], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [5.824, 1.510, 0.0, 0.630],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
            multipliers: [1.238, 1.312, 1.12, 1.201],
            resistance_factors: [1.116, 1.116, 1.102, 1.125],
            spells: [1.0; 4],
            cast: opensacred_rules::fight::CAST_SPEED,
            attributes: [0.0; 6],
        };
        assert_eq!(
            foe_values(57, &combat, 0.65),
            "HP:57 AW:10 VW:13\nDP:5 DF:0 DM:0 DG:0\nRP:5 RF:1 RM:0 RG:0"
        );
    }

    #[test]
    fn a_creatures_box_takes_its_types_size() {
        // The smith and the heroes the default; the dragon, the wolf and the chickens their own.
        assert_eq!(creature_size(688), (20.0, 170.0));
        assert_eq!(creature_size(1), (20.0, 170.0));
        assert_eq!(creature_size(52), (100.0, 280.0));
        assert_eq!(creature_size(512), (50.0, 80.0));
        assert_eq!(
            [560, 565, 566, 564].map(creature_size)[..3],
            [(20.0, 20.0); 3]
        );
        assert_eq!(creature_size(564), (20.0, 170.0));
    }

    #[test]
    fn labels_of_one_spot_stand_in_a_column_up_and_down_and_others_stay_where_they_are() {
        let size = Vec2::new(100.0, HEIGHT);
        let spot = Vec2::new(500.0, 300.0);
        let corners = [spot, spot, spot, spot, spot, Vec2::new(200.0, 300.0)];
        let rects: Vec<Rect> = lay_out(&corners, &[size; 6])
            .into_iter()
            .map(|rect| rect.expect("a free place"))
            .collect();
        // The first keeps its place; the others go 22 up, 22 down, 42 up and 42 down, their
        // left edges together, as the original's search takes them (#1425).
        let tops: Vec<f32> = rects[..5].iter().map(|rect| rect.min.y - spot.y).collect();
        assert_eq!(tops, [0.0, -22.0, 22.0, -42.0, 42.0]);
        assert!(rects[..5].iter().all(|rect| rect.min.x == spot.x));
        // One far off keeps its place.
        assert_eq!(rects[5].min, corners[5]);
    }

    #[test]
    fn a_models_box_is_its_points_bounds_unless_they_lie_wrong() {
        let at = Vec3::new(1000.0, 0.0, 2000.0);
        let points = [
            Vec3::new(990.0, 0.0, 1980.0),
            Vec3::new(1030.0, 90.0, 2010.0),
        ];
        assert_eq!(
            model_box(&points, at),
            Some((
                Vec3::new(990.0, 0.0, 1980.0),
                Vec3::new(1030.0, 90.0, 2010.0)
            ))
        );
        // Both ends 400 off to one side: a box of 10 about the point.
        let off = [
            Vec3::new(1401.0, 0.0, 2000.0),
            Vec3::new(1500.0, 50.0, 2000.0),
        ];
        assert_eq!(
            model_box(&off, at),
            Some((at - Vec3::splat(10.0), at + Vec3::splat(10.0)))
        );
        assert_eq!(model_box(&[], at), None);
    }

    #[test]
    fn a_labels_box_holds_its_least_side_about_its_middle() {
        // A point: the 35 × 35 square about it, the label 20 left and 18 above it.
        assert_eq!(
            corner_of(Vec2::new(100.0, 100.0), Vec2::new(100.0, 100.0)),
            Vec2::new(80.0, 82.0)
        );
        // A tall box keeps its top, its narrow side widened about its middle.
        assert_eq!(
            corner_of(Vec2::new(100.0, 50.0), Vec2::new(120.0, 250.0)),
            Vec2::new(90.0, 49.0)
        );
    }

    #[test]
    fn a_click_on_a_label_finds_its_item() {
        let item = Entity::PLACEHOLDER;
        let creature = Entity::from_raw_u32(7).expect("an entity");
        let shown = Shown(vec![
            (item, Rect::new(10.0, 10.0, 60.0, 30.0), true),
            (creature, Rect::new(10.0, 40.0, 60.0, 60.0), false),
        ]);
        assert_eq!(shown.under(Vec2::new(20.0, 20.0)), Some(item));
        assert_eq!(shown.under(Vec2::new(70.0, 20.0)), None);
        // A creature's label is no item's, and picks its creature.
        assert_eq!(shown.under(Vec2::new(20.0, 50.0)), None);
        assert_eq!(shown.labelled(Vec2::new(20.0, 50.0)), Some(creature));
    }
}
