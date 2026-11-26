//! The game's texts set without their fonts' pair kerning, as the original's pen draws them
//! (#3837; docs/re/interface.md, "The fonts' pen and the kerning (`cFontTTF2`)"): its font's
//! setup fetches the face's kerning pairs (`GetKerningPairsW`, `0x64DCB0`) and drops them, so that
//! its pen (`0x64E9C0`) and its measures step each glyph by its own advance. Bevy's shaper applies
//! a face's legacy `kern` table by default, whose pairs about an apostrophe in AntiquaSSK draw the
//! letter after it over the one before (h + `'` −262, `'` + s −115 units of 1,000); every
//! [`TextFont`] takes the feature off as it is inserted.

use bevy::prelude::*;
use bevy::text::{FontFeatureTag, FontFeatures};

/// The OpenType feature of the pair kerning, which the shaper takes from a face's `kern` table
/// where it has no GPOS.
const KERNING: FontFeatureTag = FontFeatureTag::new(b"kern");

/// The features of every text of the game: the pair kerning off.
fn unkerned() -> FontFeatures {
    FontFeatures::builder().set(KERNING, 0).build()
}

/// Takes the pair kerning off each text's font as it is inserted.
fn unkern(inserted: On<Insert<TextFont>>, mut fonts: Query<&mut TextFont>) {
    let Ok(mut font) = fonts.get_mut(inserted.entity) else {
        return;
    };
    let features = unkerned();
    if font.font_features != features {
        font.font_features = features;
    }
}

/// The texts without the fonts' pair kerning.
pub struct KerningPlugin;

impl Plugin for KerningPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(unkern);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_text_takes_the_kerning_off_however_it_is_made() {
        let mut app = App::new();
        app.add_plugins(KerningPlugin);
        let spawned = app.world_mut().spawn(TextFont::default()).id();
        let inserted = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(inserted).insert(TextFont {
            font_size: bevy::text::FontSize::Px(20.0),
            ..default()
        });
        app.update();
        for entity in [spawned, inserted] {
            let font = app.world().get::<TextFont>(entity).unwrap();
            assert_eq!(font.font_features, unkerned());
            assert_ne!(font.font_features, FontFeatures::default());
        }
    }
}
