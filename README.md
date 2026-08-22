# pkg

One set of verbs for every Linux package manager.

`pkg install ripgrep` runs `yay -S ripgrep` on Arch, `apt install ripgrep` on Debian, `dnf install ripgrep` on Fedora. You pick the backend once; pkg remembers it.

```console
$ pkg install ripgrep
-> yay -S ripgrep

$ pkg search ripgrep
-> yay -Ss ripgrep

$ pkg update ripgrep      # update one package
-> yay -Syu ripgrep

$ pkg update              # refresh metadata and upgrade the system
-> yay -Syu
```

pkg always echoes the command it is about to run, so it doubles as a way to learn the native syntax.

## Install

One command, identical on macOS and every Linux distribution:

```sh
/bin/sh -c "$(curl -fsSL https://raw.githubusercontent.com/danilolucasmd/pkg/main/install.sh)"
```

It detects your platform, downloads the matching prebuilt binary (x86_64 and arm64, macOS and Linux), installs it to `~/.local/bin`, and adds that directory to your `PATH` in the correct startup file for your shell — `~/.zshrc`, `~/.bashrc`, `~/.bash_profile` on macOS, or `config.fish`. Open a new terminal afterwards and `pkg install ...` works.

If no prebuilt binary matches your platform, the installer falls back to building from source with cargo.

The installer respects a few environment variables:

| Variable | Default | Purpose |
| --- | --- | --- |
| `PKG_INSTALL_DIR` | `~/.local/bin` | Where the binary goes |
| `PKG_VERSION` | latest release | Install a specific tag |
| `PKG_NO_MODIFY_PATH` | unset | Set to `1` to never touch shell startup files |

```sh
# system-wide, without editing any profile
curl -fsSL https://raw.githubusercontent.com/danilolucasmd/pkg/main/install.sh \
  | PKG_INSTALL_DIR=/usr/local/bin PKG_NO_MODIFY_PATH=1 sudo -E sh
```

### Other ways

```sh
cargo install --git https://github.com/danilolucasmd/pkg   # needs Rust
cargo install --path .                                     # from a clone
makepkg -si                                                # Arch, via the included PKGBUILD
```

### If `pkg` is not found afterwards

This only happens with the `cargo install` routes; the installer script handles it for you. `cargo install` writes to `~/.cargo/bin`, which is added to your `PATH` only if you installed Rust through rustup — a Homebrew or distro Rust leaves it off, so `pkg` works only by full path.

pkg detects this itself: the first run, and any `pkg config show`, prints the exact line to add for your shell. For zsh:

```sh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
rehash
```

`rehash` matters for zsh, which caches command lookups; a new terminal tab works too.

## First run

The first time you run any pkg command without a config file, pkg detects the package managers on your `PATH` and asks which one to use:

```
? Which package manager should pkg use?
> pacman   Arch Linux native packages
  yay      Arch repos + AUR (yay)
  paru     Arch repos + AUR (paru)
```

The answer is saved to `~/.config/pkg/pkg.conf` and you are never asked again. If exactly one manager is detected, pkg picks it without asking. In a script or CI (no TTY), pkg fails with instructions rather than hanging.

## Configuration

`~/.config/pkg/pkg.conf` (or `$XDG_CONFIG_HOME/pkg/pkg.conf`):

```toml
backend = "yay"
escalator = "sudo"
```

| Key | Values | Meaning |
| --- | --- | --- |
| `backend` | `pacman` `apt` `dnf` `yay` `paru` `flatpak` `snap` `brew` `nix` | The package manager pkg drives |
| `escalator` | `sudo` `doas` `run0` `none` | How pkg gains root when the backend needs it |

Manage it from the CLI:

```sh
pkg config show                 # current settings + detected managers
pkg config path                 # where the file lives
pkg config init                 # re-run the backend picker
pkg config get backend
pkg config set backend paru
pkg config set escalator doas
```

Set `PKG_CONFIG_FILE` to point pkg at a different config file, which is handy for testing.

