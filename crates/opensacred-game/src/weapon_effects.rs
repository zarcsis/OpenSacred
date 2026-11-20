//! The effects of a weapon in hand (#2922; docs/re/effects.md, "776, a weapon's glow" and "The
//! unique weapons' effects"), as `cWeapon3D`'s switch (`0x5CDB60`) makes them for a model put on
//! (`npcs::Piece`), each following its bones while the model is there:
//!
//! - a weapon in hand whose model has the bone `stdfx_bone01` glows at its bone `weapon_gl01`:
//!   sparks of `PARTICLE_GLOW01`, white turning to its bonuses' style's colour, and a pulsing
//!   flare (`opensacred_rules::weapon_effects::{bonus_style, WeaponGlow}`);
//! - the whip 3073 trails fifty blue glows from its bone `weapon_fx01` (`MagicWhip`);
//! - the wings 4010 and 4092 trail a ribbon of small blue glows in a faint haze from each streak
//!   bone `fx_streak01` on, drawn out along the bone (`MagicStreak`);
//! - the sword 1771 sheds red sparks within 40 of its grip and a line of red glows from its grip
//!   to its bone `weapon_fx01` (`MagicWorms`).
//!
//! All added and untinted by the night, just behind the holder's model in the order; the glow
//! lights the night about it.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{GlowLine, argb};
use opensacred_rules::spawning::Rng;
use opensacred_rules::weapon_effects::{
    self as rules, GLOW_AT, GLOW_BONE, GLOW_FLARE, GLOW_TEXTURE, MagicStreak, MagicWhip,
    MagicWorms, WHIP, WINGS, WORMS_COLOUR, WORMS_SWORD, WeaponGlow,
};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::npcs::Piece;

/// The bones of the whip's trail and of the sword's line (15), and the streak bones' first name.
const FX01: &str = "weapon_fx01";
const STREAKS: [&str; 10] = [
    "fx_streak01",
    "fx_streak02",
    "fx_streak03",
    "fx_streak04",
    "fx_streak05",
    "fx_streak06",
    "fx_streak07",
    "fx_streak08",
    "fx_streak09",
    "fx_streak10",
];
/// The sword's line (`0x40DA80` of `0x77C600`): its density, its glows' half sides from 3 to 4
/// at random, its colour.
const WORMS_LINE: GlowLine = GlowLine {
    density: 0.5,
    half: 3.0,
    argb: 0xFFFF_4040,
};
const WORMS_LINE_MOST: f32 = 4.0;

/// The effects' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x5CD))
    }
}

/// The materials, made at the first effect: the sparks' and the trails' (`PARTICLE_GLOW01`
/// added by their alpha) and the flare's (`PARTICLE_FLARE02` added whole).
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    glows: Option<Handle<Effect>>,
    flare: Option<Handle<Effect>>,
}

impl Looks {
    fn make(&mut self, data: Option<&Data>, assets: &AssetServer, effects: &mut Assets<Effect>) {
        if self.made {
            return;
        }
        self.made = true;
        let Some(data) = data else {
            return;
        };
        let image = |name: &str| {
            let image = particle_texture(&data.0.assets, assets, name);
            if image.is_none() {
                warn!("weapon effects: no texture {name}; it is not drawn");
            }
            image
        };
        // The trails' and the sparks' texture is the glow's (`TRAIL_TEXTURE`, `WORMS_TEXTURE`).
        self.glows =
            image(GLOW_TEXTURE).map(|image| effects.add(particle_material(image, AlphaMode::Add)));
        self.flare = image(GLOW_FLARE).map(|image| effects.add(whole_material(image)));
    }
}

/// A model put on whose bones are still to be looked for, until its scene is in.
#[derive(Component)]
struct Unsought;

/// What a model shows.
enum Kind {
    Glow {
        at: Entity,
        glow: WeaponGlow,
    },
    Whip {
        at: Entity,
        whip: MagicWhip,
        hung: bool,
    },
    Streaks {
        bones: Vec<(Entity, MagicStreak)>,
    },
    Worms {
        line: Option<Entity>,
        worms: MagicWorms,
    },
}

/// A model's effect shown: its model, its kind, and its meshes, the glows' and the flare's.
#[derive(Component)]
struct Shown {
    model: Entity,
    kind: Kind,
    meshes: [Handle<Mesh>; 2],
}

