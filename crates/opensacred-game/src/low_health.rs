//! The hero's low health on the portraits' column (#1463; docs/re/interface.md, "The portraits'
//! column", `cUI_Mercenary`): the health's red ring glows while the health is below a quarter of
//! its most (`0x6D73D0`, `0x732220`), and a red alarm's ring spreads from the portrait once as the
//! health falls below the settings' warning level (`0x6D70F0`, `0x75FC70`).

use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
    ShaderType,
};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::UiMaterialKey;

use crate::bar::{self, ColumnRoot, HealthRed, UiImages};
use crate::fight::Health;
use crate::game::AppState;
use crate::hero::Body;
use crate::hud;
use crate::settings::Settings;

const GLOW_SHADER: &str = "embedded://opensacred_game/glow.wesl";
const SONAR_SHADER: &str = "embedded://opensacred_game/sonar.wesl";

/// The alarm's texture (`FX_SONAREFFECT.TGA`).
pub(crate) const SONAR: &str = "fx_sonareffect";

/// The glow's reach about the ring in the original's pixels, the same as the shader's.
const GLOW_REACH: f32 = 6.0;
/// The glow's alpha of each copy at its brightest, as a byte (`0x732220`: ⌊12.75 a⌋).
const GLOW_BYTE: f32 = 12.75;
/// The glow's pulse: a = (sin 4T + 1) / 2, T the real time in seconds.
const GLOW_PULSE: f32 = 4.0;

/// The alarm's centre in the column, (975, 70) of the original's screen; its first radius, its
/// width, its growth a second and its last radius, and its tint (`0xFF0000`).
const SONAR_CENTRE: Vec2 = Vec2::new(975.0 - 932.0, 70.0);
const SONAR_FIRST: f32 = 10.0;
const SONAR_WIDTH: f32 = 40.0;
const SONAR_GROWTH: f32 = 600.0;
const SONAR_LAST: f32 = 500.0;
const SONAR_TINT: Vec3 = Vec3::new(1.0, 0.0, 0.0);

/// What the glow's shader takes: the part of the texture drawn (its corner and size in texels)
/// and the alpha of each copy.
#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub(crate) struct GlowPart {
    part: Vec4,
    alpha: f32,
}

/// The glow of the red ring's part drawn: the part again at the offsets −6 to +6 by 2 on both
/// axes, 49 times, its colours added onto the screen (`DESTBLEND` `ONE`) at the alpha of each
/// copy.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(crate) struct Glow {
    #[uniform(0)]
    glow: GlowPart,
    #[texture(1)]
    #[sampler(2)]
    picture: Handle<Image>,
}

impl UiMaterial for Glow {
    fn fragment_shader() -> ShaderRef {
        GLOW_SHADER.into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        // The light added, the screen's alpha kept.
        let target = descriptor
            .fragment
            .as_mut()
            .and_then(|fragment| fragment.targets.first_mut())
            .and_then(Option::as_mut);
        if let Some(target) = target {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            });
        }
    }
}

/// What the alarm's shader takes: the ring's inner and outer radii and its colour with its alpha.
#[derive(ShaderType, Debug, Clone, Copy, Default)]
pub(crate) struct Ring {
    inner: f32,
    outer: f32,
    colour: Vec4,
}

/// The alarm's ring: the texture's row laid across it, its left at the outer edge and its right
/// at the inner, tinted and blended by its alpha.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(crate) struct Sonar {
    #[uniform(0)]
    ring: Ring,
    #[texture(1)]
    #[sampler(2)]
    texture: Handle<Image>,
}

impl UiMaterial for Sonar {
    fn fragment_shader() -> ShaderRef {
        SONAR_SHADER.into()
    }
}

/// The glow's node, a child of the red ring's.
#[derive(Component)]
struct GlowNode;

/// The alarm's node in the column: its radius while it spreads, and whether the hero's health
/// was below the warning level at the frame before (`+0x938`; below at first, so that a game
/// started with little health raises none).
#[derive(Component)]
struct SonarNode {
    radius: Option<f32>,
    below: bool,
}

