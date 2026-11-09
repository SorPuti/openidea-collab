#!/usr/bin/env bash
# OpenIDEA Collab — animated interactive installer / updater
set -euo pipefail

# ── Colors ────────────────────────────────────────────────────────────────────
R='\033[0;31m'  G='\033[0;32m'  Y='\033[1;33m'
C='\033[0;36m'  B='\033[0;34m'  M='\033[0;35m'
W='\033[1;37m'  D='\033[2m'     X='\033[0m'    BOLD='\033[1m'

# ── Config ────────────────────────────────────────────────────────────────────
REPO="SorPuti/openidea-collab"
SERVER_PORT="${COLLAB_PORT:-4444}"
INSTALL_DIR="${COLLAB_INSTALL_DIR:-$HOME/.local/bin}"
PLUGIN_DIR="${OPENIDEA_COLLAB_DIR:-$HOME/.local/share/openidea-collab-plugin}"
IS_UPDATE=false
SRC_DIR=""
LOG="/tmp/collab-install.log"
LG="pt"
OS_ID="unknown"
SPIN_PID=""
STEP_N=0
STEP_TOTAL=7

# ── i18n ──────────────────────────────────────────────────────────────────────
t() {
    local k="$1"
    if [ "$LG" = "en" ]; then
        case "$k" in
            lang_q)       echo "Select language" ;;
            title)        echo "OpenIDEA Collab Installer" ;;
            upd_title)    echo "OpenIDEA Collab Updater" ;;
            subtitle)     echo "Real-time collaboration for OpenIDEA" ;;
            welcome_msg)  echo "This installer will set up:" ;;
            w_server)     echo "collab-server   (HTTP sync backend)" ;;
            w_plugin)     echo "Plugin          (dev-linked, no marketplace)" ;;
            w_service)    echo "Auto-start      (systemd / launchd)" ;;
            upd_msg)      echo "Existing install found — will update and restart" ;;
            press_enter)  echo "Press Enter to continue" ;;
            conf_title)   echo "Configuration" ;;
            conf_port)    echo "Server port" ;;
            conf_dir)     echo "Install directory" ;;
            s_rust)       echo "Rust toolchain" ;;
            s_wasm)       echo "WASM target (wasm32-wasip2)" ;;
            s_cli)        echo "openidea-plugin CLI" ;;
            s_clone)      echo "Cloning repository" ;;
            s_server)     echo "Building collab-server" ;;
            s_plugin)     echo "Building plugin" ;;
            s_link)       echo "Linking plugin" ;;
            s_service)    echo "Setting up service" ;;
            s_verify)     echo "Verifying installation" ;;
            done_title)   echo "All done!" ;;
            done_restart) echo "Restart OpenIDEA to load the plugin" ;;
            done_logs)    echo "Logs" ;;
            done_ngrok)   echo "Cross-network" ;;
            err_title)    echo "Step failed — check the log" ;;
        esac
    else
        case "$k" in
            lang_q)       echo "Selecione o idioma" ;;
            title)        echo "Instalador OpenIDEA Collab" ;;
            upd_title)    echo "Atualizador OpenIDEA Collab" ;;
            subtitle)     echo "Colaboração em tempo real para OpenIDEA" ;;
            welcome_msg)  echo "O instalador vai configurar:" ;;
            w_server)     echo "collab-server   (backend HTTP de sincronização)" ;;
            w_plugin)     echo "Plugin          (dev-link, sem marketplace)" ;;
            w_service)    echo "Início automático (systemd / launchd)" ;;
            upd_msg)      echo "Instalação existente — será atualizada e reiniciada" ;;
            press_enter)  echo "Pressione Enter para continuar" ;;
            conf_title)   echo "Configuração" ;;
            conf_port)    echo "Porta do servidor" ;;
            conf_dir)     echo "Diretório de instalação" ;;
            s_rust)       echo "Ferramentas Rust" ;;
            s_wasm)       echo "Alvo WASM (wasm32-wasip2)" ;;
            s_cli)        echo "CLI openidea-plugin" ;;
            s_clone)      echo "Clonando repositório" ;;
            s_server)     echo "Compilando collab-server" ;;
            s_plugin)     echo "Compilando plugin" ;;
            s_link)       echo "Vinculando plugin" ;;
            s_service)    echo "Configurando serviço" ;;
            s_verify)     echo "Verificando instalação" ;;
            done_title)   echo "Pronto!" ;;
            done_restart) echo "Reinicie o OpenIDEA para carregar o plugin" ;;
            done_logs)    echo "Logs" ;;
            done_ngrok)   echo "Rede externa" ;;
            err_title)    echo "Etapa falhou — veja o log" ;;
        esac
    fi
}

