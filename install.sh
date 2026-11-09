#!/usr/bin/env bash
# OpenIDEA Collab — one-line installer / updater
# Usage: curl -fsSL https://raw.githubusercontent.com/SorPuti/openidea-collab/master/install.sh | bash
set -euo pipefail

REPO="SorPuti/openidea-collab"
PLUGIN_ID="sorputi.openidea-collab"
SERVER_PORT="${COLLAB_PORT:-4444}"
INSTALL_DIR="${COLLAB_INSTALL_DIR:-$HOME/.local/bin}"
DOWNLOADS_DIR="${DOWNLOADS_DIR:-$HOME/Downloads}"
IS_UPDATE=false

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
        . /etc/os-release
        OS_ID="${ID:-unknown}"
    elif [ "$(uname)" = "Darwin" ]; then
        OS_ID="macos"
    else
        OS_ID="unknown"
    fi
}

# ── Package manager ───────────────────────────────────────────────────────────
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

# ── Detect if already installed ───────────────────────────────────────────────
check_existing() {
    if [ -f "$INSTALL_DIR/collab-server" ]; then
        IS_UPDATE=true
        echo -e "${BOLD}"
        echo "  ╔════════════════════════════════╗"
        echo "  ║   OpenIDEA Collab Updater      ║"
        echo "  ╚════════════════════════════════╝"
        echo -e "${RESET}"
        info "Existing installation found — updating collab-server and plugin"
    else
        echo -e "${BOLD}"
        echo "  ╔════════════════════════════════╗"
        echo "  ║   OpenIDEA Collab Installer    ║"
        echo "  ╚════════════════════════════════╝"
        echo -e "${RESET}"
    fi
}

# ── Rust ──────────────────────────────────────────────────────────────────────
ensure_rust() {
    header "▸ Rust toolchain"
    if command -v cargo &>/dev/null; then
        ok "Rust $(rustc --version) already installed"
        return
    fi
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
    ok "Rust installed: $(rustc --version)"
}

# ── System dependencies ───────────────────────────────────────────────────────
install_deps() {
    # Skip on update — deps don't change
    if $IS_UPDATE; then return; fi

    header "▸ System dependencies"
    detect_os
    info "Detected OS: $OS_ID"

    case "$OS_ID" in
        fedora|rhel|centos)
            install_pkg gcc curl git openssl-devel pkg-config ;;
        ubuntu|debian|pop|linuxmint|elementary)
            sudo apt-get update -qq
            install_pkg build-essential curl git libssl-dev pkg-config ;;
        arch|manjaro|endeavouros)
            install_pkg base-devel curl git openssl pkg-config ;;
        macos)
            if ! command -v brew &>/dev/null; then
                /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
            fi
            install_pkg curl git openssl pkg-config ;;
        *)
            warn "Unsupported OS — ensure gcc, curl, git, openssl, pkg-config are installed" ;;
    esac
    ok "System dependencies ready"
}

# ── collab-server ─────────────────────────────────────────────────────────────
install_server() {
    header "▸ collab-server"
    mkdir -p "$INSTALL_DIR"

    # Try a pre-built binary first
    LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/' || true)

    if [ -n "$LATEST_TAG" ]; then
        BINARY_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/collab-server-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
        if curl -fsSL --head "$BINARY_URL" 2>/dev/null | grep -q "200\|302"; then
            info "Downloading collab-server $LATEST_TAG..."
            curl -fsSL "$BINARY_URL" -o "$INSTALL_DIR/collab-server"
            chmod +x "$INSTALL_DIR/collab-server"
            ok "collab-server $LATEST_TAG installed → $INSTALL_DIR/collab-server"
            return
        fi
    fi

    warn "No pre-built binary — building from source (may take ~2 min)"
    build_server_from_source
}

build_server_from_source() {
    TMP_DIR=$(mktemp -d)
    # Clean up clone dir on exit, but keep the binary we installed
    trap 'rm -rf "$TMP_DIR"' EXIT
    info "Cloning repository..."
    git clone --depth 1 "https://github.com/$REPO.git" "$TMP_DIR/src"
    info "Building collab-server..."
    cargo build --release --manifest-path "$TMP_DIR/src/collab-server/Cargo.toml"
    cp "$TMP_DIR/src/collab-server/target/release/collab-server" "$INSTALL_DIR/collab-server"
    chmod +x "$INSTALL_DIR/collab-server"
    ok "collab-server built and installed → $INSTALL_DIR/collab-server"
}

# ── Restart service if already running ───────────────────────────────────────
restart_service() {
    case "$OS_ID" in
        macos)
            PLIST="$HOME/Library/LaunchAgents/com.sorputi.collab-server.plist"
            if [ -f "$PLIST" ]; then
                launchctl unload "$PLIST" 2>/dev/null || true
                launchctl load "$PLIST"
                ok "collab-server restarted (launchd)"
            fi ;;
        *)
            if systemctl --user is-active --quiet collab-server.service 2>/dev/null; then
                systemctl --user restart collab-server.service
                ok "collab-server restarted (systemd)"
            fi ;;
    esac
}

# ── systemd service (Linux) ───────────────────────────────────────────────────
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
    ok "Service enabled: collab-server.service (port $SERVER_PORT)"
}