/// The glow under the red ring's part and the alarm over the column.
fn spawn(
    mut commands: Commands,
    images: Res<UiImages>,
    assets: Res<AssetServer>,
    mut glows: ResMut<Assets<Glow>>,
    mut sonars: ResMut<Assets<Sonar>>,
    reds: Query<Entity, With<HealthRed>>,
    columns: Query<Entity, With<ColumnRoot>>,
) {
    let (Ok(red), Ok(column)) = (reds.single(), columns.single()) else {
        return;
    };
    let glow = glows.add(Glow {
        glow: GlowPart::default(),
        picture: Handle::default(),
    });
    commands.entity(red).with_child((
        GlowNode,
        MaterialNode(glow),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(-GLOW_REACH),
            top: Val::Px(-GLOW_REACH),
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
    let Some(file) = images.file(SONAR) else {
        return;
    };
    let sonar = sonars.add(Sonar {
        ring: Ring::default(),
        texture: hud::image(&assets, file.to_owned()),
    });
    commands.entity(column).with_child((
        SonarNode {
            radius: None,
            below: true,
        },
        MaterialNode(sonar),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
}

/// The hero's health over its most, and whether it is below `percent` of it, as the original
/// compares its float share (`+0x4D8` / `+0x4D4`, held at 0).
fn below(health: &Health, percent: u32) -> bool {
    u64::from(health.current) * 100 < u64::from(percent) * u64::from(health.most)
}

/// The alpha of each of the glow's copies at the real time `seconds`: ⌊12.75 a⌋/255 of the
/// pulse a = (sin 4T + 1) / 2.
fn glow_alpha(seconds: f32) -> f32 {
    let pulse = ((GLOW_PULSE * seconds).sin() + 1.0) * 0.5;
    (GLOW_BYTE * pulse).floor() / 255.0
}

/// The alarm's alpha at the radius `radius`: ⌊(1 − r/500)·255⌋/255.
fn sonar_alpha(radius: f32) -> f32 {
    ((1.0 - radius / SONAR_LAST) * 255.0).floor() / 255.0
}

/// The red ring's glow while the hero's health is below a quarter of its most, of the part of
/// the ring drawn, 6 pixels about it.
fn glow(
    time: Res<Time<Real>>,
    heroes: Query<&Health, With<Body>>,
    reds: Query<&ImageNode, With<HealthRed>>,
    mut nodes: Query<(&MaterialNode<Glow>, &mut Node, &mut Visibility), With<GlowNode>>,
    mut glows: ResMut<Assets<Glow>>,
) {
    let (Ok(red), Ok((material, mut node, mut visibility))) = (reds.single(), nodes.single_mut())
    else {
        return;
    };
    let low = heroes.iter().next().is_some_and(|health| below(health, 25));
    let alpha = glow_alpha(time.elapsed_secs());
    let part = red.rect.filter(|rect| rect.height() > 0.0);
    let (Some(part), true) = (part, low && alpha > 0.0) else {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    };
    visibility.set_if_neq(Visibility::Inherited);
    node.width = Val::Px(part.width() + 2.0 * GLOW_REACH);
    node.height = Val::Px(part.height() + 2.0 * GLOW_REACH);
    if let Some(mut glow) = glows.get_mut(&material.0) {
        glow.glow = GlowPart {
            part: Vec4::new(part.min.x, part.min.y, part.width(), part.height()),
            alpha,
        };
        if glow.picture != red.image {
            glow.picture = red.image.clone();
        }
    }
}

/// The alarm: as the hero's health falls below the warning level from at or above it, a ring
/// from the radius 10 outwards, 40 wide, growing 600 pixels a second and fading as it grows,
/// until its radius passes 500.
fn alarm(
    time: Res<Time>,
    settings: Option<Res<Settings>>,
    heroes: Query<&Health, With<Body>>,
    mut nodes: Query<(
        &mut SonarNode,
        &MaterialNode<Sonar>,
        &mut Node,
        &mut Visibility,
    )>,
    mut sonars: ResMut<Assets<Sonar>>,
) {
    let Ok((mut sonar, material, mut node, mut visibility)) = nodes.single_mut() else {
        return;
    };
    if let Some(health) = heroes.iter().next() {
        let level = settings.map_or(25, |settings| settings.interface.warning_level);
        let now = below(health, u32::from(level));
        if now && !sonar.below {
            sonar.radius = Some(SONAR_FIRST);
        }
        sonar.below = now;
    }
    let Some(radius) = sonar.radius else {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    };
    visibility.set_if_neq(Visibility::Inherited);
    let outer = radius + SONAR_WIDTH;
    node.left = Val::Px(SONAR_CENTRE.x - outer);
    node.top = Val::Px(SONAR_CENTRE.y - outer);
    node.width = Val::Px(2.0 * outer);
    node.height = Val::Px(2.0 * outer);
    if let Some(mut material) = sonars.get_mut(&material.0) {
        material.ring = Ring {
            inner: radius,
            outer,
            colour: SONAR_TINT.extend(sonar_alpha(radius)),
        };
    }
    let next = radius + SONAR_GROWTH * time.delta_secs();
    sonar.radius = (next <= SONAR_LAST).then_some(next);
}

/// The low health's glow and alarm.
pub struct LowHealthPlugin;

impl Plugin for LowHealthPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "glow.wesl");
        embedded_asset!(app, "sonar.wesl");
        app.add_plugins((
            UiMaterialPlugin::<Glow>::default(),
            UiMaterialPlugin::<Sonar>::default(),
        ))
        .add_systems(OnEnter(AppState::InGame), spawn.after(bar::spawn))
        .add_systems(
            Update,
            (glow, alarm)
                .after(bar::fill)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn health(current: u32, most: u32) -> Health {
        Health {
            current,
            most,
            level: 1,
        }
    }

    #[test]
    fn the_health_is_low_below_its_share_of_the_most() {
        assert!(below(&health(24, 100), 25));
        assert!(!below(&health(25, 100), 25));
        assert!(below(&health(34, 137), 25));
        assert!(!below(&health(35, 137), 25));
        // No most: the original's share is not a number, below nothing.
        assert!(!below(&health(0, 0), 25));
    }

    #[test]
    fn the_glow_pulses_in_steps_of_a_byte() {
        // a = 1 at sin 4T = 1: ⌊12.75⌋ = 12.
        let top = std::f32::consts::FRAC_PI_8;
        assert!((glow_alpha(top) - 12.0 / 255.0).abs() < 1e-6);
        // a = 0.5 at T = 0: ⌊6.375⌋ = 6.
        assert!((glow_alpha(0.0) - 6.0 / 255.0).abs() < 1e-6);
        // a = 0 at sin 4T = −1.
        assert!(glow_alpha(3.0 * top).abs() < 1e-6);
    }

    #[test]
    fn the_alarm_fades_as_it_spreads() {
        assert!((sonar_alpha(10.0) - 249.0 / 255.0).abs() < 1e-6);
        assert!((sonar_alpha(250.0) - 127.0 / 255.0).abs() < 1e-6);
        assert!(sonar_alpha(500.0).abs() < 1e-6);
    }
}
