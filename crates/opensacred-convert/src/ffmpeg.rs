//! FFmpeg as an external program (`FFMPEG`, else `ffmpeg` from `PATH`; docs/03): it decodes the
//! VTex textures and encodes the samples, so no codec code enters the converter.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// The program to run.
fn program() -> String {
    std::env::var("FFMPEG").unwrap_or_else(|_| "ffmpeg".to_owned())
}

/// Runs FFmpeg with `args` (after `-v error`), `input` on its stdin, and returns its stdout.
/// `what` names the input in errors.
pub fn pipe(args: &[&str], input: &[u8], what: &str) -> Result<Vec<u8>> {
    let program = program();
    let mut child = Command::new(&program)
        .args(["-v", "error"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot run `{program}` for {what} (set FFMPEG)"))?;
    let mut stdin = child.stdin.take().context("FFmpeg's stdin")?;
    // Feed the input from another thread, so that a full stdout pipe cannot stall the write.
    let (output, written) = std::thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(input));
        (child.wait_with_output(), writer.join())
    });
    let output = output.context("waiting for FFmpeg")?;
    if !output.status.success() {
        bail!(
            "FFmpeg failed on {what}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    match written {
        Ok(result) => result.context("feeding FFmpeg")?,
        Err(_) => bail!("feeding FFmpeg: the writer thread panicked"),
    }
    Ok(output.stdout)
}

/// Runs FFmpeg with `args` (after `-v error`), which name its input and output files.
pub fn run(args: &[&std::ffi::OsStr], what: &str) -> Result<()> {
    let program = program();
    let output = Command::new(&program)
        .args(["-v", "error"])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("cannot run `{program}` for {what} (set FFMPEG)"))?;
    if !output.status.success() {
        bail!(
            "FFmpeg failed on {what}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}