/// Marks each model put on, to look for its bones.
fn mark(mut commands: Commands, pieces: Query<Entity, Added<Piece>>) {
    for piece in &pieces {
        commands.entity(piece).insert(Unsought);
    }
}

/// The bonus slots of a weapon in hand, each its condition and its effect, in the order that
/// `0x5CEA50` reads them: its own record's, then each socketed item's; a creature's of its arms,
/// the hero's of its weapon set's half for that hand.
fn bonus_slots(
    hand: &crate::npcs::Wielded,
    holder: Entity,
    holders: &Query<&crate::npcs::Arms>,
    (sets, tables): (Option<&crate::sets::WeaponSets>, &crate::drops::Tables),
) -> Vec<(u16, u32)> {
    let own = |values: &opensacred_rules::fight::ItemValues| {
        values
            .bonuses
            .iter()
            .map(|bonus| (bonus.condition, bonus.effect))
            .collect::<Vec<_>>()
    };
    match holders.get(holder) {
        Ok(arms) => arms
            .in_hand(hand.slot())
            .map(|(item, values)| own(&tables.own_values(&tables.held_item(item, 1, values))))
            .unwrap_or_default(),
        Err(_) => sets
            .and_then(|sets| {
                let halves = sets.halves.get(sets.selected)?;
                let dropped = halves[hand.half()].as_ref()?;
                let mut slots = own(&tables.own_values(dropped));
                for socketed in &dropped.socketed {
                    slots.extend(own(&socketed.values));
                }
                Some(slots)
            })
            .unwrap_or_default(),
    }
}

/// Looks for the bones of each marked model once its scene is in, and gives it its effect by its
/// type (`0x5CDB60`): the sword 1771's sparks, the bonus's glow at `weapon_gl01` of a weapon in
/// hand whose model has `stdfx_bone01`, the whip 3073's trail or the wings' streaks.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn seek(
    mut commands: Commands,
    pieces: Query<
        (
            Entity,
            &Piece,
            Option<&crate::npcs::Wielded>,
            Option<&crate::models::OrderedWith>,
        ),
        With<Unsought>,
    >,
    (children, names, holders): (Query<&Children>, Query<&Name>, Query<&crate::npcs::Arms>),
    (tables, sets): (
        Option<Res<crate::drops::Tables>>,
        Option<Res<crate::sets::WeaponSets>>,
    ),
    (data, assets, mut looks, mut effects, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (model, piece, wielded, holder) in &pieces {
        if children.get(model).is_err() {
            continue;
        }
        commands.entity(model).remove::<Unsought>();
        let named = |name: &str| {
            children
                .iter_descendants(model)
                .find(|&bone| names.get(bone).is_ok_and(|n| n.as_str() == name))
        };
        let item = piece.item();
        let style = || {
            let (wielded, holder, tables) = (wielded?, holder?, tables.as_deref()?);
            named(GLOW_BONE)?;
            let slots = bonus_slots(wielded, holder.0, &holders, (sets.as_deref(), tables));
            rules::bonus_style(slots)
        };
        let kind = if item == WORMS_SWORD {
            Some(Kind::Worms {
                line: named(FX01),
                worms: MagicWorms::default(),
            })
        } else if let Some(style) = style() {
            Some(Kind::Glow {
                at: named(GLOW_AT).unwrap_or(model),
                glow: WeaponGlow::new(style),
            })
        } else if item == WHIP {
            Some(Kind::Whip {
                at: named(FX01).unwrap_or(model),
                whip: MagicWhip::new(),
                hung: false,
            })
        } else if WINGS.contains(&item) {
            let bones: Vec<(Entity, MagicStreak)> = STREAKS
                .iter()
                .map_while(|name| named(name))
                .map(|bone| (bone, MagicStreak::new()))
                .collect();
            (!bones.is_empty()).then_some(Kind::Streaks { bones })
        } else {
            None
        };
        let Some(kind) = kind else {
            continue;
        };
        looks.make(data.as_deref(), &assets, &mut effects);
        debug!("weapon effects: {model} of {item} shows its effect");
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut shown = commands.spawn((
            Transform::default(),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        shown.with_children(|children| {
            for (mesh, material) in parts.iter().zip([looks.glows.clone(), looks.flare.clone()]) {
                if let Some(material) = material {
                    children.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        RenderLayers::layer(iso::UNTINTED),
                    ));
                }
            }
        });
        // The glow lights the night about it (`+0x9C` 250).
        if matches!(kind, Kind::Glow { .. }) {
            shown.insert(crate::daynight::Light(rules::GLOW_LIGHT));
        }
        shown.insert(Shown {
            model,
            kind,
            meshes: parts,
        });
    }
}

