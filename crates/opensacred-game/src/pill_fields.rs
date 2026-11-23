//! The scripts' field of pills and ghosts (`FX_TRAP02_PARENT`, 891, with 892 and 893; #2916;
//! docs/re/effects.md, "The scripts' traps"): the field that a sector's `Enter` makes (`npcs`), in
//! the cave of 38,98, lays a pill on every point of a grid two cells apart that may be walked on
//! and up to twelve ghosts among them, which walk the grid's lines
//! (`opensacred_rules::script_traps::Field`); the hero coming near a pill takes it for its
//! experience, and near a ghost takes a strike from nobody of 200 of each kind, at most once a
//! second for the whole field (`fight::Burn`). Each pill is a yellow glow over the ground, added
//! whole, each ghost a blue face over the plane of the height 0, added by its alpha, both beating
//! once a second, untinted by the night, each at its place in the original's order of drawing
//! (`fx::Orders::at`). As the original's, the field is made anew, its pills all back, at each
//! coming of its sector; a save keeps each field laid as it stands, which its load puts back in
//! place of a field made anew, its eaten pills eaten still ([`Again`], #3450).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::script_traps::{
    self, GHOST_COLOUR, GHOST_DAMAGES, GHOST_TEXTURE, GLOW_LIFT, PILL_COLOUR, PILL_EXPERIENCE,
    PILL_TEXTURE, Piece,
};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::paths::cell_of;
use crate::terrain::Heights;

/// A field that a script made, at its record's point on the plane of the height 0, which lays
/// its pieces once its cell's sector is in ([`lay`]).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Field;

/// A field laid: its rules, the heights of its pills' ground, by its pieces' indices, and the
/// meshes of its pills and of its ghosts.
#[derive(Component)]
struct Laid {
    field: script_traps::Field,
    grounds: Vec<f32>,
    meshes: [Handle<Mesh>; 2],
}

/// The pills' and the ghosts' materials, made as the first field lays; none where a texture is
/// missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    pills: Option<Handle<Effect>>,
    ghosts: Option<Handle<Effect>>,
}

/// The fields that a loaded save kept (#3450; docs/re/effects.md, "The scripts' traps", "The
/// save"), each put back onto the field of its point, laid already ([`put_back`]) or as it lays
/// ([`lay`]), in place of one laid anew: the original's load brings the field's objects back and
/// runs no `Enter` of the sectors it kept marked.
#[derive(Resource, Default)]
pub(crate) struct Again(pub Vec<script_traps::Field>);

impl Again {
    /// The kept field of the point `origin`, taken.
    fn take(&mut self, origin: [f32; 2]) -> Option<script_traps::Field> {
        let index = self.0.iter().position(|field| same(field.origin, origin))?;
        Some(self.0.swap_remove(index))
    }

    /// Whether it keeps a field of the point `origin`.
    fn holds(&self, origin: [f32; 2]) -> bool {
        self.0.iter().any(|field| same(field.origin, origin))
    }
}

/// Whether two fields' points are one: a record's point placed again.
fn same(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1.0 && (a[1] - b[1]).abs() < 1.0
}

/// The fields as they stand, to save (#3450): those that a load kept and that wait for their
/// sectors, then each laid field of another point.
pub(crate) fn saved(world: &mut World) -> Vec<script_traps::Field> {
    let mut fields = world
        .get_resource::<Again>()
        .map(|again| again.0.clone())
        .unwrap_or_default();
    let kept = Again(fields.clone());
    let mut laid = world.query::<&Laid>();
    fields.extend(
        laid.iter(world)
            .filter(|laid| !kept.holds(laid.field.origin))
            .map(|laid| laid.field.clone()),
    );
    fields
}

/// Forgets the fields that a loaded save kept, as another game loads or the menu comes.
fn forget(mut again: ResMut<Again>) {
    again.0.clear();
}

/// The fields' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x37B))
    }
}

/// Whether the world's point `at` lies on a cell of the ground that may be walked on (`0x636890`).
fn walkable(heights: &Heights, at: [f32; 2]) -> bool {
    heights.walkable_at(cell_of(at[0], at[1]), None)
}

