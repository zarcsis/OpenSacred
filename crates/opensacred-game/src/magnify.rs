//! How the art is filtered where it is magnified (#42): at 3840 × 2160 a texel of the original's
//! art covers 2.8 pixels at zoom 1 and up to 5.6 at zoom 2. The original draws its sprites with
//! point filtering, and its ground linearly only when zoomed out below 1/1.2 (docs/re/drawing.md,
//! "Blending"; docs/02 §5.4), at 1,024 × 768. Which way the game goes is the maintainer's
//! decision; until it is taken, `--filter` chooses, linear by default. The ground's and the
//! sprites' shaders take it as a shader definition (`magnify.wesl`); the statics' shadows stay
//! linear, as the original filters them.

use bevy::prelude::Resource;

/// The filter of the magnified art.
#[derive(clap::ValueEnum, Resource, Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub enum Filter {
    /// Linear between the four texels about a point: smooth.
    #[default]
    Linear,
    /// The nearest texel: the original's sprites, square texels, blocky when magnified.
    Point,
    /// The nearest texel, blended across the one screen pixel at the texels' edges: square
    /// texels without the blocks' shimmer as the camera moves.
    Sharp,
}

impl Filter {
    /// The shader definition of the filter (`magnify.wesl`); none for the linear one.
    pub fn shader_def(self) -> Option<&'static str> {
        match self {
            Filter::Linear => None,
            Filter::Point => Some("POINT"),
            Filter::Sharp => Some("SHARP"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_filter_but_the_linear_has_its_definition() {
        assert_eq!(
            [Filter::Linear, Filter::Point, Filter::Sharp].map(Filter::shader_def),
            [None, Some("POINT"), Some("SHARP")]
        );
        // The definitions are the ones `magnify.wesl` tests.
        let shader = include_str!("magnify.wesl");
        assert!(shader.contains("@if(POINT)") && shader.contains("@if(SHARP)"));
    }
}