## Commands

| Command | Aliases | Notes |
| --- | --- | --- |
| `pkg install <pkg>...` | `i`, `add` | |
| `pkg search <query>` | `s` | Backend output is passed through unmodified, colours and all |
| `pkg update [<pkg>...]` | `u` | With no arguments: refresh metadata **and** upgrade everything |
| `pkg remove <pkg>...` | `rm`, `uninstall` | |
| `pkg list` | `ls` | Installed packages |
| `pkg info <pkg>` | | |
| `pkg config <subcommand>` | | See above |

### Global flags

- `-y`, `--yes` — answer yes to the backend's prompts, translated per backend (`--noconfirm` for pacman-family, `-y` for apt/dnf/flatpak, omitted for backends that never prompt).
- `-w <backend>`, `--with <backend>` — use a different backend for this one command, e.g. `pkg install --with flatpak org.gimp.GIMP`.

Anything pkg does not recognise is forwarded to the backend verbatim:

```console
$ pkg install ripgrep --needed --overwrite '*'
-> yay -S ripgrep --needed --overwrite *
```

Because of that passthrough, **pkg's own flags must come before the package names**. `pkg install -y foo` works; `pkg install foo -y` sends `-y` to the backend.

## What each verb becomes

| | `install` | `search` | `update <pkg>` | `update` | `remove` | `list` | `info` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **pacman** | `-S` | `-Ss` | `-Syu <pkg>` | `-Syu` | `-Rns` | `-Q` | `-Si` |
| **yay** | `-S` | `-Ss` | `-Syu <pkg>` | `-Syu` | `-Rns` | `-Q` | `-Si` |
| **paru** | `-S` | `-Ss` | `-Syu <pkg>` | `-Syu` | `-Rns` | `-Q` | `-Si` |
| **apt** | `install` | `search` | `update` + `install --only-upgrade` | `update` + `upgrade` | `remove` | `list --installed` | `show` |
| **dnf** | `install` | `search` | `upgrade --refresh <pkg>` | `upgrade --refresh` | `remove` | `list --installed` | `info` |
| **flatpak** | `install` | `search` | `update <pkg>` | `update` | `uninstall` | `list` | `info` |
| **snap** | `install` | `find` | `refresh <pkg>` | `refresh` | `remove` | `list` | `info` |
| **brew** | `install` | `search` | `update` + `upgrade <pkg>` | `update` + `upgrade` | `uninstall` | `list` | `info` |
| **nix** | `profile install nixpkgs#<pkg>` | `search nixpkgs` | `profile upgrade <pkg>` | `profile upgrade --all` | `profile remove` | `profile list` | `search nixpkgs` |

Notes:

- **Arch has no supported partial-upgrade path**, so `pkg update <pkg>` on pacman/yay/paru expands to `-Syu <pkg>` rather than `-Sy <pkg>`. Updating one package upgrades the system with it, which is the only safe thing to do.
- Where a verb needs two commands, they run in order and stop at the first failure. Package names attach to the last step only, so `pkg update foo` on apt runs a bare `apt update` and then `apt install --only-upgrade foo`.
- `nix info` reuses `nix search`, which is the closest equivalent nix offers.
- Bare `pkg install foo` on nix becomes `nixpkgs#foo`; anything already containing `#` or `:` is left alone.

## Privilege escalation

pkg prepends your escalator only when the backend actually needs root:

| Escalated | Not escalated |
| --- | --- |
| pacman, apt, dnf, snap | yay, paru (they call sudo themselves), flatpak (polkit), brew (refuses to run as root), nix (per-user profiles) |

Read-only verbs (`search`, `list`, `info`) never escalate, and nothing is escalated when you are already root.

## Development

```sh
cargo test          # command-mapping tests for every backend x verb pair
cargo clippy --all-targets
cargo fmt
```

The mapping lives in one function, `backend::plan`, and the tests assert the exact argv it produces for each backend.

## License

MIT