/// The heights of the ground under each pill of `field`, by its pieces' indices.
fn grounds(field: &script_traps::Field, heights: &Heights) -> Vec<f32> {
    field
        .pieces
        .iter()
        .map(|piece| match piece {
            Piece::Pill { at, .. } => heights.standing(at[0], at[1], None).unwrap_or(0.0),
            Piece::Ghost(_) => 0.0,
        })
        .collect()
}

/// Puts each field that a loaded save kept back onto the field of its point laid already, as the
/// load's sectors laid it anew before the save's world was put back (#3450).
fn put_back(mut again: ResMut<Again>, mut fields: Query<&mut Laid>, heights: Option<Res<Heights>>) {
    if again.0.is_empty() {
        return;
    }
    let Some(heights) = heights else {
        return;
    };
    for mut laid in &mut fields {
        if let Some(field) = again.take(laid.field.origin) {
            info!(
                "pill fields: the field at {:?} as the save kept it",
                cell_of(field.origin[0], field.origin[1])
            );
            laid.grounds = grounds(&field, &heights);
            laid.field = field;
        }
    }
}

/// Lays each new field whose sector is in (`0x5A40E2`): its pills and ghosts, or those that a
/// loaded save kept of its point ([`Again`]), and the meshes that draw them.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn lay(
    mut commands: Commands,
    fields: Query<(Entity, &Transform), (With<Field>, Without<Laid>)>,
    (heights, data, server): (Option<Res<Heights>>, Option<Res<Data>>, Res<AssetServer>),
    (mut looks, mut effects, mut meshes, mut dice, mut again): (
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
        ResMut<Again>,
    ),
) {
    let Some(heights) = heights else {
        return;
    };
    for (entity, at) in &fields {
        let origin = [at.translation.x, at.translation.z];
        if !heights.holds(cell_of(origin[0], origin[1])) {
            continue;
        }
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            looks.made = true;
            let assets = &data.0.assets;
            let image = |name: &str| {
                let image = particle_texture(assets, &server, name);
                if image.is_none() {
                    warn!("pill fields: no {name}; its pieces are not drawn");
                }
                image
            };
            looks.pills = image(PILL_TEXTURE).map(|image| effects.add(whole_material(image)));
            looks.ghosts = image(GHOST_TEXTURE)
                .map(|image| effects.add(particle_material(image, AlphaMode::Add)));
        }
        let field = again.take(origin).unwrap_or_else(|| {
            script_traps::Field::new(origin, |at| walkable(&heights, at), &mut dice.0)
        });
        let grounds = grounds(&field, &heights);
        info!(
            "pill fields: a field of {} pieces at {:?}",
            field.pieces.len(),
            cell_of(origin[0], origin[1])
        );
        let made = [looks.pills.clone(), looks.ghosts.clone()].map(|material| {
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
            mesh
        });
        commands.entity(entity).insert(Laid {
            field,
            grounds,
            meshes: made,
        });
    }
}

