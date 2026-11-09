#!/usr/bin/env bash
# OpenIDEA Collab — one-line installer / updater
# Usage: curl -fsSL https://raw.githubusercontent.com/SorPuti/openidea-collab/master/install.sh | bash
set -euo pipefail

REPO="SorPuti/openidea-collab"
SERVER_PORT="${COLLAB_PORT:-4444}"
INSTALL_DIR="${COLLAB_INSTALL_DIR:-$HOME/.local/bin}"
PLUGIN_LINK_DIR="${OPENIDEA_COLLAB_DIR:-$HOME/.local/share/openidea-collab-plugin}"
IS_UPDATE=false
SRC_DIR=""   # set by clone_repo

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()   { echo -e "${CYAN}[collab]${RESET} $*"; }
ok()     { echo -e "${GREEN}[✓]${RESET} $*"; }
warn()   { echo -e "${YELLOW}[!]${RESET} $*"; }
die()    { echo -e "${RED}[✗]${RESET} $*" >&2; exit 1; }
header() { echo -e "\n${BOLD}$*${RESET}"; }

# ── OS detection ─────────────────────────────────────────────────────────────
detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release; OS_ID="${ID:-unknown}"
    elif [ "$(uname)" = "Darwin" ]; then
        OS_ID="macos"
    else
        OS_ID="unknown"
    fi
}

install_pkg() {
    case "$OS_ID" in
        fedora|rhel|centos)                     sudo dnf install -y "$@" ;;
        ubuntu|debian|pop|linuxmint|elementary) sudo apt-get install -y "$@" ;;
        arch|manjaro|endeavouros)               sudo pacman -S --noconfirm "$@" ;;
        opensuse*|sles)                         sudo zypper install -y "$@" ;;
        macos)                                  brew install "$@" ;;
        *) sudo apt-get install -y "$@" 2>/dev/null || sudo dnf install -y "$@" ;;
    esac
}

# ── Detect update vs fresh install ───────────────────────────────────────────
check_existing() {
    if [ -f "$INSTALL_DIR/collab-server" ] || [ -d "$PLUGIN_LINK_DIR" ]; then
        IS_UPDATE=true
        echo -e "${BOLD}"
        echo "  ╔════════════════════════════════╗"
        echo "  ║   OpenIDEA Collab Updater      ║"
        echo "  ╚════════════════════════════════╝"
        echo -e "${RESET}"
        info "Existing installation found — updating"
    else
        echo -e "${BOLD}"
        echo "  ╔════════════════════════════════╗"
        echo "  ║   OpenIDEA Collab Installer    ║"
        echo "  ╚════════════════════════════════╝"
        echo -e "${RESET}"
    fi
}

# ── System deps (fresh only) ──────────────────────────────────────────────────
install_deps() {
    header "▸ System dependencies"
    detect_os; info "OS: $OS_ID"
    case "$OS_ID" in
        fedora|rhel|centos)
            install_pkg gcc curl git openssl-devel pkg-config ;;
        ubuntu|debian|pop|linuxmint|elementary)
            sudo apt-get update -qq
            install_pkg build-essential curl git libssl-dev pkg-config ;;
        arch|manjaro|endeavouros)
            install_pkg base-devel curl git openssl pkg-config ;;
        macos)
            command -v brew &>/dev/null || \
                /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
            install_pkg curl git openssl pkg-config ;;
        *) warn "Unknown OS — ensure gcc, curl, git, openssl, pkg-config are installed" ;;
    esac
    ok "System dependencies ready"
}

# ── Rust + wasm target ────────────────────────────────────────────────────────
ensure_rust() {
    header "▸ Rust toolchain"
    if ! command -v cargo &>/dev/null; then
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
        ok "Rust installed: $(rustc --version)"
    else
        ok "Rust $(rustc --version) already installed"
    fi
    # wasm32-wasip2 target needed to build the plugin
    if ! rustup target list --installed 2>/dev/null | grep -q wasm32-wasip2; then
        info "Adding wasm32-wasip2 target..."
        rustup target add wasm32-wasip2
    fi
    ok "wasm32-wasip2 target ready"
}

