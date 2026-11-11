//! The health rings on the ground (#2486; docs/re/fight.md, "The health rings"): a creature's ring
//! shows while its timer runs (its `+0x20C`, `opensacred_rules::fight::RING_*`), the hero's while it
//! fights or a hostile creature is near, a hostile creature's for 7 s after the hero strikes it,
//! while it is under the cursor or, with Alt held, in view. The ring lies flat under the creature,
//! drawn before it (`cCreature_render` `0x599910`, `0x4080C0`): a fan of the texture
//! `UI_DISPLAY_GROUNDSYMBOL01` over its health's share of a turn from its near side, in a colour
//! by its level against the hero's (`opensacred_rules::fight::ring_colour`), and the frame
//! `UI_DISPLAY_GROUNDSYMBOL02` whole, red; the hero's in the balance's `Healthbar` and
//! `Healthborder`. Laid over the ground by their alpha, unlit, their depth tested and not
//! written, their alpha fading over the timer's last half second. Never for the dead, the horses,
//! a creature thrown through the air or the demon's horn 6114.
//!
//! Not followed: the champions' and the bosses' turning symbols (`UI_DISPLAY_GROUNDSYMBOL03`
//! and `04`, `0x4087E0`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::fight::{
    RING_FIGHTS, RING_NEAR, RING_NEAR_RANGED, RING_NEAR_WITHIN, RING_STRUCK, ring_alpha,
    ring_colour, ring_half_side, ring_pie,
};

use crate::frame::{Playing, WorldTick};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::hero::Body;
use crate::iso;
use crate::npcs::Dies;

/// The textures of the ring's fill and its frame (`texture.pak` 682 and 683).
const FILL: &str = "UI_DISPLAY_GROUNDSYMBOL01";
const FRAME: &str = "UI_DISPLAY_GROUNDSYMBOL02";
/// The hero's colours where the balance lacks them (`Healthbar`, `Healthborder`, docs/02 §6.3).
const HERO_FILL: u32 = 0xE0FF_8030;
const HERO_FRAME: u32 = 0xFFFF_8030;
/// The demon's horn, which shows no ring but the hero's (`0x5999A6`).
const DEMON_HORN: u32 = 6114;
/// How far over the ground the ring lies, as a burnt ground does (`fx`).
const LIFT: f32 = 0.5;

/// A creature's ring's timer, in seconds of the world's ticks, and its look while it shows.
#[derive(Component, Default)]
pub(crate) struct Ring {
    timer: f32,
    look: Option<Entity>,
}

impl Ring {
    /// Its timer at least `seconds`.
    fn at_least(&mut self, seconds: f32) {
        self.timer = self.timer.max(seconds);
    }
}

/// A ring's look: its fill's mesh and its frame's.
#[derive(Component)]
struct Look {
    fill: Handle<Mesh>,
    frame: Handle<Mesh>,
}

/// Despawns the look of a ring that its creature loses, or loses with it: one despawned while
/// its ring shows, as an undead that falls and rises again is made anew (`undead::remake`,
/// #2540), leaves no ring on the ground.
fn gone(removed: On<Remove<Ring>>, rings: Query<&Ring>, mut commands: Commands) {
    if let Some(look) = rings.get(removed.entity).ok().and_then(|ring| ring.look) {
        commands.entity(look).try_despawn();
    }
}

/// The rings' materials, made at the first ring; none where a texture is missing.
#[derive(Resource, Default)]
struct Materials(Option<Option<[Handle<Effect>; 2]>>);

/// Runs every ring's timer down by the world's tick (`0x537ECD`–`0x537F01`): held at 0, put back
/// to 0 above 100.
fn tick(world: Res<WorldTick>, mut rings: Query<&mut Ring>) {
    let Some(seconds) = world.seconds else {
        return;
    };
    for mut ring in &mut rings {
        ring.timer = if ring.timer > 100.0 {
            0.0
        } else {
            (ring.timer - seconds).max(0.0)
        };
    }
}

/// Sets the hero's ring at each tick: 4 s while it fights, an attack's target, a strike or an art
/// under way; else 1 s while a hostile creature is within 350, or 1,000 with a ranged weapon
/// (`0x547070`), though it held more.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn hero(
    mut commands: Commands,
    world: Res<WorldTick>,
    mut heroes: Query<
        (
            Entity,
            &Transform,
            Option<&mut Ring>,
            Option<&crate::attack::Attack>,
            (Has<crate::attack::Striking>, Has<crate::making::Playing>),
            Option<&crate::fight::Holding>,
        ),
        (With<Body>, Without<Dies>),
    >,
    creatures: Query<(&Transform, &crate::ai::Mind), (Without<Body>, Without<Dies>)>,
) {
    if world.seconds.is_none() {
        return;
    }
    for (entity, at, ring, attack, (striking, making), holding) in &mut heroes {
        let fights = striking || making || attack.is_some_and(|attack| attack.target().is_some());
        let within = if holding.is_some_and(|holding| holding.0.ranged) {
            RING_NEAR_RANGED
        } else {
            RING_NEAR_WITHIN
        };
        let near = || {
            creatures.iter().any(|(place, mind)| {
                mind.hostile() && (place.translation - at.translation).xz().length() <= within
            })
        };
        let timer = if fights {
            Some(RING_FIGHTS)
        } else if near() {
            Some(RING_NEAR)
        } else {
            None
        };
        match (ring, timer) {
            (Some(mut ring), Some(timer)) => ring.timer = timer,
            (None, Some(timer)) => {
                commands.entity(entity).insert(Ring { timer, look: None });
            }
            _ => {}
        }
    }
}

