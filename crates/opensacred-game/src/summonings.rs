//! A big demon's summoning (#2006; docs/re/effects.md, "The big demon's summoning"): the six
//! objects of `FX_BIG_DEMON_SUMMON` (918) that a script's `CreateObj` makes about one place
//! (`npcs`) draw, as `cParticleSystem_bigdemonsummon`'s draw (`0x7A9060`), a burning pentagram in its
//! circle of glows of `PARTICLE_GLOW03` on the ground, added whole, for 8 s
//! (`opensacred_rules::particles::DemonSummon`); untinted by the night, the circle's light of
//! 600. Each growls as it is set up, `BOSS_SAKKARA_DEMON_GROWL01` or `02` at even odds.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, DemonSummon};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// An object of a big demon's summoning, of its variant (the record's `0x03`), which `npcs` makes
/// for a script's `CreateObj` of 918.
#[derive(Component)]
pub(crate) struct Summon(pub u8);

/// A summoning's look: its state, its ramp, its dice and its glows' mesh.
#[derive(Component)]
struct Shown {
    summon: DemonSummon,
    ramp: particles::Ramp,
    dice: Rng,
    mesh: Handle<Mesh>,
}

/// A summoning's look as a save keeps it (#3812, the epic #3753): its object by its name, its
/// state and its dice; a load gives it to its object made again, going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedDemonSummon")]
pub struct SavedLook {
    #[serde(with = "crate::names::required")]
    object: Entity,
    shown: (DemonSummon, Rng),
}

impl bevy::ecs::entity::MapEntities for SavedLook {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.object = mapper.get_mapped(self.object);
    }
}

/// The summonings' looks in `world` as a save keeps them (#3812).
pub(crate) fn saved(world: &mut World) -> Vec<SavedLook> {
    let mut shown = world.query::<(Entity, &Shown)>();
    shown
        .iter(world)
        .map(|(object, shown)| SavedLook {
            object,
            shown: (shown.summon.clone(), shown.dice.clone()),
        })
        .collect()
}

/// Gives a loaded save's summonings' looks to their objects made again (#3812): one that shows
/// already takes its state, another takes it as it shows; one whose object the load finds not is
/// left out.
pub(crate) fn again(world: &mut World, looks: Vec<SavedLook>) {
    for saved in looks {
        let Some(mut object) = crate::names::found(Some(saved.object))
            .and_then(|object| world.get_entity_mut(object).ok())
        else {
            continue;
        };
        match object.get_mut::<Shown>() {
            Some(mut shown) => {
                let (summon, dice) = saved.shown;
                shown.ramp = summon.ramp();
                shown.summon = summon;
                shown.dice = dice;
            }
            None => {
                object.insert(crate::fx::Resumes(saved.shown));
            }
        }
    }
}

/// The summonings' material, made as the first shows; none where the texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<Effect>>>);

/// The seed of the summonings' `rand()` (**estimate**: any).
const SEED: usize = 0x396;

/// Gives each new summoning's object its look, its light and its growl (`0x7A92E0`, `0x5A48F0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    summons: Query<
        (
            Entity,
            &Summon,
            Option<&crate::fx::Resumes<(DemonSummon, Rng)>>,
        ),
        Added<Summon>,
    >,
    mut sounds: MessageWriter<crate::sfx::Play>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Material>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, &Summon(variant), resumed) in &summons {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data.as_ref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::SUMMON_TEXTURE)
                });
                if image.is_none() {
                    warn!("summonings: no texture {}", particles::SUMMON_TEXTURE);
                }
                image.map(|image| materials.add(whole_material(image)))
            })
            .clone();
        let mesh = meshes.add(empty());
        if let Some(material) = material {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
                ChildOf(entity),
            ));
        }
        // A loaded summoning goes on as it went, its growl not made again (#3812): no load
        // sends an FX object's start.
        let (summon, dice) = match resumed {
            Some(crate::fx::Resumes(kept)) => kept.clone(),
            None => {
                let index = usize::try_from(entity.index_u32()).unwrap_or(0);
                let mut dice = crate::seed::dice(SEED ^ index);
                let growl = particles::SUMMON_GROWLS[usize::from(dice.below(2) == 1)];
                sounds.write(crate::sfx::Play {
                    sound: crate::sfx::Of::Id(growl),
                    at: Some(entity),
                    rate: (0.5, crate::sfx::Roll::None),
                });
                (DemonSummon::new(variant), dice)
            }
        };
        let mut made = commands.entity(entity);
        made.remove::<crate::fx::Resumes<(DemonSummon, Rng)>>();
        if summon.light() > 0.0 {
            made.insert(daynight::Light(summon.light()));
        }
        made.insert(Shown {
            ramp: summon.ramp(),
            summon,
            dice,
            mesh,
        });
    }
}

/// Runs each summoning a frame on (`0x7A8C10`, `0x7A9060`); once its glows are gone it draws
/// nothing more and puts its light out.
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: crate::fx::Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look, at) in &mut shown {
        let look = &mut *look;
        look.summon.update(dt, &mut look.dice);
        let mut glows = Quads::default();
        for glow in &look.summon.system.particles {
            glows.push(
                world(glow.place),
                glow.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                look.ramp.at(glow.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            // In list 0, under every sprite and model, the creature it summons among them, as
            // the original's walk appends the FX objects of the flag `0x4` (#3285).
            glows.at_order(orders.in_list_0(at.translation()), at.translation());
            glows.into_mesh(&mut mesh);
        }
        if look.summon.done() {
            commands.entity(entity).remove::<(Shown, daynight::Light)>();
        }
    }
}

/// The big demons' summonings.
pub struct SummoningsPlugin;

impl Plugin for SummoningsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Material>().add_systems(
            Update,
            (show, burn)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