# ── launchd service (macOS) ───────────────────────────────────────────────────
setup_service_macos() {
    header "▸ launchd service"
    PLIST="$HOME/Library/LaunchAgents/com.sorputi.collab-server.plist"
    mkdir -p "$HOME/Library/LaunchAgents"
    cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>              <string>com.sorputi.collab-server</string>
  <key>ProgramArguments</key>  <array><string>$INSTALL_DIR/collab-server</string></array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>COLLAB_PORT</key> <string>$SERVER_PORT</string>
    <key>RUST_LOG</key>    <string>info</string>
  </dict>
  <key>RunAtLoad</key>         <true/>
  <key>KeepAlive</key>         <true/>
  <key>StandardOutPath</key>   <string>/tmp/collab-server.log</string>
  <key>StandardErrorPath</key> <string>/tmp/collab-server.log</string>
</dict>
</plist>
EOF
    launchctl unload "$PLIST" 2>/dev/null || true
    launchctl load "$PLIST"
    ok "launchd service loaded (port $SERVER_PORT)"
}

# ── Plugin .oiplugin download ─────────────────────────────────────────────────
download_plugin() {
    header "▸ Plugin"
    LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/' || true)

    if [ -z "$LATEST_TAG" ]; then
        warn "Could not fetch release tag — get plugin from https://github.com/$REPO/releases"
        PLUGIN_PATH=""; return
    fi

    VERSION="${LATEST_TAG#v}"
    PLUGIN_FILE="${PLUGIN_ID}-${VERSION}.oiplugin"
    PLUGIN_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/$PLUGIN_FILE"
    DEST="$DOWNLOADS_DIR/$PLUGIN_FILE"

    # Skip if same version already downloaded
    if [ -f "$DEST" ]; then
        ok "Plugin $VERSION already in $DOWNLOADS_DIR — skipping download"
        PLUGIN_PATH="$DEST"; return
    fi

    mkdir -p "$DOWNLOADS_DIR"
    info "Downloading $PLUGIN_FILE..."
    if curl -fsSL "$PLUGIN_URL" -o "$DEST"; then
        ok "Plugin downloaded → $DEST"
        PLUGIN_PATH="$DEST"
    else
        warn "Plugin not in releases yet — get it from https://github.com/$REPO/releases"
        PLUGIN_PATH=""
    fi
}

# ── PATH setup ────────────────────────────────────────────────────────────────
ensure_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        for rc in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
            if [ -f "$rc" ] && ! grep -q "$INSTALL_DIR" "$rc"; then
                echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$rc"
                ok "Added $INSTALL_DIR to PATH in $rc"
            fi
        done
        export PATH="$INSTALL_DIR:$PATH"
    fi
}

# ── Verify server is up ───────────────────────────────────────────────────────
verify() {
    header "▸ Verify"
    local retries=8 i=0
    while [ $i -lt $retries ]; do
        if curl -sf "http://localhost:$SERVER_PORT/rooms" -o /dev/null; then
            ok "collab-server is running on port $SERVER_PORT"
            return
        fi
        sleep 1; i=$((i+1))
    done
    warn "Server not responding on port $SERVER_PORT after ${retries}s"
    warn "Check logs: journalctl --user -u collab-server -f"
}

# ── Summary ───────────────────────────────────────────────────────────────────
print_summary() {
    local action
    $IS_UPDATE && action="updated" || action="installed"
    echo
    echo -e "${BOLD}╔══════════════════════════════════════════════════╗${RESET}"
    echo -e "${BOLD}║      OpenIDEA Collab — ${action}! $(printf '%-16s' '')║${RESET}"
    echo -e "${BOLD}╚══════════════════════════════════════════════════╝${RESET}"
    echo
    echo -e "  ${GREEN}collab-server${RESET}  →  $INSTALL_DIR/collab-server"
    echo -e "  ${GREEN}Port${RESET}           →  $SERVER_PORT"
    [ -n "${PLUGIN_PATH:-}" ] && echo -e "  ${GREEN}Plugin${RESET}         →  $PLUGIN_PATH"
    echo
    echo -e "${BOLD}Next steps:${RESET}"
    if $IS_UPDATE; then
        echo -e "  ✓ Server updated and restarted automatically."
        echo
    else
        echo -e "  1. Server is running (auto-starts on login)."
        echo -e "     Logs: journalctl --user -u collab-server -f"
        echo
    fi
    if [ -n "${PLUGIN_PATH:-}" ]; then
        echo -e "  ${$IS_UPDATE && echo '2' || echo '2'}. Install plugin in OpenIDEA:"
        echo -e "     • Drag ${CYAN}$PLUGIN_PATH${RESET} onto the OpenIDEA window, or"
        echo -e "     • Settings → Plugins → Install from file…"
        echo
        echo -e "  3. Run ${CYAN}Collab: Create Room${RESET} to start collaborating."
    else
        echo -e "  2. Install plugin: https://github.com/$REPO/releases"
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
    check_existing  # sets IS_UPDATE and prints header

    if $IS_UPDATE; then
        # Update path: rebuild server + restart, then refresh plugin
        ensure_rust
        install_server
        ensure_path
        restart_service
        download_plugin
        verify
    else
        # Fresh install
        install_deps
        ensure_rust
        install_server
        ensure_path
        case "$OS_ID" in
            macos) setup_service_macos ;;
            *)     setup_service_linux ;;
        esac
        download_plugin
        verify
    fi

    print_summary
}

main "$@"
