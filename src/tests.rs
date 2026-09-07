use crate::backend::{plan, Action, Backend};

/// Flatten a plan into `["yay -S foo", ...]` so expectations read like shell.
fn cmds(backend: Backend, action: Action, rest: &[&str], yes: bool) -> Vec<String> {
    let rest: Vec<String> = rest.iter().map(|s| s.to_string()).collect();
    plan(backend, action, &rest, yes)
        .iter()
        .map(|s| s.display(None))
        .collect()
}

fn one(backend: Backend, action: Action, rest: &[&str]) -> String {
    let out = cmds(backend, action, rest, false);
    assert_eq!(out.len(), 1, "expected a single step, got {out:?}");
    out.into_iter().next().unwrap()
}

#[test]
fn install_maps_per_backend() {
    let cases = [
        (Backend::Pacman, "pacman -S ripgrep"),
        (Backend::Apt, "apt install ripgrep"),
        (Backend::Dnf, "dnf install ripgrep"),
        (Backend::Yay, "yay -S ripgrep"),
        (Backend::Paru, "paru -S ripgrep"),
        (Backend::Flatpak, "flatpak install ripgrep"),
        (Backend::Snap, "snap install ripgrep"),
        (Backend::Brew, "brew install ripgrep"),
        (Backend::Nix, "nix profile install nixpkgs#ripgrep"),
    ];
    for (backend, expected) in cases {
        assert_eq!(one(backend, Action::Install, &["ripgrep"]), expected);
    }
}

#[test]
fn search_maps_per_backend() {
    let cases = [
        (Backend::Pacman, "pacman -Ss ripgrep"),
        (Backend::Apt, "apt search ripgrep"),
        (Backend::Dnf, "dnf search ripgrep"),
        (Backend::Yay, "yay -Ss ripgrep"),
        (Backend::Paru, "paru -Ss ripgrep"),
        (Backend::Flatpak, "flatpak search ripgrep"),
        (Backend::Snap, "snap find ripgrep"),
        (Backend::Brew, "brew search ripgrep"),
        (Backend::Nix, "nix search nixpkgs ripgrep"),
    ];
    for (backend, expected) in cases {
        assert_eq!(one(backend, Action::Search, &["ripgrep"]), expected);
    }
}

#[test]
fn remove_maps_per_backend() {
    let cases = [
        (Backend::Pacman, "pacman -Rns ripgrep"),
        (Backend::Apt, "apt remove ripgrep"),
        (Backend::Dnf, "dnf remove ripgrep"),
        (Backend::Yay, "yay -Rns ripgrep"),
        (Backend::Paru, "paru -Rns ripgrep"),
        (Backend::Flatpak, "flatpak uninstall ripgrep"),
        (Backend::Snap, "snap remove ripgrep"),
        (Backend::Brew, "brew uninstall ripgrep"),
        (Backend::Nix, "nix profile remove ripgrep"),
    ];
    for (backend, expected) in cases {
        assert_eq!(one(backend, Action::Remove, &["ripgrep"]), expected);
    }
}

#[test]
fn list_and_info_map_per_backend() {
    let list = [
        (Backend::Pacman, "pacman -Q"),
        (Backend::Apt, "apt list --installed"),
        (Backend::Dnf, "dnf list --installed"),
        (Backend::Yay, "yay -Q"),
        (Backend::Paru, "paru -Q"),
        (Backend::Flatpak, "flatpak list"),
        (Backend::Snap, "snap list"),
        (Backend::Brew, "brew list"),
        (Backend::Nix, "nix profile list"),
    ];
    for (backend, expected) in list {
        assert_eq!(one(backend, Action::List, &[]), expected);
    }

    let info = [
        (Backend::Pacman, "pacman -Si ripgrep"),
        (Backend::Apt, "apt show ripgrep"),
        (Backend::Dnf, "dnf info ripgrep"),
        (Backend::Yay, "yay -Si ripgrep"),
        (Backend::Paru, "paru -Si ripgrep"),
        (Backend::Flatpak, "flatpak info ripgrep"),
        (Backend::Snap, "snap info ripgrep"),
        (Backend::Brew, "brew info ripgrep"),
        (Backend::Nix, "nix search nixpkgs ripgrep"),
    ];
    for (backend, expected) in info {
        assert_eq!(one(backend, Action::Info, &["ripgrep"]), expected);
    }
}

#[test]
fn bare_update_refreshes_and_upgrades_everything() {
    let cases: [(Backend, &[&str]); 9] = [
        (Backend::Pacman, &["pacman -Syu"]),
        (Backend::Apt, &["apt update", "apt upgrade"]),
        (Backend::Dnf, &["dnf upgrade --refresh"]),
        (Backend::Yay, &["yay -Syu"]),
        (Backend::Paru, &["paru -Syu"]),
        (Backend::Flatpak, &["flatpak update"]),
        (Backend::Snap, &["snap refresh"]),
        (Backend::Brew, &["brew update", "brew upgrade"]),
        (Backend::Nix, &["nix profile upgrade --all"]),
    ];
    for (backend, expected) in cases {
        assert_eq!(cmds(backend, Action::UpdateAll, &[], false), expected);
    }
}

#[test]
fn updating_one_package_never_produces_a_partial_upgrade_on_pacman() {
    // Arch has no supported partial-upgrade path, so `pkg update foo` upgrades
    // the system as well rather than running -Sy foo.
    assert_eq!(
        one(Backend::Pacman, Action::Update, &["ripgrep"]),
        "pacman -Syu ripgrep"
    );
}

#[test]
fn updating_one_package_on_an_aur_helper_touches_only_that_package() {
    // -S resolves the current AUR version without a sync-DB refresh, so the
    // rest of the system is left where it is.
    for backend in [Backend::Yay, Backend::Paru] {
        assert_eq!(
            one(backend, Action::Update, &["ripgrep"]),
            format!("{backend} -S ripgrep")
        );
    }
}

