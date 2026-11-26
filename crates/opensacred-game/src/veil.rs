//! The veil over the effects that the night leaves untinted (#3851; docs/decisions/drawing.md,
//! "Effects dimmed by the faded sprites before them"): the original blends each sprite drawn
//! after an effect over it by its alpha, so that a faded crown or roof before a spell lets it
//! through dimmed by its own fade (docs/re/drawing.md, "Depth", "The models"). The game draws
//! those effects by a camera of their own after the world and its night (`iso::Untinted`), tested
//! against the depth that the sprites' opaque texels alone write, so that a faded texel neither
//! hid nor dimmed them: an effect behind a fading wall showed whole inside the fade's zone and was
//! cut at its edge. A camera of the veil ([`VeilCamera`]), a child of the world's, draws before
//! it the fading sprites' texels that a point fades and the second pass draws translucent
//! (`statics::Pass::Veil`, `sprites.wesl`'s `fragment_veil`) into [`VEIL`]: at each texel the
//! nearest such texel's place toward the camera (`veil.wesl`) and the product of what each lets
//! through, `1 − alpha`. What the untinted camera draws blended, the particles of
//! [`fx::Effect`](crate::fx::Effect) and the models' passes of
//! [`ModelMaterial`](crate::models::ModelMaterial) (#3856), takes that product of its colour where
//! it lies behind that place (`standard.wesl`'s `VEILED`, [`veiled`]).

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{CameraUpdateSystems, ClearColorConfig, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::pbr::MeshPipelineKey;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::iso;

/// The render layer of the veil, which [`VeilCamera`] alone draws.
pub const VEIL_LAYER: usize = 23;

/// The veil's image, of a fixed size whatever the window's, which every effect's material holds
/// (`fx::Particles`) and the effects read by their share of the screen: never replaced, so that
/// no material has to be given it again.
pub const VEIL: Handle<Image> = bevy::asset::uuid_handle!("6b1f3c9e-58a4-4d2b-9e7a-3c41f0d2a7b5");

/// The veil's size in texels: a texel over two pixels at 3,840 × 2,160, where the fade's alpha
/// changes over tens of them.
const SIZE: UVec2 = UVec2::new(1920, 1080);

/// The place toward the camera, in world units from the point it looks at, of a texel that no
/// faded sprite covers: further than anything drawn, a half float's power of two (`0xF800`).
const NOTHING: f32 = -32768.0;

// Further than the far end of the camera's depth, half its range from the point it looks at.
const _: () = assert!(NOTHING < -iso::DISTANCE);

/// The half floats of a texel of the veil that no faded sprite covers: [`NOTHING`] in each of its
/// colours and all let through in its alpha (`0x3C00`, 1).
const CLEAR_TEXEL: [u16; 4] = [0xF800, 0xF800, 0xF800, 0x3C00];

/// Whether a material's pipeline of `key` reads the veil (`standard.wesl`'s `VEILED`): the
/// untinted camera's, the one with no depth prepass, and blended, as only a blended texel lets a
/// faded sprite before it through; an opaque one is drawn whole. The cameras of pictures of their
/// own (`iso::Apart`) have none either: the veil is clear outside the game, where alone they draw
/// such materials (the figures of the choice of the hero).
pub fn veiled(key: MeshPipelineKey) -> bool {
    let blend = key.intersection(MeshPipelineKey::BLEND_RESERVED_BITS);
    !key.contains(MeshPipelineKey::DEPTH_PREPASS)
        && [
            MeshPipelineKey::BLEND_PREMULTIPLIED_ALPHA,
            MeshPipelineKey::BLEND_MULTIPLY,
            MeshPipelineKey::BLEND_ALPHA,
        ]
        .contains(&blend)
}

/// The camera of the veil: a child of the world's, drawing the layer [`VEIL_LAYER`] into [`VEIL`]
/// before it, over the world camera's area of the world.
#[derive(Component)]
pub struct VeilCamera;

/// The veil's image, every texel clear ([`CLEAR_TEXEL`]), for the effects drawn before its
/// camera's first frame.
fn image() -> Image {
    let texels = usize::try_from(SIZE.x * SIZE.y).unwrap_or(0);
    let data: Vec<u8> = CLEAR_TEXEL
        .iter()
        .flat_map(|half| half.to_le_bytes())
        .collect::<Vec<u8>>()
        .repeat(texels);
    let mut image = Image::new(
        Extent3d {
            width: SIZE.x,
            height: SIZE.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image
}

/// Puts the veil's image at [`VEIL`].
fn setup(mut images: ResMut<Assets<Image>>) {
    if let Err(error) = images.insert(&VEIL, image()) {
        warn!("veil: its image: {error}");
    }
}

/// Gives the world's camera, as it comes, the [`VeilCamera`] as a child.
fn attach(mut commands: Commands, cameras: Query<Entity, iso::Eye<Added<Camera3d>>>) {
    for camera in &cameras {
        commands.entity(camera).with_child((
            VeilCamera,
            Camera3d::default(),
            Camera {
                order: -2,
                clear_color: ClearColorConfig::Custom(Color::linear_rgba(
                    NOTHING, NOTHING, NOTHING, 1.0,
                )),
                ..default()
            },
            RenderTarget::from(VEIL),
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            // It draws no light: one cluster for the lights it does not use.
            bevy::light::cluster::ClusterConfig::Single,
            RenderLayers::layer(VEIL_LAYER),
            // Out of the world's cameras' queries (`iso::Eye`), its projection kept by `follow`.
            iso::Apart,
        ));
    }
}

/// Keeps the [`VeilCamera`] at its world camera's area of the world, which its image of a fixed
/// size takes whatever the window's shape: the world camera's projection at its zoom, its area
/// without the zoom taken whole ([`ScalingMode::Fixed`]); that area, the window's shape's, is the
/// last frame's, as the world camera's is made after this.
fn follow(
    eyes: Query<&Projection, iso::Eye<Without<VeilCamera>>>,
    mut cameras: Query<(&ChildOf, &mut Projection), With<VeilCamera>>,
) {
    for (parent, mut projection) in &mut cameras {
        let Ok(Projection::Orthographic(eye)) = eyes.get(parent.parent()) else {
            continue;
        };
        let scale = eye.scale.max(f32::EPSILON);
        *projection = Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: eye.area.width() / scale,
                height: eye.area.height() / scale,
            },
            ..eye.clone()
        });
    }
}

