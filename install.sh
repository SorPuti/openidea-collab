#!/usr/bin/env bash
# OpenIDEA Collab — one-line installer
# Usage: curl -fsSL https://raw.githubusercontent.com/SorPuti/openidea-collab/master/install.sh | bash
set -euo pipefail

REPO="SorPuti/openidea-collab"
PLUGIN_ID="sorputi.openidea-collab"
SERVER_PORT="${COLLAB_PORT:-4444}"
INSTALL_DIR="${COLLAB_INSTALL_DIR:-$HOME/.local/bin}"
DOWNLOADS_DIR="${DOWNLOADS_DIR:-$HOME/Downloads}"

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}[collab]${RESET} $*"; }
ok()      { echo -e "${GREEN}[✓]${RESET} $*"; }
warn()    { echo -e "${YELLOW}[!]${RESET} $*"; }
die()     { echo -e "${RED}[✗]${RESET} $*" >&2; exit 1; }
header()  { echo -e "\n${BOLD}$*${RESET}"; }

# ── OS detection ─────────────────────────────────────────────────────────────
detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS_ID="${ID:-unknown}"
        OS_LIKE="${ID_LIKE:-}"
    elif [ "$(uname)" = "Darwin" ]; then
        OS_ID="macos"
    else
        OS_ID="unknown"
    fi
}

# ── Package manager ───────────────────────────────────────────────────────────
install_pkg() {
    case "$OS_ID" in
        fedora|rhel|centos)         sudo dnf install -y "$@" ;;
        ubuntu|debian|pop|linuxmint|elementary)
                                    sudo apt-get install -y "$@" ;;
        arch|manjaro|endeavouros)   sudo pacman -S --noconfirm "$@" ;;
        opensuse*|sles)             sudo zypper install -y "$@" ;;
        macos)                      brew install "$@" ;;
        *)  warn "Unknown OS — trying apt-get, then dnf"
            sudo apt-get install -y "$@" 2>/dev/null || sudo dnf install -y "$@" ;;
    esac
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
    header "▸ System dependencies"
    detect_os
    info "Detected OS: $OS_ID"

    case "$OS_ID" in
        fedora|rhel|centos)
            info "Installing build tools..."
            install_pkg gcc curl git openssl-devel pkg-config
            ;;
        ubuntu|debian|pop|linuxmint|elementary)
            info "Updating package lists..."
            sudo apt-get update -qq
            info "Installing build tools..."
            install_pkg build-essential curl git libssl-dev pkg-config
            ;;
        arch|manjaro|endeavouros)
            install_pkg base-devel curl git openssl pkg-config
            ;;
        macos)
            if ! command -v brew &>/dev/null; then
                info "Installing Homebrew..."
                /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
            fi
            install_pkg curl git openssl pkg-config
            ;;
        *)
            warn "Unsupported OS — skipping system deps. Install gcc, curl, git, openssl, pkg-config manually."
            ;;
    esac
    ok "System dependencies ready"
}

# ── collab-server binary ───────────────────────────────────────────────────────
install_server() {
    header "▸ collab-server"
    mkdir -p "$INSTALL_DIR"

    # Try downloading a pre-built binary from latest release first
    LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')

    if [ -z "$LATEST_TAG" ]; then
        warn "Could not fetch latest release tag — building from source"
        build_server_from_source
        return
    fi

    BINARY_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/collab-server-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"

    if curl -fsSL --head "$BINARY_URL" | grep -q "200\|302"; then
        info "Downloading collab-server $LATEST_TAG..."
        curl -fsSL "$BINARY_URL" -o "$INSTALL_DIR/collab-server"
        chmod +x "$INSTALL_DIR/collab-server"
        ok "collab-server installed → $INSTALL_DIR/collab-server"
    else
        warn "No pre-built binary for this platform — building from source (may take ~2 min)"
        build_server_from_source
    fi
}

build_server_from_source() {
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT
    info "Cloning repository..."
    git clone --depth 1 "https://github.com/$REPO.git" "$TMP_DIR/openidea-collab"
    info "Building collab-server..."
    cargo build --release --manifest-path "$TMP_DIR/openidea-collab/collab-server/Cargo.toml"
    mkdir -p "$INSTALL_DIR"
    cp "$TMP_DIR/openidea-collab/collab-server/target/release/collab-server" "$INSTALL_DIR/collab-server"
    chmod +x "$INSTALL_DIR/collab-server"
    ok "collab-server built and installed → $INSTALL_DIR/collab-server"
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
    launchctl load "$PLIST"
    ok "launchd service loaded (port $SERVER_PORT)"
}

