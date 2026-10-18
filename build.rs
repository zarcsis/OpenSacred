//! The build script of opensacred-convert and opensacred-game (their `package.build`): the version
//! of the binaries, passed to the code as `OPENSACRED_VERSION`. The version lives only in Git: the
//! Release workflow tags every version `vX.Y.Z` (docs/03 §3.6), and `Cargo.toml` has none.
//! `git describe` gives `0.4.2` on a tag and `0.4.2-3-gabc1234` three commits after it; a clone
//! without the tags gives the commit alone, a source tree without Git "unknown".

use std::path::Path;
use std::process::Command;

fn main() {
    let version = git(&["describe", "--always", "--match", "v[0-9]*"])
        .map(|v| v.strip_prefix('v').unwrap_or(&v).to_owned())
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo::rustc-env=OPENSACRED_VERSION={version}");
    // The version changes with HEAD: its reflog takes every commit, checkout and reset, whether the
    // branch's ref is loose or packed. And with the tags, loose or packed. A worktree keeps HEAD
    // apart from the shared refs; `git rev-parse --git-path` finds each.
    for name in ["HEAD", "logs/HEAD", "refs/tags", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name])
            && Path::new(&path).exists()
        {
            println!("cargo::rerun-if-changed={path}");
        }
    }
}

/// The output of a Git command, trimmed; `None` when Git is missing or fails.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}
