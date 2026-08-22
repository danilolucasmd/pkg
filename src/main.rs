mod backend;
mod config;
mod exec;

#[cfg(test)]
mod tests;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};

use backend::{Action, Backend};
use config::Config;

/// Common flags shared by every package-facing verb.
///
/// They must be given before the package names: everything from the first
/// operand onwards is forwarded to the backend untouched.
#[derive(Debug, Args)]
struct Common {
    /// Use this backend for this command only
    #[arg(short = 'w', long = "with", value_name = "BACKEND")]
    with: Option<String>,

    /// Answer yes to the backend's prompts
    #[arg(short = 'y', long = "yes")]
    yes: bool,
}

#[derive(Debug, Parser)]
#[command(
    name = "pkg",
    version,
    about = "One set of verbs for every Linux package manager",
    after_help = "Package names and any flags pkg does not recognise are passed \
                  straight to the backend, so pkg-level flags such as -y must come \
                  before the package names."
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Install one or more packages
    #[command(visible_alias = "i", visible_alias = "add")]
    Install {
        #[command(flatten)]
        common: Common,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        packages: Vec<String>,
    },

    /// Search the backend's repositories
    #[command(visible_alias = "s")]
    Search {
        #[command(flatten)]
        common: Common,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        query: Vec<String>,
    },

    /// Update packages, or the whole system when given no package names
    #[command(visible_alias = "u")]
    Update {
        #[command(flatten)]
        common: Common,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        packages: Vec<String>,
    },

    /// Remove one or more packages
    #[command(visible_alias = "rm", visible_alias = "uninstall")]
    Remove {
        #[command(flatten)]
        common: Common,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        packages: Vec<String>,
    },

    /// List installed packages
    #[command(visible_alias = "ls")]
    List {
        #[command(flatten)]
        common: Common,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra: Vec<String>,
    },

    /// Show details about a package
    Info {
        #[command(flatten)]
        common: Common,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        packages: Vec<String>,
    },

    /// Inspect or change pkg's own settings
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCmd {
    /// Print the current configuration
    Show,
    /// Print the path to the configuration file
    Path,
    /// Re-run the backend picker and overwrite the configuration
    Init,
    /// Print one setting (backend, escalator)
    Get { key: String },
    /// Change one setting (backend, escalator)
    Set { key: String, value: String },
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("pkg: {err:#}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();

    let (action, common, rest) = match cli.command {
        Cmd::Config { action } => return config_cmd(action),
        Cmd::Install { common, packages } => (Action::Install, common, packages),
        Cmd::Search { common, query } => (Action::Search, common, query),
        Cmd::Update { common, packages } => {
            let action = if packages.iter().any(|a| !a.starts_with('-')) {
                Action::Update
            } else {
                Action::UpdateAll
            };
            (action, common, packages)
        }
        Cmd::Remove { common, packages } => (Action::Remove, common, packages),
        Cmd::List { common, extra } => (Action::List, common, extra),
        Cmd::Info { common, packages } => (Action::Info, common, packages),
    };

    let cfg = config::load_or_onboard()?;

    let chosen = match &common.with {
        Some(name) => Backend::parse(name).with_context(|| {
            format!(
                "unknown backend {name:?}; supported: {}",
                backend::ALL
                    .iter()
                    .map(|b| b.id())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?,
        None => cfg.resolve_backend()?,
    };

    if !chosen.is_installed() {
        bail!(
            "{chosen} is not installed on this system.\n\
             Detected package managers: {}",
            detected_list()
        );
    }

    let steps = backend::plan(chosen, action, &rest, common.yes);
    exec::run(&steps, &cfg)
}

fn detected_list() -> String {
    let found = backend::detect();
    if found.is_empty() {
        "none".to_string()
    } else {
        found.iter().map(|b| b.id()).collect::<Vec<_>>().join(", ")
    }
}

fn config_cmd(action: ConfigCmd) -> Result<i32> {
    match action {
        ConfigCmd::Path => println!("{}", config::path().display()),

        ConfigCmd::Init => {
            config::onboard()?;
        }

        ConfigCmd::Show => match config::load()? {
            Some(cfg) => {
                println!("backend   = {}", cfg.backend);
                println!("escalator = {}", cfg.escalator);
                println!("path      = {}", config::path().display());
                println!("detected  = {}", detected_list());
            }
            None => {
                println!("pkg is not configured yet; run `pkg config init`.");
                println!("detected  = {}", detected_list());
            }
        },

        ConfigCmd::Get { key } => {
            let cfg =
                config::load()?.context("pkg is not configured yet; run `pkg config init`")?;
            match key.as_str() {
                "backend" => println!("{}", cfg.backend),
                "escalator" => println!("{}", cfg.escalator),
                other => bail!("unknown key {other:?}; valid keys: backend, escalator"),
            }
        }

        ConfigCmd::Set { key, value } => {
            let mut cfg = config::load()?.unwrap_or(Config {
                backend: String::new(),
                escalator: "sudo".to_string(),
            });
            match key.as_str() {
                "backend" => {
                    let b = Backend::parse(&value).with_context(|| {
                        format!(
                            "unknown backend {value:?}; supported: {}",
                            backend::ALL
                                .iter()
                                .map(|b| b.id())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })?;
                    if !b.is_installed() {
                        eprintln!("pkg: warning: {b} is not on PATH right now.");
                    }
                    cfg.backend = b.id().to_string();
                }
                "escalator" => {
                    if !matches!(value.as_str(), "sudo" | "doas" | "run0" | "none") {
                        eprintln!("pkg: warning: unusual escalator {value:?}.");
                    }
                    cfg.escalator = value.clone();
                }
                other => bail!("unknown key {other:?}; valid keys: backend, escalator"),
            }
            if cfg.backend.is_empty() {
                bail!("no backend set yet; run `pkg config init` first");
            }
            config::save(&cfg)?;
            println!("{key} = {value}");
        }
    }

    Ok(0)
}