/// Runs each field a frame on (`0x5A6680`, `0x5AA7E0`): its ghosts' walk, the hero's touch of a
/// pill, which gives its experience, or of a ghost, which strikes him; and draws its pieces
/// (`0x7A21A0`, `0x7A24A0`).
fn run(
    time: Res<Time>,
    mut fields: Query<(&mut Laid, &Transform)>,
    heroes: Query<(Entity, &Transform), With<crate::hero::Body>>,
    (heights, mut dice, orders): (Option<Res<Heights>>, ResMut<Dice>, Orders),
    (mut experience, rules, mut burns, mut meshes): (
        ResMut<crate::fight::Experience>,
        Res<crate::fight::Rules>,
        MessageWriter<crate::fight::Burn>,
        ResMut<Assets<Mesh>>,
    ),
) {
    let Some(heights) = heights else {
        return;
    };
    let dt = time.delta_secs().min(0.1);
    let hero = heroes.iter().next();
    let s = iso::MODEL_SCALE;
    let lift = world([0.0, 0.0, GLOW_LIFT]);
    for (mut laid, at) in &mut fields {
        let laid = &mut *laid;
        laid.field
            .update(dt, |at| walkable(&heights, at), &mut dice.0);
        if let Some((hero, place)) = hero {
            let touched = laid.field.touch([place.translation.x, place.translation.z]);
            if touched.pill.is_some() {
                let grew = experience.0.add(PILL_EXPERIENCE, &rules.0);
                debug!("pill fields: the hero takes a pill, {PILL_EXPERIENCE} experience");
                if grew {
                    info!("pill fields: the hero reaches level {}", experience.0.level);
                }
            }
            if touched.struck {
                debug!("pill fields: a ghost strikes the hero");
                burns.write(crate::fight::Burn {
                    target: hero,
                    damages: GHOST_DAMAGES,
                });
            }
        }
        let origin = at.translation;
        let [pill_half, ghost_half] = laid.field.halves();
        let mut pills = Quads::default();
        let mut ghosts = Quads::default();
        for (piece, &ground) in laid.field.pieces.iter().zip(&laid.grounds) {
            let (quads, place, half, colour) = match piece {
                Piece::Pill { taken: true, .. } => continue,
                Piece::Pill { at, .. } => (
                    &mut pills,
                    Vec3::new(at[0], ground, at[1]),
                    pill_half,
                    PILL_COLOUR,
                ),
                Piece::Ghost(ghost) => (
                    &mut ghosts,
                    Vec3::new(ghost.at[0], 0.0, ghost.at[1]),
                    ghost_half,
                    GHOST_COLOUR,
                ),
            };
            let mut glow = Quads::default();
            glow.push_facing(place + lift - origin, half * s, [colour; 4]);
            glow.in_order(orders.at(place), origin);
            quads.append(glow);
        }
        for (quads, mesh) in [pills, ghosts].into_iter().zip(&laid.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

pub struct PillFieldsPlugin;

impl Plugin for PillFieldsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .init_resource::<Again>()
            .add_systems(OnEnter(AppState::Loading), forget)
            .add_systems(OnEnter(AppState::Menu), forget)
            .add_systems(
                Update,
                (put_back, lay, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A field of the point `origin` laid on ground walked on everywhere, its first pill taken
    /// where `eaten`.
    fn field(origin: [f32; 2], eaten: bool) -> script_traps::Field {
        let mut field = script_traps::Field::new(origin, |_| true, &mut crate::seed::dice(1));
        if eaten && let Some(Piece::Pill { taken, .. }) = field.pieces.first_mut() {
            *taken = true;
        }
        field
    }

    #[test]
    fn a_kept_field_is_taken_by_its_point_alone() {
        let mut again = Again(vec![
            field([100.0, 200.0], true),
            field([900.0, 50.0], false),
        ]);
        assert!(again.take([5_000.0, 5_000.0]).is_none());
        assert!(again.holds([100.5, 199.5]));
        let taken = again.take([100.5, 199.5]).unwrap();
        assert_eq!(taken, field([100.0, 200.0], true));
        assert!(!again.holds([100.0, 200.0]));
        assert!(again.take([100.0, 200.0]).is_none());
        assert_eq!(again.0, vec![field([900.0, 50.0], false)]);
        // Apart by a whole unit along either axis, a point is another.
        assert!(!again.holds([901.0, 50.0]));
        assert!(!again.holds([900.0, 51.0]));
    }

    #[test]
    fn a_save_keeps_the_fields_laid_and_those_a_load_kept() {
        let mut world = World::new();
        for (origin, eaten) in [([100.0, 200.0], false), ([900.0, 50.0], true)] {
            world.spawn(Laid {
                field: field(origin, eaten),
                grounds: Vec::new(),
                meshes: [Handle::default(), Handle::default()],
            });
        }
        assert_eq!(saved(&mut world).len(), 2);
        // A field that a load kept and has not put back yet stands for its point's laid field.
        world.insert_resource(Again(vec![field([100.0, 200.0], true)]));
        let fields = saved(&mut world);
        assert_eq!(fields.len(), 2);
        assert!(fields.contains(&field([100.0, 200.0], true)));
        assert!(fields.contains(&field([900.0, 50.0], true)));
        assert!(!fields.contains(&field([100.0, 200.0], false)));
    }
}