# ── Terminal helpers ──────────────────────────────────────────────────────────
cls()       { printf '\033[2J\033[H'; }
cursor_off(){ tput civis 2>/dev/null || true; }
cursor_on() { tput cnorm 2>/dev/null || true; }
hr()        { echo -e "   ${D}$(printf '─%.0s' {1..46})${X}"; }

box() {  # box "title" [color]
    local title="$1" col="${2:-$C}" w=50
    local pad=$(( (w - ${#title}) / 2 ))
    echo -e "   ${W}╭$(printf '─%.0s' $(seq 1 $w))╮${X}"
    printf "   ${W}│${X}%${pad}s${col}${BOLD}%s${X}%${pad}s${W}│${X}\n" "" "$title" ""
    echo -e "   ${W}╰$(printf '─%.0s' $(seq 1 $w))╯${X}"
}

# ── Spinner + step runner ─────────────────────────────────────────────────────
FRAMES=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')

_spin_loop() {
    local msg="$1" i=0
    while true; do
        printf "\r   ${C}${FRAMES[$((i % 10))]}${X}  ${D}[${STEP_N}/${STEP_TOTAL}]${X}  ${msg}   "
        sleep 0.07
        i=$((i+1))
    done
}

run_step() {
    local msg="$1"; shift
    STEP_N=$((STEP_N + 1))
    _spin_loop "$msg" &
    SPIN_PID=$!
    if "$@" >> "$LOG" 2>&1; then
        kill "$SPIN_PID" 2>/dev/null; wait "$SPIN_PID" 2>/dev/null; SPIN_PID=""
        printf "\r   ${G}✓${X}  ${D}[${STEP_N}/${STEP_TOTAL}]${X}  ${msg}             \n"
    else
        local code=$?
        kill "$SPIN_PID" 2>/dev/null; wait "$SPIN_PID" 2>/dev/null; SPIN_PID=""
        printf "\r   ${R}✗${X}  ${D}[${STEP_N}/${STEP_TOTAL}]${X}  ${msg}             \n"
        echo
        echo -e "   ${R}$(t err_title):${X} ${D}${LOG}${X}"
        echo
        exit $code
    fi
}

skip_step() {  # skip a step silently (e.g. rust already installed)
    local msg="$1"
    STEP_N=$((STEP_N + 1))
    printf "   ${D}—${X}  ${D}[${STEP_N}/${STEP_TOTAL}]${X}  ${msg}  ${D}(ok)${X}\n"
}

# ── Cleanup ───────────────────────────────────────────────────────────────────
_cleanup() {
    [ -n "${SPIN_PID:-}" ] && kill "$SPIN_PID" 2>/dev/null || true
    cursor_on
    [ -n "${SRC_DIR:-}" ] && rm -rf "$SRC_DIR" 2>/dev/null || true
}
trap _cleanup EXIT

# ── OS detect ─────────────────────────────────────────────────────────────────
detect_os() {
    if [ -f /etc/os-release ]; then . /etc/os-release; OS_ID="${ID:-unknown}"
    elif [ "$(uname)" = "Darwin" ]; then OS_ID="macos"; fi
}

check_existing() {
    [ -f "$INSTALL_DIR/collab-server" ] || [ -d "$PLUGIN_DIR" ] && IS_UPDATE=true || true
}

# ── Screen 1 : language ───────────────────────────────────────────────────────
screen_language() {
    cls; cursor_on; echo
    echo -e "   ${C}${BOLD}⟡  OpenIDEA Collab${X}"
    echo
    echo -e "   ${W}$(t lang_q)${X}"
    echo
    echo -e "   ${Y}1${X}  English"
    echo -e "   ${Y}2${X}  Português (BR)  ${D}← padrão${X}"
    echo
    printf "   › "
    read -r choice
    cursor_off
    [ "$choice" = "1" ] && LG="en" || LG="pt"
}

# ── Screen 2 : welcome ────────────────────────────────────────────────────────
screen_welcome() {
    cls; echo; echo
    if $IS_UPDATE; then
        box "$(t upd_title)" "$Y"
    else
        box "$(t title)" "$C"
    fi
    echo
    echo -e "   ${D}$(t subtitle)${X}"
    echo
    echo -e "   $(t welcome_msg)"
    echo
    echo -e "   ${G}▸${X}  $(t w_server)"
    echo -e "   ${G}▸${X}  $(t w_plugin)"
    echo -e "   ${G}▸${X}  $(t w_service)"
    echo
    if $IS_UPDATE; then
        echo -e "   ${Y}⚑  $(t upd_msg)${X}"
        echo
    fi
    hr
    cursor_on
    printf "\n   ${D}$(t press_enter) ↵${X}  "
    read -r
    cursor_off
}

# ── Screen 3 : config ─────────────────────────────────────────────────────────
screen_config() {
    $IS_UPDATE && return  # skip config on update
    cls; echo
    echo -e "   ${C}${BOLD}⚙  $(t conf_title)${X}"
    echo
    cursor_on
    printf "   ${W}$(t conf_port)${X}      ${D}[${SERVER_PORT}]${X}  › "
    read -r _in; [ -n "$_in" ] && SERVER_PORT="$_in"
    echo
    printf "   ${W}$(t conf_dir)${X}  ${D}[${INSTALL_DIR}]${X}  › "
    read -r _in; [ -n "$_in" ] && INSTALL_DIR="$_in"
    cursor_off; echo
}

# ── Screen 4 : install ────────────────────────────────────────────────────────
screen_install() {
    cls; echo
    local ttl; $IS_UPDATE && ttl="$(t upd_title)" || ttl="$(t title)"
    echo -e "   ${C}${BOLD}⟡  ${ttl}${X}   ${D}v0.2.0${X}"
    echo
    echo -e "   ${D}port ${SERVER_PORT}  ·  ${INSTALL_DIR}${X}"
    hr; echo
}

# ── Step implementations ──────────────────────────────────────────────────────
_do_rust() {
    if ! command -v cargo &>/dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
        source "$HOME/.cargo/env"
    fi
    rustup target add wasm32-wasip2 2>/dev/null || true
}

_do_cli() {
    command -v openidea-plugin &>/dev/null && return
    cargo install --locked --git https://github.com/l0ng-ai/openidea ide-host --bin openidea-plugin
}

_do_clone() {
    SRC_DIR=$(mktemp -d)
    git clone --depth 1 "https://github.com/$REPO.git" "$SRC_DIR"
}

_do_server() {
    mkdir -p "$INSTALL_DIR"
    # Stop service first to avoid ETXTBSY
    systemctl --user stop collab-server.service 2>/dev/null || {
        local p="$HOME/Library/LaunchAgents/com.sorputi.collab-server.plist"
        [ -f "$p" ] && launchctl unload "$p" 2>/dev/null; } || true
    cargo build --release --manifest-path "$SRC_DIR/collab-server/Cargo.toml"
    rm -f "$INSTALL_DIR/collab-server"
    install -m 755 "$SRC_DIR/collab-server/target/release/collab-server" "$INSTALL_DIR/collab-server"
    # Ensure PATH
    [[ ":$PATH:" != *":$INSTALL_DIR:"* ]] && {
        for rc in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
            [ -f "$rc" ] && ! grep -q "$INSTALL_DIR" "$rc" && \
                echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$rc"
        done
        export PATH="$INSTALL_DIR:$PATH"
    }
}

_do_plugin() {
    mkdir -p "$PLUGIN_DIR"
    cp -r "$SRC_DIR/openidea-collab/." "$PLUGIN_DIR/"
    openidea-plugin build "$PLUGIN_DIR"
}

_do_link() {
    openidea-plugin links 2>/dev/null | grep -qF "$PLUGIN_DIR" && \
        openidea-plugin unlink "$PLUGIN_DIR" 2>/dev/null || true
    openidea-plugin link "$PLUGIN_DIR"
}

_do_service_linux() {
    local d="$HOME/.config/systemd/user"
    mkdir -p "$d"
    cat > "$d/collab-server.service" <<EOF
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
}

_do_service_macos() {
    local p="$HOME/Library/LaunchAgents/com.sorputi.collab-server.plist"
    mkdir -p "$HOME/Library/LaunchAgents"
    cat > "$p" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.sorputi.collab-server</string>
  <key>ProgramArguments</key><array><string>$INSTALL_DIR/collab-server</string></array>
  <key>EnvironmentVariables</key><dict>
    <key>COLLAB_PORT</key><string>$SERVER_PORT</string>
    <key>RUST_LOG</key><string>info</string>
  </dict>
  <key>RunAtLoad</key><true/><key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>/tmp/collab-server.log</string>
  <key>StandardErrorPath</key><string>/tmp/collab-server.log</string>
</dict></plist>
EOF
    launchctl load "$p"
}

_do_service() {
    [ "$OS_ID" = "macos" ] && _do_service_macos || _do_service_linux
}

_do_restart() {
    case "$OS_ID" in
        macos)
            local p="$HOME/Library/LaunchAgents/com.sorputi.collab-server.plist"
            [ -f "$p" ] && launchctl load "$p" 2>/dev/null || true ;;
        *)  systemctl --user start collab-server.service 2>/dev/null || true ;;
    esac
}

_do_verify() {
    local i=0
    while [ $i -lt 10 ]; do
        curl -sf "http://localhost:$SERVER_PORT/rooms" -o /dev/null 2>/dev/null && return 0
        sleep 1; i=$((i+1))
    done
    return 1
}

# ── Screen 5 : done ───────────────────────────────────────────────────────────
screen_done() {
    echo; hr; echo
    local ttl; $IS_UPDATE && ttl="$(t upd_title)" || ttl="$(t title)"
    box "$(t done_title)  ${ttl}" "$G"
    echo
    echo -e "   ${W}collab-server${X}  ${D}→${X}  ${INSTALL_DIR}/collab-server"
    echo -e "   ${W}plugin${X}         ${D}→${X}  ${PLUGIN_DIR}"
    echo -e "   ${W}porta${X}          ${D}→${X}  :${SERVER_PORT}"
    echo
    if $IS_UPDATE; then
        echo -e "   ${G}→${X}  $(t done_restart)"
    else
        echo -e "   ${Y}1.${X}  $(t done_restart)"
        echo -e "   ${Y}2.${X}  ${D}$(t done_logs):${X} journalctl --user -u collab-server -f"
        echo -e "   ${Y}3.${X}  ${D}$(t done_ngrok):${X} ngrok http ${SERVER_PORT} → Collab: Set Server URL"
    fi
    echo
    echo -e "   ${D}log: ${LOG}${X}"
    echo
}

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
    > "$LOG"
    cursor_off
    detect_os
    check_existing

    screen_language
    screen_welcome
    screen_config
    screen_install

    if $IS_UPDATE; then
        STEP_TOTAL=6
        run_step  "$(t s_rust)"    _do_rust
        run_step  "$(t s_clone)"   _do_clone
        run_step  "$(t s_server)"  _do_server
        run_step  "$(t s_plugin)"  _do_plugin
        run_step  "$(t s_link)"    _do_link
        run_step  "$(t s_verify)"  _do_restart
    else
        STEP_TOTAL=7
        run_step  "$(t s_rust)"    _do_rust
        run_step  "$(t s_cli)"     _do_cli
        run_step  "$(t s_clone)"   _do_clone
        run_step  "$(t s_server)"  _do_server
        run_step  "$(t s_plugin)"  _do_plugin
        run_step  "$(t s_link)"    _do_link
        run_step  "$(t s_service)" _do_service
    fi

    screen_done
    cursor_on
}

main "$@"
