//! The hell gates' looks (1160, 1161, 1181; #2917; docs/re/effects.md, "The hell gates"): the
//! scripts' FX objects at the Underworld's dimension gates and at the gates' heads of the sectors
//! 12,79 and 39,55 (`npcs`). The gate up is fifteen thin lavender threads of glows, 300 high,
//! swaying over pools of blue light, which break up and blink out over 14 s once the scripts
//! signal it (`opensacred_rules::particles::HellgateUp`); the gate down rings of sparks laid flat
//! on the ground, spinning ever faster as they shrink, red laid over by their alpha and one in
//! five yellow added, humming with the tentacles' loop (`HellgateDown`); the head two pulsing red
//! eyes with pale red smoke drifting before them, gone at once when signalled (`HellgateHead`).
//! The threads, the rings and the eyes are untinted by the night, the head's smoke tinted; none
//! lights the world.
//!
//! Not followed: a signal to a gate down, which the original takes for its event again (its
//! point unwritten, its loop started twice; docs/re/effects.md, "The hell gates").

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    Blend, GATE_DOWN_GRID, GATE_DOWN_LOOP, GATE_DOWN_TEXTURE, GATE_HEAD_GRID, GATE_HEAD_TEXTURES,
    GATE_POOL_COLOUR, GATE_UP_TEXTURES, HELLGATE_DOWN, HELLGATE_HEAD, HELLGATE_UP, HellgateDown,
    HellgateHead, HellgateUp, Particle, Ramp, System,
};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;

/// A hell gate that a script made, of its FX type, and the name by which the scripts signal it.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(crate) struct Gate {
    pub kind: u32,
    pub name: Option<String>,
}

impl Gate {
    /// Whether the FX type `kind` is a hell gate's.
    pub(crate) fn is(kind: u32) -> bool {
        [HELLGATE_UP, HELLGATE_DOWN, HELLGATE_HEAD].contains(&kind)
    }
}

/// A gate's run.
enum Run {
    Up(HellgateUp),
    Down(HellgateDown),
    Head(HellgateHead),
}

/// A gate that shows: its run and the meshes of its parts (the gate up's glows and pools, the
/// gate down's red and yellow rings, the head's smoke and its eyes' large and small glows).
#[derive(Component)]
struct Shown {
    run: Run,
    meshes: Vec<Handle<Mesh>>,
}

/// The gates' materials, made as the first gate shows: the gate up's glows and pools, the gate
/// down's red and yellow rings, the head's smoke and its eyes' large and small glows. None where
/// a texture is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    up: [Option<Handle<Effect>>; 2],
    down: [Option<Handle<Effect>>; 2],
    head: [Option<Handle<Effect>>; 3],
}

/// The gates' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x488))
    }
}

/// Finds the gates' textures at the first gate (`0x7BDF50`, `0x7BEA70`, `0x7C6CA0`).
fn find_looks(looks: &mut Looks, data: &Data, server: &AssetServer, effects: &mut Assets<Effect>) {
    looks.made = true;
    let assets = &data.0.assets;
    let mut image = |name: &str| {
        let image = particle_texture(assets, server, name);
        if image.is_none() {
            warn!("hell gates: no {name}; its part is not drawn");
        }
        image
    };
    let [glows, pools] = GATE_UP_TEXTURES.map(&mut image);
    looks.up = [glows, pools].map(|image| image.map(|image| effects.add(whole_material(image))));
    let rings = image(GATE_DOWN_TEXTURE);
    looks.down = HellgateDown::BLENDS.map(|blend| {
        rings.clone().map(|image| {
            effects.add(match blend {
                Blend::Over => particle_material(image, AlphaMode::Blend),
                Blend::Add => particle_material(image, AlphaMode::Add),
                Blend::Whole => whole_material(image),
            })
        })
    });
    let [smoke, large, small] = GATE_HEAD_TEXTURES.map(&mut image);
    looks.head = [
        // The head's smoke, tinted, which the world's camera draws, among the objects (#3459).
        smoke.map(|image| {
            effects.add(crate::fx::among_objects(particle_material(
                image,
                AlphaMode::Blend,
            )))
        }),
        large.map(|image| effects.add(particle_material(image, AlphaMode::Add))),
        small.map(|image| effects.add(whole_material(image))),
    ];
}

