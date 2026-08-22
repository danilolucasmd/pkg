use std::fmt;
use std::path::PathBuf;

/// A package manager pkg knows how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    Pacman,
    Apt,
    Dnf,
    Yay,
    Paru,
    Flatpak,
    Snap,
    Brew,
    Nix,
}

/// Detection order: native managers first, then AUR helpers, then universal ones.
pub const ALL: &[Backend] = &[
    Backend::Pacman,
    Backend::Apt,
    Backend::Dnf,
    Backend::Yay,
    Backend::Paru,
    Backend::Flatpak,
    Backend::Snap,
    Backend::Brew,
    Backend::Nix,
];

impl Backend {
    pub fn id(self) -> &'static str {
        match self {
            Backend::Pacman => "pacman",
            Backend::Apt => "apt",
            Backend::Dnf => "dnf",
            Backend::Yay => "yay",
            Backend::Paru => "paru",
            Backend::Flatpak => "flatpak",
            Backend::Snap => "snap",
            Backend::Brew => "brew",
            Backend::Nix => "nix",
        }
    }

    /// The executable pkg looks for on PATH and ultimately runs.
    pub fn binary(self) -> &'static str {
        self.id()
    }

    pub fn describe(self) -> &'static str {
        match self {
            Backend::Pacman => "Arch Linux native packages",
            Backend::Apt => "Debian / Ubuntu native packages",
            Backend::Dnf => "Fedora / RHEL native packages",
            Backend::Yay => "Arch repos + AUR (yay)",
            Backend::Paru => "Arch repos + AUR (paru)",
            Backend::Flatpak => "Flatpak sandboxed apps",
            Backend::Snap => "Snap packages",
            Backend::Brew => "Homebrew (user-level)",
            Backend::Nix => "Nix profiles (user-level)",
        }
    }

    pub fn parse(s: &str) -> Option<Backend> {
        ALL.iter().copied().find(|b| b.id() == s)
    }

    /// Whether mutating operations on this backend need privilege escalation.
    ///
    /// yay/paru call sudo themselves, flatpak authenticates through polkit, and
    /// brew/nix are user-level by design (brew actively refuses to run as root).
    pub fn needs_escalation(self) -> bool {
        matches!(
            self,
            Backend::Pacman | Backend::Apt | Backend::Dnf | Backend::Snap
        )
    }

    /// The flag that suppresses this backend's confirmation prompts, if it has one.
    fn yes_flag(self) -> Option<&'static str> {
        match self {
            Backend::Pacman | Backend::Yay | Backend::Paru => Some("--noconfirm"),
            Backend::Apt | Backend::Dnf | Backend::Flatpak => Some("-y"),
            // snap, brew and nix never prompt.
            Backend::Snap | Backend::Brew | Backend::Nix => None,
        }
    }

    pub fn is_installed(self) -> bool {
        which(self.binary()).is_some()
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// What the user asked pkg to do, before it is translated to a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Search,
    /// `pkg update` with no packages: refresh metadata and upgrade everything.
    UpdateAll,
    /// `pkg update <pkg>...`
    Update,
    Remove,
    List,
    Info,
}

impl Action {
    fn mutating(self) -> bool {
        matches!(
            self,
            Action::Install | Action::UpdateAll | Action::Update | Action::Remove
        )
    }
}

/// One concrete process invocation. A single Action can expand to several
/// (`apt update` then `apt upgrade`), run in order, stopping on the first failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub program: String,
    pub args: Vec<String>,
    pub needs_root: bool,
}

impl Step {
    fn new(backend: Backend, action: Action, args: &[&str]) -> Step {
        Step {
            program: backend.binary().to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            needs_root: action.mutating() && backend.needs_escalation(),
        }
    }

    /// The command as it would be typed in a shell, for the echo line.
    pub fn display(&self, escalator: Option<&str>) -> String {
        let mut parts = Vec::new();
        if let Some(e) = escalator {
            if self.needs_root {
                parts.push(e.to_string());
            }
        }
        parts.push(self.program.clone());
        parts.extend(self.args.iter().cloned());
        parts.join(" ")
    }
}

