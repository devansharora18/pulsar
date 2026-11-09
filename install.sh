#!/bin/sh
set -eu

REPO="https://github.com/devansharora18/pulsar"

if ! command -v cargo >/dev/null 2>&1; then
    echo "pulsar: cargo not found. Install Rust first: https://rustup.rs" >&2
    exit 1
fi

# Prefer a bin directory that is already on PATH.
BIN_DIR="$HOME/.cargo/bin"
if ! printf '%s' ":$PATH:" | grep -Fq ":$HOME/.cargo/bin:"; then
    if printf '%s' ":$PATH:" | grep -Fq ":$HOME/.local/bin:"; then
        BIN_DIR="$HOME/.local/bin"
    fi
fi

if [ "$BIN_DIR" = "$HOME/.local/bin" ]; then
    CARGO_INSTALL_ROOT="$HOME/.local" cargo install --git "$REPO" pulsar --force
else
    cargo install --git "$REPO" pulsar --force
fi

echo
echo "Installed pulsar to $BIN_DIR/pulsar"

# A piped script cannot change the parent shell, so persist PATH for future shells.
if ! printf '%s' ":$PATH:" | grep -Fq ":$BIN_DIR:"; then
    if printf '%s' "${SHELL:-}" | grep -q fish; then
        RC="$HOME/.config/fish/config.fish"
        mkdir -p "$(dirname "$RC")"
        if ! grep -Fq "$BIN_DIR" "$RC" 2>/dev/null; then
            printf '\n# Added by the Pulsar installer\nfish_add_path "%s"\n' "$BIN_DIR" >> "$RC"
            echo "Added $BIN_DIR to PATH in $RC"
        fi
    else
        case "${SHELL:-}" in
            */zsh) RC="$HOME/.zshrc" ;;
            */bash) RC="$HOME/.bashrc" ;;
            *) RC="$HOME/.profile" ;;
        esac
        if ! grep -Fq "$BIN_DIR" "$RC" 2>/dev/null; then
            {
                printf '\n# Added by the Pulsar installer\n'
                printf 'export PATH="%s:$PATH"\n' "$BIN_DIR"
            } >> "$RC"
            echo "Added $BIN_DIR to PATH in $RC"
        fi
    fi
    echo "Open a new terminal, or run: source \"$RC\""
else
    echo "Run: pulsar"
fi
