//! The sights of the casts in their casters' hands (791, `FX_CAST_SPELL`, #1092;
//! docs/re/effects.md, "The casts' sights"): as a spell's clip starts (`0x51E110`), the hero's or
//! a creature's, or the Daemon's arts' (`0x51B46E`), or as a script's queued `Partikel` of `n0` −2
//! comes to its turn (`0x59CE78`, #1890), each of its sights follows its bone, its particles left
//! where they are born along the hand's way, or a glow swelling and shrinking at it, or the twirls
//! on the ground, all untinted by the night
//! (`opensacred_rules::particles::{CastSight, cast_sights, art_sights, ritual_sights}`).

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

/// A cast's sight as a save keeps it (#3814, the epic #3753): its caster by its name, its bone,
/// its system, and its bone's place and the place before; a load makes it again there, going on
/// as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedCastSight")]
pub struct SavedLook {
    #[serde(with = "crate::names::optional")]
    caster: Option<Entity>,
    bone: CastBone,
    look: CastSight,
    place: Vec3,
    before: Option<Vec3>,
}

impl bevy::ecs::entity::MapEntities for SavedLook {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.caster = self.caster.map(|caster| mapper.get_mapped(caster));
    }
}

/// The casts' sights in `world` as a save keeps them (#3814).
pub(crate) fn saved(world: &mut World) -> Vec<SavedLook> {
    let mut sights = world.query::<&Sight>();
    sights
        .iter(world)
        .map(|sight| SavedLook {
            caster: sight.caster,
            bone: sight.bone,
            look: sight.look.clone(),
            place: sight.place,
            before: sight.before,
        })
        .collect()
}

/// Makes a loaded save's casts' sights again (#3814).
pub(crate) fn again(world: &mut World, sights: Vec<SavedLook>) {
    if sights.is_empty() {
        return;
    }
    if let Err(e) = world.run_system_cached_with(sights_again, sights) {
        warn!("cast sights: the saved sights are not made: {e}");
    }
}

/// Makes the saved sights `sights` again ([`again`]); one whose caster the load finds not ends
/// where it was.
fn sights_again(
    In(sights): In<Vec<SavedLook>>,
    mut commands: Commands,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for mut saved in sights {
        let caster = crate::names::found(saved.caster);
        if saved.caster.is_some() && caster.is_none() {
            saved.look.end();
        }
        make(
            &mut commands,
            (caster, saved.place, saved.before),
            (saved.bone, saved.look),
            (
                data.as_deref(),
                &assets,
                &mut looks,
                &mut effects,
                &mut meshes,
            ),
        );
    }
}

/// A creature's cast as its clip begins (`creature_casts`, `0x51E110`): its caster, its spell
/// and its clip's rate.
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct CreatureCast {
    pub(crate) caster: Entity,
    pub(crate) spell: u16,
    pub(crate) rate: f32,
}

/// The sights that a scripts' queued `Partikel` of `n0` −2 makes at its creature at its turn, of
/// its `n1` (`opensacred_rules::particles::ritual_sights`, #1890), at the creature's casts' rate.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScriptSights {
    pub(crate) caster: Entity,
    pub(crate) code: i32,
}

/// What a sight's making takes: the assets and the materials, the effects' and the meshes'.
type Makers<'a> = (
    Option<&'a Data>,
    &'a AssetServer,
    &'a mut Looks,
    &'a mut Assets<Effect>,
    &'a mut Assets<Mesh>,
);