/// Sets a creature's ring for 7 s at each strike of the hero's at it, hit or missed (`0x56B610`);
/// at least 1 s under the cursor (`0x61258A`); and with Alt held, every hostile one's whose ring
/// holds 1 s or less (`0x599ACC`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn mark(
    mut commands: Commands,
    mut hits: MessageReader<crate::fight::Hit>,
    heroes: Query<Entity, With<Body>>,
    mut rings: Query<&mut Ring, Without<Body>>,
    hostiles: Query<(Entity, &crate::ai::Mind), (Without<Body>, Without<Dies>)>,
    (foe, keys): (Res<crate::cursors::Foe>, crate::controls::Keys),
) {
    let hero = heroes.iter().next();
    let mut set = |commands: &mut Commands, target: Entity, timer: f32, exact: bool| match rings
        .get_mut(target)
    {
        Ok(mut ring) if exact => ring.timer = timer,
        Ok(mut ring) => ring.at_least(timer),
        Err(_) => {
            commands.entity(target).insert(Ring { timer, look: None });
        }
    };
    for hit in hits.read() {
        if Some(hit.attacker) == hero && Some(hit.target) != hero && hostiles.contains(hit.target) {
            set(&mut commands, hit.target, RING_STRUCK, true);
        }
    }
    if let Some(target) = foe.0.filter(|&target| hostiles.contains(target)) {
        set(&mut commands, target, RING_NEAR, false);
    }
    if crate::labels::held(&keys) {
        for (target, mind) in &hostiles {
            if mind.hostile() {
                set(&mut commands, target, RING_NEAR, false);
            }
        }
    }
}