/// Gives each new gate its run and its parts' meshes (`0x7BE3A0`, `0x7BEBF0`, `0x7C7040`); the
/// gate down its hum.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn show(
    mut commands: Commands,
    gates: Query<(Entity, &Gate), Without<Shown>>,
    (data, server, mut looks, mut effects): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
    ),
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
) {
    for (entity, gate) in &gates {
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            find_looks(&mut looks, data, &server, &mut effects);
        }
        // Each part's material and whether the night tints it.
        let (run, parts): (Run, Vec<(Option<Handle<Effect>>, bool)>) = match gate.kind {
            HELLGATE_UP => {
                let factor = System::new(0).factor();
                (
                    Run::Up(HellgateUp::new(factor, &mut dice.0)),
                    looks.up.iter().map(|m| (m.clone(), false)).collect(),
                )
            }
            HELLGATE_DOWN => {
                commands
                    .entity(entity)
                    .insert(crate::hums::Hum(GATE_DOWN_LOOP));
                (
                    Run::Down(HellgateDown::default()),
                    looks.down.iter().map(|m| (m.clone(), false)).collect(),
                )
            }
            _ => (
                Run::Head(HellgateHead::default()),
                looks
                    .head
                    .iter()
                    .enumerate()
                    .map(|(index, m)| (m.clone(), index == 0))
                    .collect(),
            ),
        };
        let made = parts
            .into_iter()
            .map(|(material, tinted)| {
                let mesh = meshes.add(empty());
                if let Some(material) = material {
                    commands
                        .spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            Transform::IDENTITY,
                            NoFrustumCulling,
                            ChildOf(entity),
                        ))
                        .insert_if(RenderLayers::layer(iso::UNTINTED), || !tinted);
                }
                mesh
            })
            .collect();
        debug!("hell gates: the gate {} shows", gate.kind);
        commands.entity(entity).insert(Shown { run, meshes: made });
    }
}

/// Hands the scripts' signals to the gates up and the heads of their names (`SetObjState`'s
/// `0x3C`, the cases `0xA4` and `0xB4`).
fn signal(
    mut signals: MessageReader<crate::forge_elements::Signal>,
    mut shown: Query<(&Gate, &mut Shown)>,
) {
    for crate::forge_elements::Signal(name) in signals.read() {
        for (gate, mut shown) in &mut shown {
            if gate.name.as_deref() != Some(name.as_str()) {
                continue;
            }
            match &mut shown.run {
                Run::Up(up) => up.signalled = true,
                Run::Head(head) => head.signalled = true,
                Run::Down(_) => continue,
            }
            debug!("hell gates: the gate {} signalled", gate.kind);
        }
    }
}

/// A particle's square of a texture's grid of `grid` × `grid` by its frame.
fn frame_of(particle: &Particle, grid: u8) -> (Vec2, f32) {
    let side = grid.max(1);
    let corner = Vec2::new(
        f32::from(particle.frame % side),
        f32::from(particle.frame / side % side),
    );
    (corner / f32::from(side), 1.0 / f32::from(side))
}