# ── Plugin .oiplugin download ─────────────────────────────────────────────────
download_plugin() {
    header "▸ Plugin download"
    LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')

    if [ -z "$LATEST_TAG" ]; then
        warn "Could not determine latest release tag — skipping plugin download"
        return
    fi

    VERSION="${LATEST_TAG#v}"
    PLUGIN_FILE="${PLUGIN_ID}-${VERSION}.oiplugin"
    PLUGIN_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/$PLUGIN_FILE"

    mkdir -p "$DOWNLOADS_DIR"
    info "Downloading $PLUGIN_FILE..."
    if curl -fsSL "$PLUGIN_URL" -o "$DOWNLOADS_DIR/$PLUGIN_FILE"; then
        ok "Plugin downloaded → $DOWNLOADS_DIR/$PLUGIN_FILE"
        PLUGIN_PATH="$DOWNLOADS_DIR/$PLUGIN_FILE"
    else
        warn "Could not download plugin (release may not exist yet)"
        warn "Get it manually from: https://github.com/$REPO/releases"
        PLUGIN_PATH=""
    fi
}

# ── PATH setup ────────────────────────────────────────────────────────────────
ensure_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        warn "$INSTALL_DIR is not in PATH"
        for rc in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
            [ -f "$rc" ] && echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$rc" && ok "Added to $rc"
        done
        export PATH="$INSTALL_DIR:$PATH"
    fi
}

# ── Verify ────────────────────────────────────────────────────────────────────
verify() {
    header "▸ Verify"
    if "$INSTALL_DIR/collab-server" --version &>/dev/null; then
        ok "collab-server $("$INSTALL_DIR/collab-server" --version 2>&1 | head -1)"
    else
        warn "collab-server installed but --version not supported (run it to verify)"
    fi
    sleep 1
    if curl -sf "http://localhost:$SERVER_PORT/rooms" &>/dev/null; then
        ok "Server is responding on port $SERVER_PORT"
    else
        warn "Server not responding yet on port $SERVER_PORT (may still be starting)"
    fi
}

# ── Summary ───────────────────────────────────────────────────────────────────
print_summary() {
    echo
    echo -e "${BOLD}╔══════════════════════════════════════════════════╗${RESET}"
    echo -e "${BOLD}║        OpenIDEA Collab — setup complete!         ║${RESET}"
    echo -e "${BOLD}╚══════════════════════════════════════════════════╝${RESET}"
    echo
    echo -e "  ${GREEN}collab-server${RESET}  →  $INSTALL_DIR/collab-server"
    echo -e "  ${GREEN}Port${RESET}           →  $SERVER_PORT"
    if [ -n "${PLUGIN_PATH:-}" ]; then
    echo -e "  ${GREEN}Plugin file${RESET}    →  $PLUGIN_PATH"
    fi
    echo
    echo -e "${BOLD}Next steps:${RESET}"
    echo -e "  1. The collab-server is running (auto-starts on login)."
    echo -e "     Logs: journalctl --user -u collab-server -f"
    echo
    if [ -n "${PLUGIN_PATH:-}" ]; then
    echo -e "  2. Install the plugin in OpenIDEA:"
    echo -e "     • Drag ${CYAN}$PLUGIN_PATH${RESET} onto the OpenIDEA window, or"
    echo -e "     • Settings → Plugins → Install from file… → select the .oiplugin"
    echo
    echo -e "  3. Run ${CYAN}Collab: Create Room${RESET} in OpenIDEA to start collaborating."
    else
    echo -e "  2. Install the plugin: download the .oiplugin from"
    echo -e "     ${CYAN}https://github.com/$REPO/releases${RESET}"
    echo -e "     and drag it onto the OpenIDEA window."
    echo
    echo -e "  3. Run ${CYAN}Collab: Create Room${RESET} in OpenIDEA to start collaborating."
    fi
    echo
    echo -e "  ${YELLOW}Cross-network${RESET}: expose the server with  ${CYAN}ngrok http $SERVER_PORT${RESET}"
    echo -e "  then run ${CYAN}Collab: Set Server URL${RESET} and paste the HTTPS URL."
    echo
}

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
    echo -e "${BOLD}"
    echo "  ╔════════════════════════════════╗"
    echo "  ║   OpenIDEA Collab Installer    ║"
    echo "  ╚════════════════════════════════╝"
    echo -e "${RESET}"

    detect_os
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
    print_summary
}

main "$@"