# ── openidea-plugin CLI ───────────────────────────────────────────────────────
ensure_openidea_cli() {
    header "▸ openidea-plugin CLI"
    if command -v openidea-plugin &>/dev/null; then
        ok "openidea-plugin already installed"
        return
    fi
    info "Installing openidea-plugin CLI..."
    cargo install --locked --git https://github.com/l0ng-ai/openidea ide-host --bin openidea-plugin
    ok "openidea-plugin installed"
}

# ── Clone repo (shared by server + plugin build) ─────────────────────────────
clone_repo() {
    SRC_DIR=$(mktemp -d)
    # Clean up only the temp clone, not installed files
    trap 'rm -rf "$SRC_DIR"' EXIT
    info "Cloning $REPO..."
    git clone --depth 1 "https://github.com/$REPO.git" "$SRC_DIR"
    ok "Cloned to $SRC_DIR"
}

# ── collab-server ─────────────────────────────────────────────────────────────
install_server() {
    header "▸ collab-server"
    mkdir -p "$INSTALL_DIR"

    # Try pre-built binary first
    LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        2>/dev/null | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/' || true)

    if [ -n "$LATEST_TAG" ]; then
        BINARY_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/collab-server-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
        if curl -fsSL --head "$BINARY_URL" 2>/dev/null | grep -q "200\|302"; then
            info "Downloading collab-server $LATEST_TAG..."
            curl -fsSL "$BINARY_URL" -o "$INSTALL_DIR/collab-server"
            chmod +x "$INSTALL_DIR/collab-server"
            ok "collab-server $LATEST_TAG → $INSTALL_DIR/collab-server"
            return
        fi
    fi

    info "Building collab-server from source..."
    cargo build --release --manifest-path "$SRC_DIR/collab-server/Cargo.toml"
    cp "$SRC_DIR/collab-server/target/release/collab-server" "$INSTALL_DIR/collab-server"
    chmod +x "$INSTALL_DIR/collab-server"
    ok "collab-server built → $INSTALL_DIR/collab-server"
}

# ── Plugin: build + link (bypasses publisher trust check) ────────────────────
install_plugin() {
    header "▸ Plugin (build + link)"

    # Install to a persistent dir (not the temp clone)
    mkdir -p "$PLUGIN_LINK_DIR"
    # Copy plugin sources there so the link persists after the temp dir is removed
    cp -r "$SRC_DIR/openidea-collab/." "$PLUGIN_LINK_DIR/"

    info "Building plugin (wasm32-wasip2)..."
    openidea-plugin build "$PLUGIN_LINK_DIR"
    ok "Plugin built"

    # Check if already linked
    if openidea-plugin links 2>/dev/null | grep -qF "$PLUGIN_LINK_DIR"; then
        info "Already linked — re-linking to pick up new build"
        openidea-plugin unlink "$PLUGIN_LINK_DIR" 2>/dev/null || true
    fi

    openidea-plugin link "$PLUGIN_LINK_DIR"
    ok "Plugin linked → OpenIDEA will load it on next restart"
}

# ── Service setup ─────────────────────────────────────────────────────────────
setup_service_linux() {
    header "▸ systemd user service"
    SERVICE_DIR="$HOME/.config/systemd/user"
    mkdir -p "$SERVICE_DIR"
    cat > "$SERVICE_DIR/collab-server.service" <<EOF
[Unit]
Description=OpenIDEA Collab Server
After=network.target

[Service]
ExecStart=$INSTALL_DIR/collab-server
Restart=on-failure
RestartSec=5
Environment=COLLAB_PORT=$SERVER_PORT
Environment=RUST_LOG=info

[Install]
WantedBy=default.target
EOF
    systemctl --user daemon-reload
    systemctl --user enable --now collab-server.service
    ok "Service enabled on port $SERVER_PORT"
}

