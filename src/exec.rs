use std::os::unix::fs::MetadataExt;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::backend::Step;
use crate::config::Config;

fn is_root() -> bool {
    std::fs::metadata("/proc/self")
        .map(|m| m.uid() == 0)
        .unwrap_or(false)
}

/// Run each step in order, stopping at the first failure and returning its
/// exit code so `pkg install foo` is as scriptable as the command it wraps.
pub fn run(steps: &[Step], cfg: &Config) -> Result<i32> {
    let escalator = if is_root() { None } else { cfg.escalator() };

    for step in steps {
        let escalate = step.needs_root && escalator.is_some();
        eprintln!(
            "\x1b[2m-> {}\x1b[0m",
            step.display(if escalate { escalator } else { None })
        );

        let mut cmd = if escalate {
            let e = escalator.unwrap();
            if crate::backend::which(e).is_none() {
                bail!(
                    "escalator {e:?} is not on PATH; set another one with \
                     `pkg config set escalator <sudo|doas|run0|none>`"
                );
            }
            let mut c = Command::new(e);
            c.arg(&step.program);
            c
        } else {
            Command::new(&step.program)
        };
        cmd.args(&step.args);

        let status = cmd
            .status()
            .with_context(|| format!("running {}", step.program))?;

        if !status.success() {
            return Ok(status.code().unwrap_or(1));
        }
    }

    Ok(0)
}
