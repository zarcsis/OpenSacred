//! The look of the Daemon's forms (1014, `FX_DEMONFORM`, #1092; docs/re/effects.md, "The Daemon's
//! forms"): while a form of 1097 to 1100 holds on him (`making`), three comets in the form's
//! colour whirl about his head and chest, ribbons of `FX_DEMONFORM_DEMON` led by flares, and a
//! crest of seven ray bursts fans over his head, growing over 2 s and pulsing, all added whole and
//! untinted by the night; its light circles with the first comet (`fx_lights`). It goes at once
//! as his form ends (`opensacred_rules::particles::DemonForm`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, DEMON_FORM_SQUARES, DemonForm};

use crate::bolt_looks::world;
use crate::effects::Effects;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// The bones the look follows (`0x5A56D2`, the bone 20): his head, else his neck.
const HEAD: [&str; 2] = ["Bip01 Head", "Bip01 Neck"];

/// The Daemon's cast as the look starts (`0x7B27D0`).
const CAST: u32 = 2122;

/// A form's look to make at its bearer: the art and its seconds (`making`, `0x51A47B`).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Form {
    pub(crate) bearer: Entity,
    pub(crate) art: u16,
    pub(crate) seconds: f32,
}

/// The looks' materials: the crest's three ray bursts, the ribbons' and the flares'.
#[derive(Resource, Default)]
struct Looks(Option<Materials>);

/// The materials of a look's parts.
type Materials = (
    [Option<Handle<Effect>>; 3],
    Option<Handle<Effect>>,
    Option<Handle<Effect>>,
);

/// A form's look: its bearer and art, its system, its crest's seven meshes, its ribbons' and its
/// flares' meshes, and its light.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    art: u16,
    look: DemonForm,
    squares: Vec<Handle<Mesh>>,
    ribbons: Handle<Mesh>,
    flares: Handle<Mesh>,
    light: Entity,
}

/// Gives each new form's look its parts (`0x7B1D50`, `0x7B27D0`): its crest's squares, each of
/// a ray burst at random, its ribbons, its flares, its light and its cast's sound; a look of the
/// bearer's other form goes.
fn show(
    mut commands: Commands,
    forms: Query<(Entity, &Form), Added<Form>>,
    shown: Query<(Entity, &Shown)>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<crate::spells::Dice>,
    ),
    (mut effects, mut meshes, mut sounds): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        MessageWriter<crate::sfx::Play>,
    ),
) {
    for (entity, form) in &forms {
        for (other, look) in &shown {
            if look.bearer == form.bearer {
                commands.entity(other).despawn();
            }
        }
        let (crests, ribbon, flare) = looks
            .0
            .get_or_insert_with(|| {
                let mut material = |name: &str| {
                    let image = data
                        .as_deref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("demon forms: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| effects.add(whole_material(image)))
                };
                (
                    particles::DEMON_FORM_CRESTS.map(&mut material),
                    material(particles::DEMON_FORM_RIBBON),
                    material(particles::DEMON_FORM_FLARE),
                )
            })
            .clone();
        let look = DemonForm::new(DemonForm::variant_of(form.art), form.seconds);
        let part = |commands: &mut Commands,
                    meshes: &mut Assets<Mesh>,
                    material: Option<Handle<Effect>>| {
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
        };
        // Each square's burst one of the three at random, fixed for its life (`rand() % 3`).
        let squares = (0..DEMON_FORM_SQUARES)
            .map(|_| {
                let pick = usize::try_from(dice.0.below(3)).unwrap_or(0);
                part(&mut commands, &mut meshes, crests[pick].clone())
            })
            .collect();
        let ribbons = part(&mut commands, &mut meshes, ribbon);
        let flares = part(&mut commands, &mut meshes, flare);
        let light = commands
            .spawn(crate::fx_lights::child(
                look.light(),
                crate::fx_lights::LOW,
                entity,
            ))
            .id();
        sounds.write(crate::sfx::Play {
            sound: crate::sfx::Of::Id(CAST),
            at: Some(form.bearer),
            rate: (0.5, crate::sfx::Roll::None),
        });
        commands.entity(entity).insert((
            Shown {
                bearer: form.bearer,
                art: form.art,
                look,
                squares,
                ribbons,
                flares,
                light,
            },
            Transform::IDENTITY,
            Visibility::default(),
        ));
    }
}

/// Runs each look a frame on at its bearer's head (`0x7B20F0`, `0x7B2230`): its comets' ribbons
/// and flares about it, its crest in the head's own plane, and its light at the first comet; it
/// goes at its seconds' end, as his form ends, or with him.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn whirl(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    mut lights: Query<&mut Transform, (With<crate::fx_lights::FxLight>, Without<Shown>)>,
    (bearers, children, bones, worn): (
        Query<Option<&Effects>>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: crate::fx::Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look, mut transform) in &mut shown {
        let look = &mut *look;
        look.look.update(dt);
        let holds = bearers
            .get(look.bearer)
            .is_ok_and(|effects| effects.is_some_and(|effects| effects.holds_any(&[look.art])));
        let head = crate::making::bone_entity(look.bearer, &HEAD, &children, (&bones, &worn))
            .and_then(|bone| bones.get(bone).ok())
            .map(|(_, at)| at.compute_transform());
        let (true, Some(head)) = (holds && !look.look.done(), head) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = head.translation;
        // Just behind his model, as the original draws the system before the creature (#2188).
        let order = orders.behind(look.bearer);
        if let Ok(mut light) = lights.get_mut(look.light) {
            light.translation = world(look.look.ring(0)[0]);
        }
        let colour = look.look.colour();
        // The ribbons, their texture's `v` from the head to the tail, and the flares at the heads.
        let mut ribbons = Quads::default();
        let mut flares = Quads::default();
        for r in 0..3 {
            let edges = look.look.ribbon(r);
            for (k, pair) in edges.windows(2).enumerate() {
                let [(a0, b0), (a1, b1)] = [pair[0], pair[1]];
                let v = |i: usize| f32::from(u8::try_from(i).unwrap_or(0)) / 8.0;
                ribbons.push_corners(
                    [world(a0), world(a1), world(b1), world(b0)],
                    [
                        Vec2::new(0.0, v(k)),
                        Vec2::new(0.0, v(k + 1)),
                        Vec2::new(1.0, v(k + 1)),
                        Vec2::new(1.0, v(k)),
                    ],
                    colour,
                );
            }
            flares.push(
                world(look.look.ring(r)[0]),
                particles::DEMON_FORM_FLARE_HALF * s,
                0.0,
                (Vec2::ZERO, 1.0),
                colour,
            );
        }
        // The crest's squares facing the viewer, fanned about its vertical, 8 over the head: the
        // original turns them in the head bone's own plane, which its frames show facing the
        // viewer from the front (**estimate**).
        let lift = Vec3::Y * particles::DEMON_FORM_CREST_HIGH * s;
        for (k, mesh) in look.squares.iter().enumerate() {
            let (half, turn, colour) = look.look.square(k);
            let mut quad = Quads::default();
            quad.push(lift, half * s, turn, (Vec2::ZERO, 1.0), colour);
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quad.in_order(order, head.translation);
                quad.into_mesh(&mut mesh);
            }
        }
        for (mut quads, mesh) in [(ribbons, &look.ribbons), (flares, &look.flares)] {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, head.translation);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Daemon's forms' looks.
pub struct DemonFormsPlugin;

impl Plugin for DemonFormsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, whirl)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