/// Translate a pkg action into the backend's own commands.
///
/// `rest` is everything the user typed after the verb: package names plus any
/// flags pkg did not recognise, which are forwarded to the backend verbatim.
pub fn plan(backend: Backend, action: Action, rest: &[String], yes: bool) -> Vec<Step> {
    let rest = normalize_operands(backend, action, rest);
    let mut steps = match (backend, action) {
        // ---- pacman family -------------------------------------------------
        // Arch does not support partial upgrades, so updating one package
        // upgrades the system too (-Syu <pkg>) rather than -Sy <pkg>.
        (Backend::Pacman, Action::Install) => vec![Step::new(backend, action, &["-S"])],
        (Backend::Pacman, Action::Search) => vec![Step::new(backend, action, &["-Ss"])],
        (Backend::Pacman, Action::UpdateAll) => vec![Step::new(backend, action, &["-Syu"])],
        (Backend::Pacman, Action::Update) => vec![Step::new(backend, action, &["-Syu"])],
        (Backend::Pacman, Action::Remove) => vec![Step::new(backend, action, &["-Rns"])],
        (Backend::Pacman, Action::List) => vec![Step::new(backend, action, &["-Q"])],
        (Backend::Pacman, Action::Info) => vec![Step::new(backend, action, &["-Si"])],

        (Backend::Yay, Action::Install) | (Backend::Paru, Action::Install) => {
            vec![Step::new(backend, action, &["-S"])]
        }
        (Backend::Yay, Action::Search) | (Backend::Paru, Action::Search) => {
            vec![Step::new(backend, action, &["-Ss"])]
        }
        (Backend::Yay, Action::UpdateAll) | (Backend::Paru, Action::UpdateAll) => {
            vec![Step::new(backend, action, &["-Syu"])]
        }
        (Backend::Yay, Action::Update) | (Backend::Paru, Action::Update) => {
            vec![Step::new(backend, action, &["-Syu"])]
        }
        (Backend::Yay, Action::Remove) | (Backend::Paru, Action::Remove) => {
            vec![Step::new(backend, action, &["-Rns"])]
        }
        (Backend::Yay, Action::List) | (Backend::Paru, Action::List) => {
            vec![Step::new(backend, action, &["-Q"])]
        }
        (Backend::Yay, Action::Info) | (Backend::Paru, Action::Info) => {
            vec![Step::new(backend, action, &["-Si"])]
        }

        // ---- apt -----------------------------------------------------------
        (Backend::Apt, Action::Install) => vec![Step::new(backend, action, &["install"])],
        (Backend::Apt, Action::Search) => vec![Step::new(backend, action, &["search"])],
        (Backend::Apt, Action::UpdateAll) => vec![
            Step::new(backend, action, &["update"]),
            Step::new(backend, action, &["upgrade"]),
        ],
        (Backend::Apt, Action::Update) => vec![
            Step::new(backend, action, &["update"]),
            Step::new(backend, action, &["install", "--only-upgrade"]),
        ],
        (Backend::Apt, Action::Remove) => vec![Step::new(backend, action, &["remove"])],
        (Backend::Apt, Action::List) => vec![Step::new(backend, action, &["list", "--installed"])],
        (Backend::Apt, Action::Info) => vec![Step::new(backend, action, &["show"])],

        // ---- dnf -----------------------------------------------------------
        (Backend::Dnf, Action::Install) => vec![Step::new(backend, action, &["install"])],
        (Backend::Dnf, Action::Search) => vec![Step::new(backend, action, &["search"])],
        (Backend::Dnf, Action::UpdateAll) => {
            vec![Step::new(backend, action, &["upgrade", "--refresh"])]
        }
        (Backend::Dnf, Action::Update) => {
            vec![Step::new(backend, action, &["upgrade", "--refresh"])]
        }
        (Backend::Dnf, Action::Remove) => vec![Step::new(backend, action, &["remove"])],
        (Backend::Dnf, Action::List) => vec![Step::new(backend, action, &["list", "--installed"])],
        (Backend::Dnf, Action::Info) => vec![Step::new(backend, action, &["info"])],

        // ---- flatpak -------------------------------------------------------
        (Backend::Flatpak, Action::Install) => vec![Step::new(backend, action, &["install"])],
        (Backend::Flatpak, Action::Search) => vec![Step::new(backend, action, &["search"])],
        (Backend::Flatpak, Action::UpdateAll) => vec![Step::new(backend, action, &["update"])],
        (Backend::Flatpak, Action::Update) => vec![Step::new(backend, action, &["update"])],
        (Backend::Flatpak, Action::Remove) => vec![Step::new(backend, action, &["uninstall"])],
        (Backend::Flatpak, Action::List) => vec![Step::new(backend, action, &["list"])],
        (Backend::Flatpak, Action::Info) => vec![Step::new(backend, action, &["info"])],

        // ---- snap ----------------------------------------------------------
        (Backend::Snap, Action::Install) => vec![Step::new(backend, action, &["install"])],
        (Backend::Snap, Action::Search) => vec![Step::new(backend, action, &["find"])],
        (Backend::Snap, Action::UpdateAll) => vec![Step::new(backend, action, &["refresh"])],
        (Backend::Snap, Action::Update) => vec![Step::new(backend, action, &["refresh"])],
        (Backend::Snap, Action::Remove) => vec![Step::new(backend, action, &["remove"])],
        (Backend::Snap, Action::List) => vec![Step::new(backend, action, &["list"])],
        (Backend::Snap, Action::Info) => vec![Step::new(backend, action, &["info"])],

        // ---- brew ----------------------------------------------------------
        (Backend::Brew, Action::Install) => vec![Step::new(backend, action, &["install"])],
        (Backend::Brew, Action::Search) => vec![Step::new(backend, action, &["search"])],
        (Backend::Brew, Action::UpdateAll) => vec![
            Step::new(backend, action, &["update"]),
            Step::new(backend, action, &["upgrade"]),
        ],
        (Backend::Brew, Action::Update) => vec![
            Step::new(backend, action, &["update"]),
            Step::new(backend, action, &["upgrade"]),
        ],
        (Backend::Brew, Action::Remove) => vec![Step::new(backend, action, &["uninstall"])],
        (Backend::Brew, Action::List) => vec![Step::new(backend, action, &["list"])],
        (Backend::Brew, Action::Info) => vec![Step::new(backend, action, &["info"])],

        // ---- nix -----------------------------------------------------------
        (Backend::Nix, Action::Install) => {
            vec![Step::new(backend, action, &["profile", "install"])]
        }
        (Backend::Nix, Action::Search) => vec![Step::new(backend, action, &["search", "nixpkgs"])],
        (Backend::Nix, Action::UpdateAll) => {
            vec![Step::new(backend, action, &["profile", "upgrade", "--all"])]
        }
        (Backend::Nix, Action::Update) => {
            vec![Step::new(backend, action, &["profile", "upgrade"])]
        }
        (Backend::Nix, Action::Remove) => vec![Step::new(backend, action, &["profile", "remove"])],
        (Backend::Nix, Action::List) => vec![Step::new(backend, action, &["profile", "list"])],
        (Backend::Nix, Action::Info) => vec![Step::new(backend, action, &["search", "nixpkgs"])],
    };

    // The yes flag goes after the subcommand (`apt install -y`, `flatpak install
    // -y`) rather than before it, which several backends reject.
    if yes {
        if let Some(flag) = backend.yes_flag() {
            for step in &mut steps {
                step.args.push(flag.to_string());
            }
        }
    }

    // Operands and passthrough flags go on the last step only: for the two-step
    // backends the first step is a metadata refresh that takes no package names.
    if let Some(last) = steps.last_mut() {
        last.args.extend(rest);
    }

    steps
}

/// `nix profile install` wants flake references, not bare names, so `pkg install
/// ripgrep` becomes `nixpkgs#ripgrep`. Anything already qualified, and any
/// passthrough flag, is left alone.
fn normalize_operands(backend: Backend, action: Action, rest: &[String]) -> Vec<String> {
    if backend != Backend::Nix || action != Action::Install {
        return rest.to_vec();
    }
    rest.iter()
        .map(|arg| {
            if arg.starts_with('-') || arg.contains('#') || arg.contains(':') {
                arg.clone()
            } else {
                format!("nixpkgs#{arg}")
            }
        })
        .collect()
}

/// Every backend whose executable is on PATH, in detection order.
pub fn detect() -> Vec<Backend> {
    ALL.iter().copied().filter(|b| b.is_installed()).collect()
}

/// Minimal `which`, so pkg does not need a crate to find its own backends.
pub fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(bin))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
