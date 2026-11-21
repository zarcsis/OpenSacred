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
//! Each wants its place 3 left and 1 above its box's top left corner, the box the rectangle that
//! the pick takes of it (`under_cursor::View`): a living creature's of its type's width and height about
//! its feet (`0x428850`), a dead one's, a container's and a special's its model's bounds on the
//! screen, an item's the 35 × 35 square about its point (the least box of `0x44CF20`); they are
//! laid out in turn from the top of the screen by the
//! original's search (`cLabelLayout`, `0x6620F0`): up to 128 tries, sideways by 8 and upright by
//! 4 more each pair, up before down, a place touching another not free, a label of no free place
//! left out. The text under the cursor is light yellow. With nothing under the cursor's
//! rectangles, the last label under it picks what it names (`0x6270A0`, `under_cursor`): a click on an
//! item's label takes its item, a click on a creature's or a container's is a click on it, its
//! attack, its talk or its opening, and the cursor's look over a label is what it names. While
//! the hero's sets know the enemies (set 3 at 4 pieces, `UI_KNOW_ENEMY`), the foe that the cursor
//! picks tells its values under its name ([`foe_values`]).
//!
//! A ridden horse's label is its rider's, about the horse's halved box, and the rider's own is
//! not drawn (`0x627303`, `0x6276D9`; **seen**, #2927); a horse standing alone, of the bit 26,
//! takes none. Not followed: the stargate 1180's (#3271); the townsfolk's doings' icons under Alt are
//! `icons`' (#2234).

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
/// The layout's tries (`0x6620F0`) and its steps by the try (`0x963A2C`), each times half the
/// try's number: left, right, up, down.
const TRIES: usize = 128;
const STEPS: [Vec2; 4] = [
    Vec2::new(-4.0, 0.0),
    Vec2::new(4.0, 0.0),
    Vec2::new(0.0, -2.0),
    Vec2::new(0.0, 2.0),
];
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
/// it names an item; with nothing under the cursor's rectangles, the last label under it picks
/// what it names (`0x6270A0`, `under_cursor`).
#[derive(Resource, Default, Debug)]
pub struct Shown(Vec<(Entity, Rect, bool)>);

impl Shown {
    /// What the last label in the list under the point `at` names, if any (`0x6270A0`).
    pub fn last_under(&self, at: Vec2) -> Option<Entity> {
        self.0
            .iter()
            .rev()
            .find(|&&(_, rect, _)| rect.contains(at))
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

/// The wanted top left of the label of the box `rect` on the screen, its sides held at the least
/// about their middles (`0x44D3BB`, `0x6272C8`).
fn corner_of(rect: opensacred_rules::pick::Rect) -> Vec2 {
    Vec2::from(rect.held().at) + FROM_BOX
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
        (
            Option<&'static ScriptName>,
            Option<&'static crate::npcs::Called>,
        ),
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
pub(crate) fn hero_name(
    texts: &Texts,
    request: Option<&crate::game::Request>,
    class: u32,
) -> Option<String> {
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
                (Option<&ScriptName>, Option<&crate::npcs::Called>),
            ),
            Without<crate::ai::Mind>,
        >,
    ),
    (request, arts): (
        Option<Res<crate::game::Request>>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    (foe, holding, fighters, rules, horses): (
        Res<crate::cursors::Foe>,
        Query<&crate::fight::Holding, With<crate::hero::Body>>,
        Query<(&crate::fight::Health, &crate::fight::Combat)>,
        Res<crate::fight::Rules>,
        Query<(
            Entity,
            &GlobalTransform,
            &crate::riding::Ridden,
            &crate::ai::Mind,
        )>,
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
        let view = crate::under_cursor::View::new(*camera, scale);
        let small = |at: Vec3| view.item(at).map(corner_of);
        let tall = |at: Vec3, kind: u32| view.creature(at, kind, false).map(corner_of);
        let modelled = |root: Entity, at: Vec3| {
            view.model(&points.bounds_points(root, &meshes), at)
                .map(corner_of)
        };
        let chest = |at: Vec3| view.chest(at).map(corner_of);
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
        // A rider's label is laid about its horse's halved box and names the horse, the horse's
        // link giving the rider's text (`0x627303`, `0x6276D9`, #2927).
        let mount = |rider: Entity| {
            horses
                .iter()
                .find(|(_, _, ridden, _)| ridden.rider == rider)
                .and_then(|(horse, at, _, mind)| {
                    let corner = view
                        .creature(at.translation(), mind.creature, true)
                        .map(corner_of)?;
                    Some((horse, corner))
                })
        };
        let class = arts.as_deref().map_or(1, |arts| arts.kind);
        let hero = heroes.iter().next();
        let hero_text = hero_name(texts, request.as_deref(), class);
        if let (Some((entity, at)), Some(text)) = (hero, hero_text.clone())
            && let Some((entity, corner)) = mount(entity)
                .or_else(|| tall(at.translation(), class).map(|corner| (entity, corner)))
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
        for (entity, at, mind, keys, follower, dies) in &creatures {
            if skipped.contains(&mind.creature) {
                continue;
            }
            // A dead creature's box of its model as it falls and lies (`+0xB0` bit 2, which its
            // death sets, `0x53FB4C`), a living one's of its type's size; a rider's its horse's.
            let ridden = mount(entity);
            let named = ridden.map_or(entity, |(horse, _)| horse);
            let corner = if let Some((_, corner)) = ridden {
                Some(corner)
            } else if dies {
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
                let text =
                    crate::npcs::name_of_creature(texts, keys, mind.creature).map(str::to_owned);
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
                entity: named,
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
        for (entity, at, corpse, keys) in &corpses {
            if skipped.contains(&corpse.creature) {
                continue;
            }
            let corner = modelled(entity, at.translation())
                .or_else(|| tall(at.translation(), corpse.creature));
            let Some(corner) = corner else {
                continue;
            };
            let named = crate::npcs::name_of_creature(texts, keys, corpse.creature);
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

/// The labels on Alt.
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
    fn a_labels_box_holds_its_least_side_about_its_middle() {
        use opensacred_rules::pick::Rect;
        // A point: the 35 × 35 square about it, the label 20 left and 18 above it.
        assert_eq!(
            corner_of(Rect::between([100.0, 100.0], [100.0, 100.0])),
            Vec2::new(80.0, 82.0)
        );
        // A tall box keeps its top, its narrow side widened about its middle.
        assert_eq!(
            corner_of(Rect::between([100.0, 50.0], [120.0, 250.0])),
            Vec2::new(90.0, 49.0)
        );
    }

    #[test]
    fn a_label_under_the_cursor_names_what_it_labels_the_last_of_two() {
        let item = Entity::PLACEHOLDER;
        let creature = Entity::from_raw_u32(7).expect("an entity");
        let shown = Shown(vec![
            (item, Rect::new(10.0, 10.0, 60.0, 30.0), true),
            (creature, Rect::new(10.0, 25.0, 60.0, 60.0), false),
        ]);
        assert_eq!(shown.last_under(Vec2::new(20.0, 15.0)), Some(item));
        assert_eq!(shown.last_under(Vec2::new(70.0, 20.0)), None);
        // Where two lie, the last (`0x6270A0`).
        assert_eq!(shown.last_under(Vec2::new(20.0, 28.0)), Some(creature));
    }
}