/// Draws the veil while some sprite fades about a point (`statics::Fade::fades`), and one frame
/// after, which clears it: some half a millisecond a frame where nothing fades.
fn activate(
    fade: Res<crate::statics::Fade>,
    mut cameras: Query<&mut Camera, With<VeilCamera>>,
    mut faded: Local<bool>,
) {
    let fades = fade.fades();
    if fades != *faded {
        debug!("veil: {}", if fades { "drawn" } else { "cleared" });
    }
    for mut camera in &mut cameras {
        camera.is_active = fades || *faded;
    }
    *faded = fades;
}

/// The veil over the effects.
pub struct VeilPlugin;

impl Plugin for VeilPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, attach)
            .add_systems(Update, activate.after(crate::frame::Step::Models))
            .add_systems(PostUpdate, follow.before(CameraUpdateSystems));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_untinted_cameras_blended_materials_alone_read_the_veil() {
        let blended = [
            MeshPipelineKey::BLEND_PREMULTIPLIED_ALPHA,
            MeshPipelineKey::BLEND_MULTIPLY,
            MeshPipelineKey::BLEND_ALPHA,
        ];
        for blend in blended {
            assert!(veiled(blend));
            // The world's camera, with its depth prepass, draws them among the sprites.
            assert!(!veiled(blend | MeshPipelineKey::DEPTH_PREPASS));
        }
        assert!(!veiled(MeshPipelineKey::BLEND_OPAQUE));
        assert!(!veiled(MeshPipelineKey::BLEND_ALPHA_TO_COVERAGE));
    }

    #[test]
    fn the_veils_clear_texel_holds_nothing_and_lets_all_through() {
        // The half floats' bits: −2^15 and 1.
        let half = |bits: u16| {
            let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
            let exponent = i32::from((bits >> 10) & 0x1F) - 15;
            let mantissa = 1.0 + f32::from(bits & 0x3FF) / 1024.0;
            sign * mantissa * 2f32.powi(exponent)
        };
        assert_eq!(half(CLEAR_TEXEL[0]), NOTHING);
        assert_eq!(half(CLEAR_TEXEL[3]), 1.0);
        let image = image();
        assert_eq!(
            image.data.as_ref().map(Vec::len),
            Some(usize::try_from(SIZE.x * SIZE.y * 8).unwrap())
        );
        assert_eq!(
            image.data.as_ref().map(|data| data[..8].to_vec()),
            Some(vec![0x00, 0xF8, 0x00, 0xF8, 0x00, 0xF8, 0x00, 0x3C])
        );
    }
}
