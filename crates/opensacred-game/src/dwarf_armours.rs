//! The look of the Dwarf's armour (58, 1021, `cParticleSystem_panzer`) and steel (59, 1022,
//! `cParticleSystem_stahl`; #1092; docs/re/effects.md, "The Dwarf's armour and steel"): on the
//! spell's target for its seconds, its ring of runes flat at the target's feet and 70 fainter
//! copies stacked over it, a column breathing up and down and turning, its colour swinging
//! between red and green (the steel's bluer), faded in and out over 2 s; added by their alpha,
//! untinted by the night, drawn just behind the bearer's model (`fx::Orders`, #2178); its light the
//! same colour (`fx_lights`) (`opensacred_rules::particles::DwarfArmour`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{DWARF_ARMOUR_HALF, DWARF_ARMOUR_TEXTURES, DwarfArmour};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::fx_lights::FxLight;
use crate::game::{AppState, Data};
use crate::iso;

/// How far over the ground the first ring lies, in the models' units, clear of its triangles.
const GROUND_LIFT: f32 = 0.5;

/// A look to make: its bearer, the steel's or the armour's, and its seconds.
#[derive(Component)]
struct Unmade {
    bearer: Entity,
    steel: bool,
    seconds: f32,
}

/// A look that shows: its bearer, its look, its light and its mesh.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    /// Whether the steel's, of which its material is.
    steel: bool,
    look: DwarfArmour,
    light: Entity,
    mesh: Handle<Mesh>,
}

/// A Dwarf's armour or steel as a save keeps it (#3777, the epic #3753): its bearer by its name and its look; a
/// load makes it again on its bearer, going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedArmour")]
pub struct SavedSight {
    #[serde(with = "crate::names::required")]
    bearer: Entity,
    steel: bool,
    look: DwarfArmour,
}

impl bevy::ecs::entity::MapEntities for SavedSight {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.bearer = mapper.get_mapped(self.bearer);
    }
}

/// The sights in `world` as a save keeps them (#3777).
pub(crate) fn saved(world: &mut World) -> Vec<SavedSight> {
    let mut shown = world.query::<&Shown>();
    shown
        .iter(world)
        .map(|shown| SavedSight {
            bearer: shown.bearer,
            steel: shown.steel,
            look: shown.look.clone(),
        })
        .collect()
}

/// Makes a loaded save's sights again on their bearers (#3777); one whose bearer the load finds
/// not is left out.
pub(crate) fn again(world: &mut World, sights: Vec<SavedSight>) {
    for saved in sights {
        if crate::names::found(Some(saved.bearer)).is_none() {
            continue;
        }
        world.spawn((
            Unmade {
                bearer: saved.bearer,
                steel: saved.steel,
                seconds: 0.0,
            },
            crate::fx::Resumes(saved.look),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// The looks' materials, their textures added by their alpha, made as the first asks; `None`
/// where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// Shows the armour's look, or the steel's where `steel`, on `bearer` for `seconds`, the spell's
/// value 0 at its level (`0x5286CE`, `0x5288BF`).
pub(crate) fn show(commands: &mut Commands, bearer: Entity, steel: bool, seconds: f32) {
    commands.spawn((
        Unmade {
            bearer,
            steel,
            seconds,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new look its rings' mesh, its material and its light (`0x7B7800`, `0x7B7B50`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade, Option<&crate::fx::Resumes<DwarfArmour>>)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade, resumed) in &unmade {
        let Ok(at) = places.get(unmade.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        let materials = looks
            .0
            .get_or_insert_with(|| {
                DWARF_ARMOUR_TEXTURES.map(|name| {
                    let image = data
                        .as_deref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("dwarf armours: no texture {name}; it is not drawn");
                    }
                    image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
                })
            })
            .clone();
        let look = resumed.map_or_else(
            || DwarfArmour::new(unmade.steel, unmade.seconds),
            |crate::fx::Resumes(look)| look.clone(),
        );
        let light = commands
            .spawn(crate::fx_lights::light(
                look.colour(),
                crate::fx_lights::HIGH,
                at,
                entity,
                (Some(unmade.bearer), Vec3::ZERO),
            ))
            .id();
        let mesh = meshes.add(empty());
        let mut made = commands.entity(entity);
        made.remove::<(Unmade, crate::fx::Resumes<DwarfArmour>)>()
            .insert((
                Shown {
                    bearer: unmade.bearer,
                    steel: unmade.steel,
                    look,
                    light,
                    mesh: mesh.clone(),
                },
                Transform::from_translation(at),
                Visibility::default(),
            ));
        if let Some(material) = materials[usize::from(unmade.steel)].clone() {
            made.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Runs each look a frame on (`0x7B78E0`) and draws it (`0x7B79A0`) at its bearer's feet: its
/// rings flat, each turned by its turn (`0x7631E0`), and its light's colour; it goes once it has
/// faded out, or with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut lights: Query<&mut FxLight>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    for (entity, mut sight, mut transform) in &mut shown {
        let sight = &mut *sight;
        sight.look.update(dt);
        let Ok(origin) = places.get(sight.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = origin;
        let colour = sight.look.colour();
        if let Ok(mut light) = lights.get_mut(sight.light)
            && light.colour != colour
        {
            light.colour = colour;
        }
        let (sin, cos) = sight.look.turn().sin_cos();
        let half = DWARF_ARMOUR_HALF;
        let mut quads = Quads::default();
        for (height, colour) in sight.look.rings() {
            let z = height + GROUND_LIFT;
            let corner = |x: f32, y: f32| {
                (across * (x * cos + y * sin) + depth * (-x * sin + y * cos) + Vec3::Y * z) * s
            };
            quads.push_corners(
                [
                    corner(-half, -half),
                    corner(half, -half),
                    corner(half, half),
                    corner(-half, half),
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                colour,
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&sight.mesh) {
            quads.in_order(orders.borne_by(sight.bearer), origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The Dwarf's armour's and steel's looks.
pub struct DwarfArmoursPlugin;

impl Plugin for DwarfArmoursPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