#[test]
fn updating_one_package_maps_per_backend() {
    let cases: [(Backend, &[&str]); 6] = [
        (
            Backend::Apt,
            &["apt update", "apt install --only-upgrade ripgrep"],
        ),
        (Backend::Dnf, &["dnf upgrade --refresh ripgrep"]),
        (Backend::Flatpak, &["flatpak update ripgrep"]),
        (Backend::Snap, &["snap refresh ripgrep"]),
        (Backend::Brew, &["brew update", "brew upgrade ripgrep"]),
        (Backend::Nix, &["nix profile upgrade ripgrep"]),
    ];
    for (backend, expected) in cases {
        assert_eq!(cmds(backend, Action::Update, &["ripgrep"], false), expected);
    }
}

#[test]
fn yes_translates_to_each_backends_own_flag() {
    assert_eq!(
        cmds(Backend::Pacman, Action::Install, &["ripgrep"], true),
        ["pacman -S --noconfirm ripgrep"]
    );
    assert_eq!(
        cmds(Backend::Apt, Action::Install, &["ripgrep"], true),
        ["apt install -y ripgrep"]
    );
    assert_eq!(
        cmds(Backend::Flatpak, Action::Install, &["ripgrep"], true),
        ["flatpak install -y ripgrep"]
    );
    // Backends that never prompt get no flag invented for them.
    assert_eq!(
        cmds(Backend::Snap, Action::Install, &["ripgrep"], true),
        ["snap install ripgrep"]
    );
    assert_eq!(
        cmds(Backend::Nix, Action::Install, &["ripgrep"], true),
        ["nix profile install nixpkgs#ripgrep"]
    );
}

#[test]
fn yes_applies_to_every_step_of_a_multi_step_plan() {
    assert_eq!(
        cmds(Backend::Apt, Action::UpdateAll, &[], true),
        ["apt update -y", "apt upgrade -y"]
    );
}

#[test]
fn unknown_flags_are_forwarded_verbatim() {
    assert_eq!(
        one(
            Backend::Pacman,
            Action::Install,
            &["ripgrep", "--needed", "--overwrite", "*"]
        ),
        "pacman -S ripgrep --needed --overwrite *"
    );
}

#[test]
fn operands_land_on_the_last_step_only() {
    // `apt update ripgrep` would be an error; the package belongs to the install.
    let steps = plan(
        Backend::Apt,
        Action::Update,
        &["ripgrep".to_string()],
        false,
    );
    assert!(!steps[0].args.contains(&"ripgrep".to_string()));
    assert!(steps[1].args.contains(&"ripgrep".to_string()));
}

#[test]
fn nix_leaves_already_qualified_references_alone() {
    assert_eq!(
        one(Backend::Nix, Action::Install, &["nixpkgs#ripgrep"]),
        "nix profile install nixpkgs#ripgrep"
    );
    assert_eq!(
        one(Backend::Nix, Action::Install, &["github:foo/bar"]),
        "nix profile install github:foo/bar"
    );
}

#[test]
fn only_root_requiring_backends_are_escalated() {
    for backend in [Backend::Pacman, Backend::Apt, Backend::Dnf, Backend::Snap] {
        let steps = plan(backend, Action::Install, &["ripgrep".to_string()], false);
        assert!(
            steps.iter().all(|s| s.needs_root),
            "{backend} should escalate"
        );
    }
    // yay/paru call sudo themselves, flatpak uses polkit, brew/nix are user-level.
    for backend in [
        Backend::Yay,
        Backend::Paru,
        Backend::Flatpak,
        Backend::Brew,
        Backend::Nix,
    ] {
        let steps = plan(backend, Action::Install, &["ripgrep".to_string()], false);
        assert!(
            steps.iter().all(|s| !s.needs_root),
            "{backend} should not escalate"
        );
    }
}

#[test]
fn read_only_actions_never_escalate() {
    for backend in crate::backend::ALL {
        for action in [Action::Search, Action::List, Action::Info] {
            let steps = plan(*backend, action, &["ripgrep".to_string()], false);
            assert!(
                steps.iter().all(|s| !s.needs_root),
                "{backend} {action:?} should not escalate"
            );
        }
    }
}

#[test]
fn escalator_is_shown_in_the_echoed_command() {
    let steps = plan(
        Backend::Pacman,
        Action::Install,
        &["ripgrep".to_string()],
        false,
    );
    assert_eq!(steps[0].display(Some("doas")), "doas pacman -S ripgrep");
    assert_eq!(steps[0].display(None), "pacman -S ripgrep");
}

#[test]
fn every_backend_id_round_trips() {
    for backend in crate::backend::ALL {
        assert_eq!(Backend::parse(backend.id()), Some(*backend));
    }
    assert_eq!(Backend::parse("portage"), None);
}

#[test]
fn path_advice_matches_the_shell() {
    use crate::install_hint::shell_advice;

    assert_eq!(shell_advice("/bin/zsh"), ("~/.zshrc", Some("rehash")));
    assert_eq!(
        shell_advice("/opt/homebrew/bin/fish"),
        ("~/.config/fish/config.fish", None)
    );
    assert_eq!(
        shell_advice("/usr/bin/some-exotic-shell").0,
        "your shell startup file"
    );

    let (rc, rehash) = shell_advice("/bin/bash");
    assert_eq!(rehash, Some("hash -r"));
    // macOS login shells read ~/.bash_profile, not ~/.bashrc.
    assert_eq!(
        rc,
        if cfg!(target_os = "macos") {
            "~/.bash_profile"
        } else {
            "~/.bashrc"
        }
    );
}