setup_service_macos() {
    header "▸ launchd service"
    PLIST="$HOME/Library/LaunchAgents/com.sorputi.collab-server.plist"
    mkdir -p "$HOME/Library/LaunchAgents"
    cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key>              <string>com.sorputi.collab-server</string>
  <key>ProgramArguments</key>  <array><string>$INSTALL_DIR/collab-server</string></array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>COLLAB_PORT</key> <string>$SERVER_PORT</string>
    <key>RUST_LOG</key>    <string>info</string>
  </dict>
  <key>RunAtLoad</key>  <true/>
  <key>KeepAlive</key>  <true/>
  <key>StandardOutPath</key> <string>/tmp/collab-server.log</string>
  <key>StandardErrorPath</key><string>/tmp/collab-server.log</string>
</dict></plist>
EOF
    launchctl unload "$PLIST" 2>/dev/null || true
    launchctl load "$PLIST"
    ok "launchd service loaded on port $SERVER_PORT"
}

restart_service() {
    case "$OS_ID" in
        macos)
            PLIST="$HOME/Library/LaunchAgents/com.sorputi.collab-server.plist"
            [ -f "$PLIST" ] && { launchctl unload "$PLIST" 2>/dev/null; launchctl load "$PLIST"; ok "collab-server restarted"; } ;;
        *)
            systemctl --user is-active --quiet collab-server.service 2>/dev/null && \
                { systemctl --user restart collab-server.service; ok "collab-server restarted"; } || true ;;
    esac
}

# ── PATH ──────────────────────────────────────────────────────────────────────
ensure_path() {
    [[ ":$PATH:" == *":$INSTALL_DIR:"* ]] && return
    for rc in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
        if [ -f "$rc" ] && ! grep -q "$INSTALL_DIR" "$rc"; then
            echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$rc"
            ok "Added $INSTALL_DIR to PATH in $rc"
        fi
    done
    export PATH="$INSTALL_DIR:$PATH"
}

# ── Verify ────────────────────────────────────────────────────────────────────
verify() {
    header "▸ Verify"
    local i=0
    while [ $i -lt 8 ]; do
        if curl -sf "http://localhost:$SERVER_PORT/rooms" -o /dev/null; then
            ok "collab-server responding on port $SERVER_PORT"; return
        fi
        sleep 1; i=$((i+1))
    done
    warn "Server not responding yet — check: journalctl --user -u collab-server -f"
}

# ── Summary ───────────────────────────────────────────────────────────────────
print_summary() {
    local action; $IS_UPDATE && action="updated" || action="installed"
    echo
    echo -e "${BOLD}╔══════════════════════════════════════════════════╗${RESET}"
    echo -e "${BOLD}║      OpenIDEA Collab — ${action}!$(printf '%-18s' ' ')║${RESET}"
    echo -e "${BOLD}╚══════════════════════════════════════════════════╝${RESET}"
    echo
    echo -e "  ${GREEN}collab-server${RESET}  →  $INSTALL_DIR/collab-server  (port $SERVER_PORT)"
    echo -e "  ${GREEN}plugin${RESET}         →  $PLUGIN_LINK_DIR  (linked)"
    echo
    echo -e "${BOLD}Next steps:${RESET}"
    if $IS_UPDATE; then
        echo -e "  ✓ Server restarted automatically."
        echo -e "  ✓ Plugin rebuilt and re-linked."
        echo -e "  → ${YELLOW}Restart OpenIDEA${RESET} to load the new plugin version."
    else
        echo -e "  1. Server is running (auto-starts on login)."
        echo -e "     Logs: journalctl --user -u collab-server -f"
        echo
        echo -e "  2. ${YELLOW}Restart OpenIDEA${RESET} — the plugin loads automatically."
        echo
        echo -e "  3. Run ${CYAN}Collab: Create Room${RESET} to start collaborating."
    fi
    echo
    echo -e "  ${YELLOW}Cross-network${RESET}: ${CYAN}ngrok http $SERVER_PORT${RESET} → ${CYAN}Collab: Set Server URL${RESET}"
    echo
}

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
    detect_os
    check_existing

    if $IS_UPDATE; then
        ensure_rust
        clone_repo
        install_server
        install_plugin
        ensure_path
        restart_service
        verify
    else
        install_deps
        ensure_rust
        ensure_openidea_cli
        clone_repo
        install_server
        install_plugin
        ensure_path
        case "$OS_ID" in
            macos) setup_service_macos ;;
            *)     setup_service_linux ;;
        esac
        verify
    fi

    print_summary
}

main "$@"
