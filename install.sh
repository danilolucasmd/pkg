#!/bin/sh
# Install pkg -- one set of verbs for every Linux package manager.
#
#   /bin/sh -c "$(curl -fsSL https://raw.githubusercontent.com/danilolucasmd/pkg/main/install.sh)"
#
# Downloads a prebuilt binary for this platform, falling back to building from
# source with cargo. Puts it on your PATH, adding the directory to your shell
# startup file if it is not there already.
#
# Environment overrides:
#   PKG_INSTALL_DIR   where to install         (default: $HOME/.local/bin)
#   PKG_VERSION       tag to install           (default: the latest release)
#   PKG_REPO          owner/name on GitHub     (default: danilolucasmd/pkg)
#   PKG_BASE_URL      release download root    (default: GitHub releases)
#   PKG_NO_MODIFY_PATH  set to 1 to never touch shell startup files

set -eu

REPO="${PKG_REPO:-danilolucasmd/pkg}"
BASE_URL="${PKG_BASE_URL:-https://github.com/$REPO/releases}"
INSTALL_DIR="${PKG_INSTALL_DIR:-$HOME/.local/bin}"
BIN=pkg

say() { printf '%s\n' "$*"; }
err() { printf 'install: %s\n' "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || err "$1 is required but not installed"
}

# ---------------------------------------------------------------- platform ---

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)

    case "$os" in
        Darwin) os=apple-darwin ;;
        Linux)  os=unknown-linux-gnu ;;
        *)      err "unsupported operating system: $os" ;;
    esac

    case "$arch" in
        x86_64 | amd64)  arch=x86_64 ;;
        arm64 | aarch64) arch=aarch64 ;;
        *)               err "unsupported architecture: $arch" ;;
    esac

    printf '%s-%s\n' "$arch" "$os"
}

latest_version() {
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null \
        | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' \
        | head -n 1
}

# ----------------------------------------------------------------- install ---

install_binary() {
    target=$1
    version=$2
    url="$BASE_URL/download/$version/$BIN-$target.tar.gz"

    tmp=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" EXIT

    say "Downloading $BIN $version for $target..."
    if ! curl -fsSL "$url" -o "$tmp/$BIN.tar.gz" 2>/dev/null; then
        return 1
    fi

    tar -xzf "$tmp/$BIN.tar.gz" -C "$tmp" || return 1
    [ -f "$tmp/$BIN" ] || return 1

    mkdir -p "$INSTALL_DIR"
    chmod 755 "$tmp/$BIN"
    mv -f "$tmp/$BIN" "$INSTALL_DIR/$BIN"
    return 0
}

install_from_source() {
    say "No prebuilt binary available; building from source..."
    command -v cargo >/dev/null 2>&1 || err \
"cargo is required to build from source. Install Rust first:

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

then re-run this installer."

    mkdir -p "$INSTALL_DIR"
    # --root puts the binary in $INSTALL_DIR/bin, so build to a staging root.
    stage=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf '$stage'" EXIT
    cargo install --git "https://github.com/$REPO" --root "$stage" --force >&2
    mv -f "$stage/bin/$BIN" "$INSTALL_DIR/$BIN"
}

# -------------------------------------------------------------------- PATH ---

on_path() {
    case ":${PATH:-}:" in
        *":$INSTALL_DIR:"*) return 0 ;;
        *) return 1 ;;
    esac
}

# Echoes: <startup file>|<reload command>|<append line>
shell_profile() {
    case "$(basename "${SHELL:-sh}")" in
        zsh)
            printf '%s|%s|%s\n' "$HOME/.zshrc" "source ~/.zshrc && rehash" \
                "export PATH=\"$INSTALL_DIR:\$PATH\""
            ;;
        bash)
            # macOS terminals start login shells, which read .bash_profile.
            if [ "$(uname -s)" = Darwin ]; then rc="$HOME/.bash_profile"; else rc="$HOME/.bashrc"; fi
            printf '%s|%s|%s\n' "$rc" "source $rc && hash -r" \
                "export PATH=\"$INSTALL_DIR:\$PATH\""
            ;;
        fish)
            printf '%s|%s|%s\n' "$HOME/.config/fish/config.fish" \
                "source ~/.config/fish/config.fish" "fish_add_path $INSTALL_DIR"
            ;;
        *) return 1 ;;
    esac
}

setup_path() {
    if on_path; then
        return 0
    fi

    if [ "${PKG_NO_MODIFY_PATH:-0}" = 1 ] || ! profile=$(shell_profile); then
        say ""
        say "$INSTALL_DIR is not on your PATH. Add it to your shell startup file:"
        say ""
        say "    export PATH=\"$INSTALL_DIR:\$PATH\""
        return 0
    fi

    rc=$(printf '%s' "$profile" | cut -d'|' -f1)
    reload=$(printf '%s' "$profile" | cut -d'|' -f2)
    line=$(printf '%s' "$profile" | cut -d'|' -f3)

    # Adding the same line twice on re-install would be untidy but harmless;
    # skipping it keeps repeated installs idempotent.
    if [ -f "$rc" ] && grep -qF "$INSTALL_DIR" "$rc"; then
        say "$INSTALL_DIR is already referenced in $rc."
    else
        mkdir -p "$(dirname "$rc")"
        printf '\n# added by the pkg installer\n%s\n' "$line" >> "$rc"
        say "Added $INSTALL_DIR to your PATH in $rc."
    fi

    PATH_RELOAD=$reload
}

# -------------------------------------------------------------------- main ---

main() {
    need curl
    need tar

    target=$(detect_target)
    version="${PKG_VERSION:-}"
    [ -n "$version" ] || version=$(latest_version || true)

    if [ -z "$version" ] || ! install_binary "$target" "$version"; then
        install_from_source
    fi

    [ -x "$INSTALL_DIR/$BIN" ] || err "installation failed"

    PATH_RELOAD=""
    setup_path

    say ""
    say "Installed $("$INSTALL_DIR/$BIN" --version) to $INSTALL_DIR/$BIN"
    if [ -n "$PATH_RELOAD" ]; then
        say ""
        say "Open a new terminal, or run:"
        say ""
        say "    $PATH_RELOAD"
    fi
    say ""
    say "Then get started with:"
    say ""
    say "    $BIN install ripgrep"
    say ""
}

main "$@"
