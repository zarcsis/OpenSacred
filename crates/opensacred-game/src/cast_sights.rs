//! The sights of the hero's casts in his hands (791, `FX_CAST_SPELL`, #1092; docs/re/effects.md,
//! "The casts' sights"): as a spell's clip starts (`0x51E110`) or the Daemon's arts' (`0x51B46E`),
//! each of its sights follows its bone, its particles left where they are born along the hand's
//! way, or a glow swelling and shrinking at it, or the twirls on the ground, all untinted by the
//! night (`opensacred_rules::particles::{CastSight, cast_sights, art_sights}`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::arts::{self, Style};
use opensacred_rules::particles::{self, Blend, CastBone, CastSight};

use crate::bolt_looks::{models, world};
use crate::fight::Combat;
use crate::frame::{Playing as Running, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::making::{Making, Playing};
use crate::spawning::Rng;

/// The bones of each place a sight follows, the first its model has.
fn bones(bone: CastBone) -> &'static [&'static str] {
    match bone {
        CastBone::Left => &["Bip01 L Finger3", "Bip01 L Hand"],
        CastBone::Right => &["Bip01 R Finger3", "Bip01 R Hand"],
        CastBone::Root => &["__Root"],
        CastBone::LeftFoot => &["Bip01 L Foot"],
        CastBone::RightFoot => &["Bip01 R Foot"],
    }
}

/// The glows' textures by their style (`0x762E10`): 7 `PARTICLE_GLOW07`, 8 `PARTICLE_FLARE04`;
/// and the twirls'.
const GLOWS: [&str; 2] = ["PARTICLE_GLOW07", "PARTICLE_FLARE04"];
const TWIRL: &str = "FX_TWIRL_RAYS";
/// The twirls' colour, added whole (`0xFF804122`).
const TWIRL_COLOUR: [f32; 4] = [128.0 / 255.0, 65.0 / 255.0, 34.0 / 255.0, 1.0];

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x791))
    }
}

/// The sights' materials by their texture's name and their blend, made as first asked.
#[derive(Resource, Default)]
struct Looks(Vec<(Key, Option<Handle<Effect>>)>);

/// A material's key: its texture's name, and whether it is added whole.
type Key = (&'static str, bool);

impl Looks {
    fn get(
        &mut self,
        (name, whole): Key,
        data: Option<&Data>,
        assets: &AssetServer,
        effects: &mut Assets<Effect>,
    ) -> Option<Handle<Effect>> {
        if let Some((_, made)) = self.0.iter().find(|(key, _)| *key == (name, whole)) {
            return made.clone();
        }
        let image = data.and_then(|data| particle_texture(&data.0.assets, assets, name));
        if image.is_none() {
            warn!("cast sights: no texture {name}; its part is not drawn");
        }
        let made = image.map(|image| {
            if whole {
                effects.add(whole_material(image))
            } else {
                effects.add(particle_material(image, AlphaMode::Blend))
            }
        });
        self.0.push(((name, whole), made.clone()));
        made
    }
}

/// A sight: its caster and its bone, its system, its bone's place the frame before, and its
/// meshes of its particles and of its glow or twirls.
#[derive(Component)]
struct Sight {
    caster: Option<Entity>,
    bone: CastBone,
    look: CastSight,
    place: Vec3,
    before: Option<Vec3>,
    meshes: [Handle<Mesh>; 2],
}

/// Makes the sights of each art or spell the hero starts: a spell's cast's on its clip's rate,
/// the casts' speed over 100, the Daemon's arts' at 1.
fn start(
    mut commands: Commands,
    started: Query<(Entity, &Making, Option<&Combat>, &GlobalTransform), Added<Playing>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (caster, making, combat, place) in &started {
        let (sights, rate) = if matches!(arts::style(making.art), Some(Style::Cast(_))) {
            (
                particles::cast_sights(making.art, false),
                combat.map_or(1.2, |combat| combat.cast / 100.0),
            )
        } else {
            (particles::art_sights(making.art), 1.0)
        };
        for (bone, style) in sights {
            let (blend, _) = particles::cast_blend(style);
            let dots = looks.get(
                (particles::cast_texture(style), blend == Blend::Whole),
                data.as_deref(),
                &assets,
                &mut effects,
            );
            let glow = if style == 22 {
                looks.get((TWIRL, true), data.as_deref(), &assets, &mut effects)
            } else {
                let look = CastSight::new(style, 1.0);
                look.glow_style().and_then(|glow| {
                    let name = if glow == 7 { GLOWS[0] } else { GLOWS[1] };
                    looks.get((name, true), data.as_deref(), &assets, &mut effects)
                })
            };
            let parts = [meshes.add(empty()), meshes.add(empty())];
            let mut sight = commands.spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ));
            for (mesh, material) in parts.iter().zip([dots, glow]) {
                if let Some(material) = material {
                    sight.with_child((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        RenderLayers::layer(iso::UNTINTED),
                    ));
                }
            }
            debug!(
                "cast sights: {} makes style {style} at {bone:?}",
                making.art
            );
            sight.insert(Sight {
                caster: Some(caster),
                bone,
                look: CastSight::new(style, rate),
                place: place.translation(),
                before: None,
                meshes: parts,
            });
        }
    }
}