/// The gate down's flat quads of a layer (`0x761B00` with the identity matrix and the flags
/// `0x10D`): each spark's ring lying in the horizontal plane at its place, turned by its angle
/// about the upright, in its layer's ramp's colour at its life.
fn flat_quads(system: &System, layer: u8, ramp: &Ramp) -> Quads {
    let mut quads = Quads::default();
    for particle in system.particles.iter().filter(|p| p.layer == layer) {
        let (sin, cos) = particle.angle.sin_cos();
        let s = particle.size;
        let corner = |x: f32, y: f32| {
            let [cx, cy, cz] = particle.place;
            world([cx + x * cos - y * sin, cy + x * sin + y * cos, cz])
        };
        let (at, side) = frame_of(particle, GATE_DOWN_GRID);
        quads.push_corners(
            [corner(-s, -s), corner(-s, s), corner(s, s), corner(s, -s)],
            [
                Vec2::new(at.x, at.y + side),
                at,
                Vec2::new(at.x + side, at.y),
                Vec2::new(at.x + side, at.y + side),
            ],
            ramp.at(particle.life),
        );
    }
    quads
}

/// Runs each gate a frame on and draws it (`0x7BDFF0`, `0x7BEB00`, `0x7C6D30`; `0x7BE060`,
/// `0x7BEB50`, `0x7C6E00`); a gate up or a head that has faded goes with its object.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &GlobalTransform)>,
    (mut meshes, orders, mut dice, preroll): (
        ResMut<Assets<Mesh>>,
        Orders,
        ResMut<Dice>,
        Res<crate::fx::Preroll>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut shown, place) in &mut shown {
        let shown = &mut *shown;
        let origin = place.translation();
        let parts: Vec<Quads> = match &mut shown.run {
            Run::Up(up) => {
                if !up.update(dt) {
                    debug!("hell gates: a gate up has faded");
                    commands.entity(entity).despawn();
                    continue;
                }
                let mut glows = Quads::default();
                let mut pools = Quads::default();
                for thread in up.threads.iter().filter(|thread| up.shows(thread)) {
                    for glow in up.glows(thread) {
                        glows.push_facing(world(glow.place), thread.half * s, [glow.colour; 4]);
                    }
                    let ([x, y, z], half) = up.pool(thread);
                    pools.push_corners(
                        [
                            world([x - half, y - half, z]),
                            world([x - half, y + half, z]),
                            world([x + half, y + half, z]),
                            world([x + half, y - half, z]),
                        ],
                        [Vec2::Y, Vec2::ZERO, Vec2::X, Vec2::ONE],
                        GATE_POOL_COLOUR,
                    );
                }
                vec![glows, pools]
            }
            Run::Down(down) => {
                // A load's first frame runs it on first (#3805).
                for (dt, _) in preroll.steps(dt) {
                    down.update(dt, &mut dice.0);
                }
                let ramps = HellgateDown::ramps();
                (0..2u8)
                    .map(|layer| flat_quads(&down.system, layer, &ramps[usize::from(layer)]))
                    .collect()
            }
            Run::Head(head) => {
                if !head.update(dt, &mut dice.0) {
                    debug!("hell gates: a gate's head has gone");
                    commands.entity(entity).despawn();
                    continue;
                }
                let ramp = HellgateHead::ramp();
                let mut smoke = Quads::default();
                for particle in &head.system.particles {
                    smoke.push(
                        world(particle.place),
                        particle.size * s,
                        particle.angle,
                        frame_of(particle, GATE_HEAD_GRID),
                        ramp.at(particle.life),
                    );
                }
                let mut large = Quads::default();
                let mut small = Quads::default();
                for eye in head.eyes() {
                    let at = world(eye.place);
                    large.push_facing(at, eye.large.0 * s, [eye.large.1; 4]);
                    small.push_facing(at, eye.small.0 * s, [eye.small.1; 4]);
                }
                vec![smoke, large, small]
            }
        };
        // The head's eyes and smoke stand far behind its point, within the great head that the
        // statics draw before it: at its place's one depth, over the head, as the original's
        // show; the gates' parts at their own depths no nearer than their place.
        let head = matches!(shown.run, Run::Head(_));
        let order = orders.at(origin);
        for (mut quads, mesh) in parts.into_iter().zip(&shown.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                if head {
                    quads.at_order(order, origin);
                } else {
                    quads.in_order(order, origin);
                }
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

pub struct HellGatesPlugin;

impl Plugin for HellGatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, signal, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