/// Runs and draws each model's effect a frame on at its bones; it goes with the model.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    (places, holders): (Query<&GlobalTransform>, Query<&crate::models::OrderedWith>),
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look, mut transform) in &mut shown {
        let look = &mut *look;
        let Ok(model) = places.get(look.model) else {
            commands.entity(entity).despawn();
            continue;
        };
        let origin = model.translation();
        transform.translation = origin;
        let order = holders
            .get(look.model)
            .map_or_else(|_| orders.at(origin), |holder| orders.behind(holder.0));
        // A bone's place in the models' frame about the model's root.
        let at = |bone: Entity| {
            places
                .get(bone)
                .ok()
                .map(|bone| models(bone.translation() - origin))
        };
        let mut glows = Quads::default();
        let mut flare = Quads::default();
        let facing = |quads: &mut Quads, point: [f32; 3], half: f32, colours: [[f32; 4]; 4]| {
            quads.push_facing(world(point), half * s, colours);
        };
        match &mut look.kind {
            Kind::Glow { at: bone, glow } => {
                glow.update(dt, &mut dice.0);
                if let Some(point) = at(*bone) {
                    let ramp = WeaponGlow::ramp(glow.style);
                    for spark in &glow.system.particles {
                        let place = std::array::from_fn(|i| point[i] + spark.place[i]);
                        facing(&mut glows, place, spark.size, [ramp.at(spark.life); 4]);
                    }
                    flare.push_facing(world(point), glow.flare() * s, [[1.0; 4]; 4]);
                }
            }
            Kind::Whip {
                at: bone,
                whip,
                hung,
            } => {
                // The trail lies in the world, its first point at the bone, hanging from it at
                // first.
                if let Some(point) = places.get(*bone).ok().map(|b| models(b.translation())) {
                    if !*hung {
                        whip.hang_from(point);
                        *hung = true;
                    }
                    whip.update(point);
                    let root = models(origin);
                    for (place, _) in &whip.points {
                        let place = std::array::from_fn(|i| place[i] - root[i]);
                        facing(
                            &mut glows,
                            place,
                            MagicWhip::HALF,
                            MagicWhip::CORNERS.map(argb),
                        );
                    }
                }
            }
            Kind::Streaks { bones } => {
                let root = models(origin);
                for (bone, streak) in bones.iter_mut() {
                    let Ok(at) = places.get(*bone) else {
                        continue;
                    };
                    let axis = models(at.rotation() * Vec3::X * s);
                    streak.update(dt, models(at.translation()), axis);
                    for (place, _) in &streak.points {
                        let place = std::array::from_fn(|i| place[i] - root[i]);
                        facing(
                            &mut glows,
                            place,
                            MagicStreak::HALF,
                            MagicStreak::CORNERS.map(argb),
                        );
                        facing(
                            &mut glows,
                            place,
                            MagicStreak::HAZE,
                            MagicStreak::HAZE_CORNERS.map(argb),
                        );
                    }
                }
            }
            Kind::Worms { line, worms } => {
                worms.update(dt, &mut dice.0);
                let [r, g, b] = WORMS_COLOUR;
                for spark in &worms.system.particles {
                    let colour = [r, g, b, spark.life / 255.0];
                    facing(&mut glows, spark.place, spark.size, [colour; 4]);
                }
                if let Some(end) = line.and_then(at) {
                    for point in WORMS_LINE.points([0.0; 3], end) {
                        let roll = f32::from(u16::try_from(dice.0.below(32_768)).unwrap_or(0));
                        let half =
                            WORMS_LINE.half + roll / 32_767.0 * (WORMS_LINE_MOST - WORMS_LINE.half);
                        facing(&mut glows, point, half, [WORMS_LINE.colour(); 4]);
                    }
                }
            }
        }
        for (mut quads, mesh) in [glows, flare].into_iter().zip(&look.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The effects of a weapon in hand.
pub struct WeaponEffectsPlugin;

impl Plugin for WeaponEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (mark, seek, run).chain().in_set(Playing).in_set(Unpaused),
            );
    }
}
