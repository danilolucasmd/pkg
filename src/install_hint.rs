use std::path::{Path, PathBuf};

use crate::backend;

/// Where pkg lives, when typing `pkg` would *not* find it.
///
/// The check deliberately asks "does the name `pkg` resolve on PATH?" rather
/// than "is my own directory on PATH?": the binary is often installed to one
/// directory and symlinked into another that is on PATH, and `current_exe`
/// reports the resolved target, which would make that working setup look broken.
pub fn unreachable_dir() -> Option<PathBuf> {
    if backend::which(env!("CARGO_BIN_NAME")).is_some() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.to_path_buf())
}

/// The shell startup file to append to, and whether the shell needs its command
/// cache cleared afterwards.
pub(crate) fn shell_advice(shell: &str) -> (&'static str, Option<&'static str>) {
    match Path::new(shell).file_name().and_then(|s| s.to_str()) {
        // zsh caches command lookups, so a correct PATH still misses until rehash.
        Some("zsh") => ("~/.zshrc", Some("rehash")),
        Some("bash") if cfg!(target_os = "macos") => ("~/.bash_profile", Some("hash -r")),
        Some("bash") => ("~/.bashrc", Some("hash -r")),
        Some("fish") => ("~/.config/fish/config.fish", None),
        _ => ("your shell startup file", None),
    }
}

/// Print how to put pkg on PATH, if it is not already there.
pub fn warn_if_unreachable() {
    let Some(dir) = unreachable_dir() else {
        return;
    };

    let shell = std::env::var("SHELL").unwrap_or_default();
    let (rc, rehash) = shell_advice(&shell);
    let dir = dir.display();

    eprintln!();
    eprintln!("pkg: {dir} is not on your PATH, so `pkg` only works by full path.");

    if rc.ends_with("config.fish") {
        eprintln!("     Fix it with:\n");
        eprintln!("         fish_add_path {dir}");
    } else {
        eprintln!("     Fix it with:\n");
        eprintln!("         echo 'export PATH=\"{dir}:$PATH\"' >> {rc}");
        eprintln!("         source {rc}");
    }

    if let Some(cmd) = rehash {
        eprintln!("         {cmd}");
    }
    eprintln!();
}
