use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use serde::{Deserialize, Serialize};

use crate::backend::{self, Backend};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The package manager pkg drives unless `--with` overrides it.
    pub backend: String,
    /// How pkg gains root: sudo, doas, run0, or "none" to never escalate.
    #[serde(default = "default_escalator")]
    pub escalator: String,
}

fn default_escalator() -> String {
    "sudo".to_string()
}

impl Config {
    pub fn resolve_backend(&self) -> Result<Backend> {
        Backend::parse(&self.backend).with_context(|| {
            format!(
                "unknown backend {:?} in {}; run `pkg config init` to pick again",
                self.backend,
                path().display()
            )
        })
    }

    /// The escalation command, or None when escalation is disabled.
    pub fn escalator(&self) -> Option<&str> {
        match self.escalator.as_str() {
            "none" | "" => None,
            other => Some(other),
        }
    }
}

pub fn path() -> PathBuf {
    if let Some(dir) = std::env::var_os("PKG_CONFIG_FILE") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("pkg").join("pkg.conf")
}

pub fn load() -> Result<Option<Config>> {
    let p = path();
    if !p.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    let cfg: Config =
        toml::from_str(&raw).with_context(|| format!("parsing {} (expected TOML)", p.display()))?;
    Ok(Some(cfg))
}

pub fn save(cfg: &Config) -> Result<()> {
    let p = path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let body = format!(
        "# pkg configuration -- https://github.com/danilolucasmd/pkg\n\
         # backend:   which package manager pkg drives\n\
         # escalator: sudo | doas | run0 | none\n\
         {}",
        toml::to_string_pretty(cfg)?
    );
    std::fs::write(&p, body).with_context(|| format!("writing {}", p.display()))?;
    Ok(())
}

/// Return the saved config, running onboarding the first time instead of failing.
pub fn load_or_onboard() -> Result<Config> {
    match load()? {
        Some(cfg) => Ok(cfg),
        None => onboard(),
    }
}

/// Ask which detected backend to use, then persist the answer.
pub fn onboard() -> Result<Config> {
    let found = backend::detect();

    if found.is_empty() {
        bail!(
            "no supported package manager found on PATH.\n\
             pkg supports: {}",
            backend::ALL
                .iter()
                .map(|b| b.id())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let chosen = if found.len() == 1 {
        let only = found[0];
        eprintln!("pkg: {only} is the only package manager found -- using it.");
        only
    } else {
        if !std::io::stdin().is_terminal() {
            bail!(
                "pkg is not configured and cannot prompt (no terminal).\n\
                 Detected: {}\n\
                 Write {} with, for example:\n\n\
                 \x20   backend = \"{}\"\n\
                 \x20   escalator = \"sudo\"\n",
                found.iter().map(|b| b.id()).collect::<Vec<_>>().join(", "),
                path().display(),
                found[0].id()
            );
        }

        let labels: Vec<String> = found
            .iter()
            .map(|b| format!("{:<8} {}", b.id(), b.describe()))
            .collect();

        let index = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Which package manager should pkg use?")
            .items(&labels)
            .default(0)
            .interact()
            .context("backend selection cancelled")?;

        found[index]
    };

    let cfg = Config {
        backend: chosen.id().to_string(),
        escalator: default_escalator(),
    };
    save(&cfg)?;
    eprintln!("pkg: saved backend = {chosen} to {}", path().display());
    crate::install_hint::warn_if_unreachable();
    Ok(cfg)
}
