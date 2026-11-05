//! The looks of the Dwarf's mines and of their sign (#1092; docs/re/effects.md, "The Dwarf's
//! mines"): each mine `spells` lays lies on the ground as its model at 0.4 of its size, turned
//! at random, in its own texture, its button flashing red for 0.2 s in every 1.7 s
//! (`opensacred_rules::particles::DwarfMine`), and puffs dark smoke as it is laid, darkened by
//! the night; the sign spreads blue tongues flat on the ground about the Dwarf's feet along the
//! arc of his mines left (`opensacred_rules::particles::MineSign`), added and untinted.

use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling, RenderLayers};
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use opensacred_rules::particles::{self, DwarfMine, MineSign};

use crate::blended::Layer;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, texture_path};
use crate::game::{AppState, Data};
use crate::models::{Anchors, ModelMaterial, Pass, in_order};
use crate::npcs::Retexture;
use crate::spawning::Rng;
use crate::spells::{Mine, MineSign as Sign};
use crate::{bolt_looks, iso, models};

/// How far over the ground the sign's tongues lie, in world units.
const GROUND_LIFT: f32 = 0.5;

/// The looks' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1153))
    }
}

/// What the looks take, found at the first mine: the mine's model and texture, relative to the
/// asset directory, its flash's texture and material, the puffs' material and the tongues'.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    texture: Option<String>,
    flash: Option<Handle<Image>>,
    flash_material: Option<Handle<ModelMaterial>>,
    smoke: Option<Handle<Effect>>,
    tongues: Option<Handle<Effect>>,
}

impl Looks {
    /// Finds the looks' files at the first look asked.
    fn make(&mut self, data: Option<&Data>, assets: &AssetServer, effects: &mut Assets<Effect>) {
        if self.made {
            return;
        }
        self.made = true;
        let Some(data) = data else {
            return;
        };
        let shared = &data.0;
        self.model = crate::npcs::model_files(&shared.assets)
            .ok()
            .and_then(|files| files.get(particles::MINE_MODEL).cloned());
        if self.model.is_none() {
            warn!(
                "mines: no {}; the mines are not drawn",
                particles::MINE_MODEL
            );
        }
        self.texture = texture_path(&shared.assets, particles::MINE_TEXTURE);
        self.flash = texture_path(&shared.assets, particles::MINE_FLASH_TEXTURE)
            .map(|path| assets.load(path));
        let mut load = |name: &str, alpha: AlphaMode| {
            let image = particle_texture(&shared.assets, assets, name);
            if image.is_none() {
                warn!("mines: no texture {name}; its part is not drawn");
            }
            image.map(|image| effects.add(particle_material(image, alpha)))
        };
        self.smoke = load(particles::MINE_SMOKE_TEXTURE, AlphaMode::Blend);
        self.tongues = load(particles::MINE_SIGN_TEXTURE, AlphaMode::Add);
    }
}

/// A mine's look: its mine, its system, its model and the copies of its meshes that flash, and
/// the mesh its puffs go into.
#[derive(Component)]
struct MineLook {
    mine: Entity,
    look: DwarfMine,
    model: Option<Entity>,
    flashes: Vec<Entity>,
    smoke: Handle<Mesh>,
}

/// A sign's look on its sign: its system and the mesh its tongues go into.
#[derive(Component)]
struct SignLook {
    look: MineSign,
    mesh: Handle<Mesh>,
}

