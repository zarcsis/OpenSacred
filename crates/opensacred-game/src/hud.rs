//! A minimal HUD: the hero's portrait in the upper left, the health bar under it, and whether the
//! hero walks or runs. The original lays its interface out in code from the `gui_main_*` atlases
//! (docs/02 §11); until that layout is recovered, the HUD is laid out here with whole textures as
//! skins, on the original's 768-pixel-high screen scaled to the window's height, as the world is.
//! The cursor is the original's `MOUSE_MAIN.TGA`, scaled the same way in whole steps.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::FontSize;
use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage};

use crate::hero::{Gait, Hero};

/// The height of the original's screen, which the HUD is laid out on.
const SCREEN_HEIGHT: f32 = 768.0;
const MARGIN: f32 = 8.0;
const PORTRAIT: f32 = 96.0;
const BAR_HEIGHT: f32 = 8.0;
/// The original's interface font (`opensacred-convert ui`).
const FONT: &str = "ui/fonts/antqs__.ttf";
const FONT_SIZE: f32 = 18.0;
const HEALTH: Color = Color::srgb(0.70, 0.06, 0.04);
const BAR_BACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.6);
const GOLD: Color = Color::srgb(0.86, 0.74, 0.45);

/// The original's cursor: the arrow in the top left 32 × 32 of its 64 × 64 image, its tip at the
/// corner.
const CURSOR: &str = "mouse_main";
const CURSOR_SIDE: usize = 32;

/// The textures of the HUD, relative to the asset directory.
#[derive(Resource)]
pub struct Hud {
    /// The hero's portrait: `gui_qbp_<class>` of `texture.pak`.
    pub portrait: String,
    /// The cursor's image file, if the converted textures have it.
    pub cursor: Option<PathBuf>,
}

/// The converted texture named `name` (`textures/texture/<record>-<name>.png`).
pub fn find_texture(assets: &Path, name: &str) -> Option<PathBuf> {
    let dir = assets.join("textures").join("texture");
    let suffix = format!("-{name}.png");
    fs::read_dir(&dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(&suffix))
        })
}

pub fn cursor_file(assets: &Path) -> Option<PathBuf> {
    find_texture(assets, CURSOR)
}

/// The top left `CURSOR_SIDE` square of an RGBA PNG, each pixel `scale` times wider and taller.
fn cursor_image(path: &Path, scale: usize) -> Result<Image> {
    let file = fs::File::open(path).map_err(|e| anyhow!("opening {}: {e}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder
        .read_info()
        .map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| anyhow!("{}: image too large", path.display()))?;
    let mut pixels = vec![0; size];
    let info = reader
        .next_frame(&mut pixels)
        .map_err(|e| anyhow!("decoding {}: {e}", path.display()))?;
    let width = usize::try_from(info.width)?;
    if info.color_type != png::ColorType::Rgba
        || width < CURSOR_SIDE
        || usize::try_from(info.height)? < CURSOR_SIDE
    {
        bail!("{}: not an RGBA image of at least 32 x 32", path.display());
    }
    let side = CURSOR_SIDE * scale;
    let mut out = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        for x in 0..side {
            let i = ((y / scale) * width + x / scale) * 4;
            out.extend_from_slice(&pixels[i..i + 4]);
        }
    }
    let side = u32::try_from(side)?;
    Ok(Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        out,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ))
}

/// Puts the original's cursor on the window, scaled in whole steps with the window's height.
pub fn cursor(
    mut commands: Commands,
    hud: Res<Hud>,
    window: Single<(Entity, &Window)>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(path) = &hud.cursor else {
        return;
    };
    let (entity, window) = *window;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a window's height over 768 is a small positive number"
    )]
    let scale = (window.height() / SCREEN_HEIGHT).round().max(1.0) as usize;
    match cursor_image(path, scale) {
        Ok(image) => {
            commands
                .entity(entity)
                .insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
                    handle: images.add(image),
                    hotspot: (0, 0),
                    ..default()
                })));
        }
        Err(e) => bevy::log::warn!("cursor: {e:#}"),
    }
}

/// The label of the hero's gait.
#[derive(Component)]
pub struct GaitLabel;

fn gait_text(gait: Gait) -> &'static str {
    match gait {
        Gait::Walk => "Walk",
        Gait::Run => "Run",
    }
}

pub fn setup(mut commands: Commands, hud: Res<Hud>, hero: Res<Hero>, assets: Res<AssetServer>) {
    let font: Handle<Font> = assets.load(FONT);
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(MARGIN),
            top: Val::Px(MARGIN),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|column| {
            column.spawn((
                ImageNode::new(assets.load(hud.portrait.clone())),
                Node {
                    width: Val::Px(PORTRAIT),
                    height: Val::Px(PORTRAIT),
                    ..default()
                },
            ));
            // Full until the rules give the hero hit points (M3).
            column
                .spawn((
                    Node {
                        width: Val::Px(PORTRAIT),
                        height: Val::Px(BAR_HEIGHT),
                        padding: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(BAR_BACK),
                ))
                .with_child((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(HEALTH),
                ));
            column.spawn((
                Text::new(gait_text(hero.gait)),
                TextFont {
                    font: font.into(),
                    font_size: FontSize::Px(FONT_SIZE),
                    ..default()
                },
                TextColor(GOLD),
                GaitLabel,
            ));
        });
}

/// Scales the HUD with the window's height.
pub fn scale(window: Single<&Window>, mut ui_scale: ResMut<UiScale>) {
    let scale = window.height() / SCREEN_HEIGHT;
    if ui_scale.0 != scale {
        ui_scale.0 = scale;
    }
}

/// Shows whether the hero walks or runs.
pub fn gait(hero: Res<Hero>, mut labels: Query<&mut Text, With<GaitLabel>>) {
    if !hero.is_changed() {
        return;
    }
    for mut label in &mut labels {
        let text = gait_text(hero.gait);
        if label.0 != text {
            label.0 = text.into();
        }
    }
}

/// The portrait, the health bar, the gait and the cursor.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup, cursor))
            .add_systems(Update, (scale, gait));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_is_the_top_left_square_scaled_in_whole_steps() {
        let path =
            std::env::temp_dir().join(format!("opensacred-game-cursor-{}.png", std::process::id()));
        let mut rgba = vec![0u8; 64 * 64 * 4];
        // An opaque red pixel at (1, 0) and one outside the square at (40, 40).
        rgba[4..8].copy_from_slice(&[255, 0, 0, 255]);
        let far = (40 * 64 + 40) * 4;
        rgba[far..far + 4].copy_from_slice(&[0, 255, 0, 255]);
        let mut encoder = png::Encoder::new(fs::File::create(&path).unwrap(), 64, 64);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
        let image = cursor_image(&path, 2).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!((image.width(), image.height()), (64, 64));
        let data = image.data.unwrap();
        let at = |x: usize, y: usize| &data[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(at(2, 0), [255, 0, 0, 255]);
        assert_eq!(at(3, 1), [255, 0, 0, 255]);
        assert_eq!(at(0, 0), [0, 0, 0, 0]);
        assert!(data.chunks(4).all(|p| p[1] == 0));
    }
}