/// Runs each sight a frame on at its bone (`0x776BB0`, `0x776CA0`): its particles born along the
/// bone's way and left there, its glow at the bone, its twirls on the ground; it ends with its
/// last particle past its time.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Sight)>,
    (places, children, bones_of, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut sight) in &mut sights {
        let sight = &mut *sight;
        match sight.caster.map(|caster| places.get(caster)) {
            Some(Ok(at)) => {
                sight.place = crate::making::bone(
                    sight.caster.unwrap_or(entity),
                    bones(sight.bone),
                    &children,
                    (&bones_of, &worn),
                )
                .unwrap_or(at.translation());
            }
            _ => {
                if sight.caster.take().is_some() {
                    sight.look.end();
                }
            }
        }
        let before = sight.before.unwrap_or(sight.place);
        sight.before = Some(sight.place);
        sight
            .look
            .update(dt, (models(before), models(sight.place)), &mut dice.0);
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let ramp = sight.look.ramp();
        let (_, turned) = particles::cast_blend(sight.look.style);
        let mut dots = Quads::default();
        for particle in &sight.look.system.particles {
            dots.push(
                world(particle.place),
                particle.size * s,
                if turned { particle.angle } else { 0.0 },
                (Vec2::ZERO, 1.0),
                sight.look.colour(particle, &ramp),
            );
        }
        let mut glow = Quads::default();
        if let Some((_, half, colour)) = sight.look.glow() {
            glow.push(sight.place, half * s, 0.0, (Vec2::ZERO, 1.0), colour);
        }
        // The style 22's two twirls flat on the ground at the root, turning as they shrink.
        if let Some(twirls) = sight.look.twirls() {
            let (across, down) = iso::fx_axes();
            for (half, turn) in twirls {
                let (sin, cos) = turn.sin_cos();
                let (a, b) = (
                    (across * cos + down * sin) * half * s,
                    (down * cos - across * sin) * half * s,
                );
                let at = sight.place + Vec3::Y * 0.5;
                glow.push_corners(
                    [at - a - b, at - a + b, at + a + b, at + a - b],
                    [
                        Vec2::new(0.0, 1.0),
                        Vec2::new(0.0, 0.0),
                        Vec2::new(1.0, 0.0),
                        Vec2::new(1.0, 1.0),
                    ],
                    TWIRL_COLOUR,
                );
            }
        }
        for (quads, mesh) in [dots, glow].into_iter().zip(&sight.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The hero's casts' sights.
pub struct CastSightsPlugin;

impl Plugin for CastSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (start, run)
                    .chain()
                    .in_set(Running)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