/// Gives each mine laid its look at its place: its model, turned and scaled, and its puffs.
fn lay(
    mut commands: Commands,
    mines: Query<(Entity, &Transform), Added<Mine>>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (mine, place) in &mines {
        looks.make(data.as_deref(), &assets, &mut effects);
        let look = DwarfMine::new(&mut dice.0);
        let at = place.translation;
        let model = looks.model.as_ref().map(|model| {
            let mut made = commands.spawn((
                models::scene(&assets, model),
                Transform::from_translation(at)
                    .with_rotation(Quat::from_rotation_y(look.turn))
                    .with_scale(Vec3::splat(particles::MINE_SCALE * iso::MODEL_SCALE)),
                models::Ordered::default(),
                DespawnOnExit(AppState::InGame),
            ));
            if let Some(texture) = &looks.texture {
                made.insert(Retexture::load(&assets, texture));
            }
            made.id()
        });
        let smoke = meshes.add(empty());
        let mut shown = commands.spawn((
            Transform::from_translation(at),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        // The puffs take the light at them, as the world does (the flag `0x40`).
        if let Some(material) = looks.smoke.clone() {
            shown.with_child((
                Mesh3d(smoke.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ));
        }
        shown.insert(MineLook {
            mine,
            look,
            model,
            flashes: Vec::new(),
            smoke,
        });
    }
}

/// A mesh of a model, its material converted: its mesh, its joints, its tag and its bounds.
type ModelMesh = (
    &'static Mesh3d,
    Option<&'static SkinnedMesh>,
    &'static MeshTag,
    Has<DynamicSkinnedMeshBounds>,
    Has<NoFrustumCulling>,
);

/// Gives each mine's model, once its meshes are converted (`models`), the copies that draw its
/// flash: the same mesh in the flash's texture, unlit, added whole in the objects' layer of the
/// blended things, hidden between its flashes (`0x40D310`).
fn flash_copies(
    mut commands: Commands,
    mut mines: Query<&mut MineLook>,
    (children, meshes): (
        Query<&Children>,
        Query<ModelMesh, With<MeshMaterial3d<ModelMaterial>>>,
    ),
    (mut looks, anchors, mut materials): (
        ResMut<Looks>,
        Res<Anchors>,
        ResMut<Assets<ModelMaterial>>,
    ),
) {
    for mut mine in &mut mines {
        let Some(model) = mine.model else {
            continue;
        };
        if !mine.flashes.is_empty() {
            continue;
        }
        let own: Vec<Entity> = children
            .iter_descendants(model)
            .filter(|&entity| meshes.contains(entity))
            .collect();
        if own.is_empty() {
            continue;
        }
        let Some(flash) = looks.flash.clone() else {
            mine.model = None;
            continue;
        };
        let material = looks
            .flash_material
            .get_or_insert_with(|| {
                let added = StandardMaterial {
                    base_color_texture: Some(flash),
                    unlit: true,
                    ..default()
                };
                materials.add(in_order(added, &anchors, Pass::Added(Layer::Objects)))
            })
            .clone();
        for entity in own {
            let Ok((mesh, skinned, tag, dynamic, unculled)) = meshes.get(entity) else {
                continue;
            };
            let mut copy = commands.spawn((
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(material.clone()),
                tag.clone(),
                Visibility::Hidden,
                ChildOf(entity),
            ));
            if let Some(skinned) = skinned {
                copy.insert(skinned.clone());
            }
            if dynamic {
                copy.insert(DynamicSkinnedMeshBounds);
            }
            if unculled {
                copy.insert(NoFrustumCulling);
            }
            mine.flashes.push(copy.id());
        }
    }
}

/// Runs each mine's look a frame on (`0x7BAD70`, `0x7BAF30`): its flash shown or hidden and its
/// puffs; it goes, its model with it, as its mine goes off.
fn run_mines(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut MineLook)>,
    mines: Query<(), With<Mine>>,
    mut visibility: Query<&mut Visibility>,
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = DwarfMine::ramp();
    for (entity, mut look) in &mut shown {
        if !mines.contains(look.mine) {
            if let Some(model) = look.model {
                commands.entity(model).try_despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        let MineLook {
            look: mine,
            flashes,
            smoke,
            ..
        } = &mut *look;
        mine.update(dt, &mut dice.0);
        let shows = if mine.flashes() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        for &copy in flashes.iter() {
            if let Ok(mut visible) = visibility.get_mut(copy)
                && *visible != shows
            {
                *visible = shows;
            }
        }
        let mut quads = Quads::default();
        for puff in &mine.system.particles {
            quads.push(
                bolt_looks::world(puff.place),
                puff.size * s,
                puff.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(puff.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*smoke) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Gives each new sign its look, on the sign itself, which rides with the Dwarf.
fn sign(
    mut commands: Commands,
    signs: Query<Entity, Added<Sign>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for sign in &signs {
        looks.make(data.as_deref(), &assets, &mut effects);
        let mesh = meshes.add(empty());
        let mut shown = commands.entity(sign);
        shown.insert((
            Visibility::default(),
            SignLook {
                look: MineSign::new(),
                mesh: mesh.clone(),
            },
        ));
        if let Some(material) = looks.tongues.clone() {
            shown.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Runs each sign's look a frame on (`0x7BA610`, `0x7BA830`): its tongues lie flat on the ground,
/// each standing on its foot on the circle and pointing out by its turn, the texture's foot at it.
fn run_signs(
    time: Res<Time>,
    mut signs: Query<(&Sign, &mut SignLook)>,
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let ramp = MineSign::ramp();
    let lift = Vec3::Y * GROUND_LIFT;
    for (sign, mut look) in &mut signs {
        let SignLook {
            look: tongues,
            mesh,
        } = &mut *look;
        tongues.update(dt, sign.share_left(), &mut dice.0);
        let mut quads = Quads::default();
        for tongue in &tongues.system.particles {
            let (sin, cos) = tongue.angle.sin_cos();
            let [x, y, _] = tongue.place;
            let size = tongue.size;
            let at = |across: f32, out: f32| {
                bolt_looks::world([
                    x + cos * across + sin * out,
                    y - sin * across + cos * out,
                    0.0,
                ]) + lift
            };
            quads.push_corners(
                [
                    at(-size, 0.0),
                    at(size, 0.0),
                    at(size, 2.0 * size),
                    at(-size, 2.0 * size),
                ],
                [Vec2::Y, Vec2::ONE, Vec2::X, Vec2::ZERO],
                ramp.at(tongue.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The mines' and their sign's looks.
pub struct MinesPlugin;

impl Plugin for MinesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (lay, flash_copies, run_mines, sign, run_signs)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
