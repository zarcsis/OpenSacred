//! FFmpeg as an external program (the one beside the converter, else `FFMPEG`, else `ffmpeg` from
//! `PATH`; docs/03): it decodes the
//! VTex textures and encodes the samples, so no codec code enters the converter.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, anyhow, bail};

/// The program to run: the FFmpeg beside the converter, as a release carries it (#1497), else
/// `FFMPEG`, else `ffmpeg` from `PATH`.
fn program() -> String {
    let beside = std::env::current_exe().ok().and_then(|exe| {
        let path = exe.with_file_name(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
        path.is_file().then(|| path.to_string_lossy().into_owned())
    });
    beside.unwrap_or_else(|| std::env::var("FFMPEG").unwrap_or_else(|_| "ffmpeg".to_owned()))
}

/// FFmpeg's command, without a console window of its own on Windows: run from a converter that
/// has no console, as the packaged game starts it, it would otherwise open one each time (#1495).
#[cfg(windows)]
fn command(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    /// `CREATE_NO_WINDOW` of `CreateProcess`.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn command(program: &str) -> Command {
    Command::new(program)
}

/// Runs FFmpeg with `args` (after `-v error`), `input` on its stdin, and returns its stdout.
/// `what` names the input in errors.
pub fn pipe(args: &[&str], input: &[u8], what: &str) -> Result<Vec<u8>> {
    let program = program();
    let mut child = command(&program)
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
        return Err(failure(what, &output));
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
    let output = command(&program)
        .args(["-v", "error"])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("cannot run `{program}` for {what} (set FFMPEG)"))?;
    if !output.status.success() {
        return Err(failure(what, &output));
    }
    Ok(())
}

/// The error of an FFmpeg run that did not succeed: its exit status, which alone tells a crash
/// or a process that could not start (an NTSTATUS such as `0xc0000142` on Windows), and its
/// message.
fn failure(what: &str, output: &Output) -> anyhow::Error {
    let status = output.status;
    match String::from_utf8_lossy(&output.stderr).trim() {
        "" => anyhow!("FFmpeg failed on {what} ({status}) without a message"),
        message => anyhow!("FFmpeg failed on {what} ({status}): {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs FFmpeg"]
    fn a_failure_names_the_input_the_exit_status_and_the_message() {
        let args = ["-f", "bink", "-i", "pipe:0", "-f", "rawvideo", "pipe:1"];
        let err = pipe(&args, b"not a Bink file", "a test input")
            .unwrap_err()
            .to_string();
        let (status, message) = err
            .strip_prefix("FFmpeg failed on a test input (exit ")
            .and_then(|rest| rest.split_once("): "))
            .unwrap_or_else(|| panic!("{err}"));
        assert!(!status.is_empty() && !message.is_empty(), "{err}");
    }
}