/// Lays out each ring that runs, and takes away those that do not (`0x4080C0`): its fill's fan by
/// the health's share and its frame, in their colours at their fading alpha, flat under its
/// creature.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn draw(
    mut commands: Commands,
    mut rings: Query<(
        &mut Ring,
        &GlobalTransform,
        &crate::fight::Health,
        Option<&crate::ai::Mind>,
        Option<&crate::arrows::Size>,
        (
            Has<Body>,
            Has<Dies>,
            Has<crate::riding::Horse>,
            Has<crate::making::Knocked>,
        ),
    )>,
    mut looks: Query<&Look>,
    (data, assets, mut materials): (Option<Res<Data>>, Res<AssetServer>, ResMut<Materials>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
    (rules, experience): (Res<crate::fight::Rules>, Res<crate::fight::Experience>),
    orders: Orders,
) {
    let colours = data
        .as_deref()
        .and_then(|data| data.0.tables.balance().ok())
        .map(|balance| {
            let argb = |name, default: u32| {
                balance
                    .i32(name)
                    .map_or(default, |value| u32::from_ne_bytes(value.to_ne_bytes()))
            };
            (
                argb("Healthbar", HERO_FILL),
                argb("Healthborder", HERO_FRAME),
            )
        })
        .unwrap_or((HERO_FILL, HERO_FRAME));
    for (mut ring, at, health, mind, size, (hero, dying, horse, knocked)) in &mut rings {
        let shows = ring.timer > 0.0
            && health.current > 0
            && !dying
            && !horse
            && !knocked
            && (hero || mind.is_some_and(|mind| mind.hostile() && mind.creature != DEMON_HORN));
        if !shows {
            if let Some(look) = ring.look.take() {
                commands.entity(look).despawn();
            }
            continue;
        }
        let look = match ring.look {
            Some(look) if looks.contains(look) => look,
            _ => {
                let Some([fill_material, frame_material]) = materials
                    .0
                    .get_or_insert_with(|| {
                        let image = |name: &str| {
                            data.as_deref()
                                .and_then(|data| particle_texture(&data.0.assets, &assets, name))
                        };
                        let (fill, frame) = (image(FILL), image(FRAME));
                        if fill.is_none() || frame.is_none() {
                            warn!("health rings: no texture {FILL} or {FRAME}; no ring is drawn");
                        }
                        Some([
                            effects.add(particle_material(fill?, AlphaMode::Blend)),
                            effects.add(particle_material(frame?, AlphaMode::Blend)),
                        ])
                    })
                    .clone()
                else {
                    continue;
                };
                let (fill, frame) = (meshes.add(empty()), meshes.add(empty()));
                let look = commands
                    .spawn((
                        Look {
                            fill: fill.clone(),
                            frame: frame.clone(),
                        },
                        Transform::IDENTITY,
                        Visibility::default(),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .with_children(|parts| {
                        for (mesh, material) in [(fill, fill_material), (frame, frame_material)] {
                            parts.spawn((
                                Mesh3d(mesh),
                                MeshMaterial3d(material),
                                Transform::IDENTITY,
                                NoFrustumCulling,
                                RenderLayers::layer(iso::UNTINTED),
                            ));
                        }
                    })
                    .id();
                ring.look = Some(look);
                look
            }
        };
        let Ok(parts) = looks.get_mut(look) else {
            continue;
        };
        let alpha = f32::from(ring_alpha(ring.timer)) / 255.0;
        let (fill, frame) = if hero {
            (argb(colours.0, alpha), argb(colours.1, alpha))
        } else {
            let level = rules.on_bronze(mind.map_or(1, |mind| mind.level));
            let [r, g, b] = ring_colour(level, experience.0.level, &rules);
            let byte = |c: u8| f32::from(c) / 255.0;
            ([byte(r), byte(g), byte(b), alpha], [1.0, 0.0, 0.0, alpha])
        };
        // Its size, 50 where its type gives none (`arrows::Size`, `0x428CE0`).
        let size = size.map_or(opensacred_rules::arrows::size(0), |size| size.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a creature's size, a small whole number"
        )]
        let half = ring_half_side(size.max(0.0) as u32, hero);
        let r = f32::from(u16::try_from(half).unwrap_or(u16::MAX));
        let place = at.translation() + Vec3::Y * LIFT;
        // In the models' units, as the ring lies in its creature's frame (seen: the Seraphim's
        // ring 140 pixels across at the original's 768 high).
        let (across, down) = iso::fx_axes();
        let point = |[x, y]: [f32; 2]| place + (across * x + down * y) * iso::MODEL_SCALE;
        let uv = |[x, y]: [f32; 2]| Vec2::new((x / r + 1.0) / 2.0, (1.0 - y / r) / 2.0);
        let share = if health.most == 0 {
            0.0
        } else {
            f32::from(u16::try_from(health.current.min(health.most)).unwrap_or(u16::MAX))
                / f32::from(u16::try_from(health.most).unwrap_or(u16::MAX))
        };
        let order = orders.at(at.translation());
        let mut quads = Quads::default();
        let fan = ring_pie(share, r);
        for pair in fan[1..].windows(2) {
            let [a, b] = [pair[0], pair[1]];
            quads.push_corners(
                [point(fan[0]), point(a), point(b), point(b)],
                [uv(fan[0]), uv(a), uv(b), uv(b)],
                fill,
            );
        }
        quads.in_order(order, Vec3::ZERO);
        if let Some(mut mesh) = meshes.get_mut(&parts.fill) {
            quads.into_mesh(&mut mesh);
        }
        let mut quads = Quads::default();
        let corners = [[-r, r], [r, r], [r, -r], [-r, -r]];
        quads.push_corners(corners.map(point), corners.map(uv), frame);
        quads.in_order(order, Vec3::ZERO);
        if let Some(mut mesh) = meshes.get_mut(&parts.frame) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// An ARGB word's colour, its alpha times `alpha`, as the vertices take it.
fn argb(colour: u32, alpha: f32) -> [f32; 4] {
    let [a, r, g, b] = colour.to_be_bytes().map(|c| f32::from(c) / 255.0);
    [r, g, b, a * alpha]
}

/// The health rings.
pub struct HealthRingsPlugin;

impl Plugin for HealthRingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>()
            .add_observer(gone)
            .add_systems(
                OnExit(AppState::InGame),
                |mut materials: ResMut<Materials>| {
                    materials.0 = None;
                },
            )
            .add_systems(
                Update,
                (tick, hero, mark, draw)
                    .chain()
                    .after(crate::frame::Step::Models)
                    .in_set(Playing),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creature_despawned_while_its_ring_shows_takes_the_rings_look_with_it() {
        let mut app = App::new();
        app.add_observer(gone);
        let look = app.world_mut().spawn_empty().id();
        let other = app.world_mut().spawn_empty().id();
        let fallen = app
            .world_mut()
            .spawn(Ring {
                timer: 7.0,
                look: Some(look),
            })
            .id();
        let standing = app
            .world_mut()
            .spawn(Ring {
                timer: 7.0,
                look: Some(other),
            })
            .id();
        app.world_mut().entity_mut(fallen).despawn();
        app.update();
        assert!(app.world().get_entity(look).is_err());
        assert!(app.world().get_entity(standing).is_ok());
        assert!(app.world().get_entity(other).is_ok());
    }
}
