//! The game's sound (#701): Ogg Opus files, as the converter's `build` writes them, which Kira's
//! own decoders do not read. A file is decoded whole, as Kira decodes its static sounds, by
//! Symphonia with libopus registered (`symphonia-adapter-libopus`), into the sound that
//! `bevy_kira_audio` plays; the Ogg demuxer and the decoder drop the encoder's delay at the start
//! and the padding at the end, so that a sound keeps its length.

use std::io::{self, Cursor};

use bevy::asset::io::Reader;
use bevy::asset::{AssetApp, AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy_kira_audio::AudioSource;
use kira::Frame;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::formats::TrackType;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia_adapter_libopus::OpusDecoder;

/// The loader of the `.opus` files.
#[derive(Default, TypePath)]
pub struct OpusLoader;

impl AssetLoader for OpusLoader {
    type Asset = AudioSource;
    type Settings = ();
    type Error = io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _context: &mut LoadContext<'_>,
    ) -> io::Result<AudioSource> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(AudioSource {
            sound: decode(bytes)?,
        })
    }

    fn extensions(&self) -> &[&str] {
        &[opensacred_data::sound::EXTENSION]
    }
}

/// The frames of an Ogg Opus file, at its sample rate; a mono file's in both channels, as Kira
/// gives a mono file's.
pub fn decode(bytes: Vec<u8>) -> io::Result<StaticSoundData> {
    let mut codecs = CodecRegistry::new();
    codecs.register_audio_decoder::<OpusDecoder>();
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    let mut format = symphonia::default::get_probe()
        .probe(&hint, source, Default::default(), Default::default())
        .map_err(io::Error::other)?;
    let params = format
        .default_track(TrackType::Audio)
        .and_then(|track| track.codec_params.as_ref())
        .and_then(|params| params.audio())
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no audio track"))?;
    let mut decoder = codecs
        .make_audio_decoder(&params, &Default::default())
        .map_err(io::Error::other)?;
    let mut sample_rate = 48_000;
    let mut frames = Vec::new();
    let mut samples = Vec::new();
    while let Some(packet) = format.next_packet().map_err(io::Error::other)? {
        let decoded = decoder.decode(&packet).map_err(io::Error::other)?;
        sample_rate = decoded.spec().rate();
        let channels = decoded.spec().channels().count();
        samples.resize(decoded.samples_interleaved(), 0.0f32);
        decoded.copy_to_slice_interleaved(&mut samples);
        match channels {
            1 => frames.extend(samples.iter().map(|&s| Frame::from_mono(s))),
            n => frames.extend(samples.chunks_exact(n).map(|c| Frame::new(c[0], c[1]))),
        }
    }
    Ok(StaticSoundData {
        sample_rate,
        frames: frames.into(),
        settings: StaticSoundSettings::default(),
        slice: None,
    })
}

/// Registers the loader of the `.opus` files.
pub struct OpusPlugin;

impl Plugin for OpusPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(OpusLoader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `tests/sound/tone.opus`: half a second of a tone of 440 Hz, stereo, that FFmpeg's libopus
    /// encoded at 24 kbit/s.
    #[test]
    fn an_ogg_opus_file_decodes_to_its_length() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sound/tone.opus");
        let sound = decode(std::fs::read(path).unwrap()).unwrap();
        assert_eq!(sound.sample_rate, 48_000);
        assert_eq!(sound.frames.len(), 24_000);
        let peak = sound
            .frames
            .iter()
            .map(|f| f.left.abs().max(f.right.abs()))
            .fold(0.0, f32::max);
        // FFmpeg's sine is at an eighth of the full scale, made stereo 3 dB lower.
        assert!((0.08..0.1).contains(&peak), "{peak}");
        assert!(decode(b"OggS but not".to_vec()).is_err());
    }
}