/// Makes the sight `look` at `caster`'s `bone`, at its place and the place before, on the layer
/// that its caster stands on as it is made (its `buildings::Layered`), with which it shows and
/// hides as the buildings' states show their floors, as the original makes its FX object at the
/// caster's place, floor byte and all, and chains it there (`0x51E2BE`, `0x5FBA40`;
/// docs/re/effects.md, "Where in the frame", #3843).
fn make(
    commands: &mut Commands,
    (caster, place, before): (Option<Entity>, Vec3, Option<Vec3>),
    (bone, look): (CastBone, CastSight),
    (data, assets, looks, effects, meshes): Makers,
) {
    let style = look.style;
    let (blend, _) = particles::cast_blend(style);
    let dots = looks.get(
        (particles::cast_texture(style), blend == Blend::Whole),
        data,
        assets,
        effects,
    );
    let glow = if style == 22 {
        looks.get((TWIRL, true), data, assets, effects)
    } else {
        look.glow_style().and_then(|glow| {
            let name = if glow == 7 { GLOWS[0] } else { GLOWS[1] };
            looks.get((name, true), data, assets, effects)
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
    debug!("cast sights: {caster:?} makes style {style} at {bone:?}");
    if let Some(caster) = caster {
        let made = sight.id();
        crate::buildings::take_layer(&mut sight.commands(), made, vec![caster]);
    }
    sight.insert(Sight {
        caster,
        bone,
        look,
        place,
        before,
        meshes: parts,
    });
}

/// Makes the sights of each art or spell the hero starts, a spell's cast's on its clip's rate,
/// the casts' speed over 100, the Daemon's arts' at 1; and of each creature's cast, every one of
/// the style 13, the blue glow, that the handler gives a creature's sight (its `+0xA9` 2: its
/// event's `+0x28` not 1.0, `0x5A3A4A`; **seen** 2026-11-05 at Anducar's demon's fire bolt); and
/// those of the scripts' queued `Partikel`s of `n0` −2 in their own styles, as their event's
/// `+0x28` is 1.0 (the step `0x59CE78`, #1890), at the creature's casts' rate.
fn start(
    mut commands: Commands,
    started: Query<(Entity, &Making, Option<&Combat>, &GlobalTransform), Added<Playing>>,
    (mut casts, mut scripted): (MessageReader<CreatureCast>, MessageReader<ScriptSights>),
    (places, combats): (Query<&GlobalTransform>, Query<&Combat>),
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
            make(
                &mut commands,
                (Some(caster), place.translation(), None),
                (bone, CastSight::new(style, rate)),
                (
                    data.as_deref(),
                    &assets,
                    &mut looks,
                    &mut effects,
                    &mut meshes,
                ),
            );
        }
    }
    for cast in casts.read() {
        let Ok(place) = places.get(cast.caster) else {
            continue;
        };
        for (bone, _) in particles::cast_sights(cast.spell, false) {
            make(
                &mut commands,
                (Some(cast.caster), place.translation(), None),
                (
                    bone,
                    CastSight::new(particles::CREATURES_CAST_STYLE, cast.rate),
                ),
                (
                    data.as_deref(),
                    &assets,
                    &mut looks,
                    &mut effects,
                    &mut meshes,
                ),
            );
        }
    }
    for sights in scripted.read() {
        let Ok(place) = places.get(sights.caster) else {
            continue;
        };
        // The creature's `+0x3E6` over 100, its clips' rate.
        let rate = combats
            .get(sights.caster)
            .map_or(1.2, |combat| combat.cast / 100.0);
        for (bone, style) in particles::ritual_sights(sights.code) {
            make(
                &mut commands,
                (Some(sights.caster), place.translation(), None),
                (bone, CastSight::new(style, rate)),
                (
                    data.as_deref(),
                    &assets,
                    &mut looks,
                    &mut effects,
                    &mut meshes,
                ),
            );
        }
    }
}

/// Runs each sight a frame on at its bone (`0x776BB0`, `0x776CA0`): its particles born along the
/// bone's way and left there, its glow at the bone, its twirls on the ground, all just behind its
/// caster's model (`fx::Orders`, #2188); it ends with its last particle past its time.
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
    orders: crate::fx::Orders,
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
        // Just behind its caster's model, as the original draws the system before the creature
        // it follows (#2188), else at its place once its caster is gone.
        let order = sight
            .caster
            .map_or_else(|| orders.at(sight.place), |caster| orders.borne_by(caster));
        for (mut quads, mesh) in [dots, glow].into_iter().zip(&sight.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, Vec3::ZERO);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The casts' sights.
pub struct CastSightsPlugin;

impl Plugin for CastSightsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CreatureCast>()
            .add_message::<ScriptSights>()
            .init_resource::<Dice>()
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
