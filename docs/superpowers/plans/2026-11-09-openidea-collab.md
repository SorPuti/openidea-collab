# openidea-collab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Plugin Rust/WASM para OpenIDEA que permite colaboração em tempo real entre múltiplos desenvolvedores (e agentes), com sincronização CRDT de arquivos, presença e cursores.

**Architecture:** Plugin WASM (openidea-collab) conecta via WebSocket a um servidor Rust autônomo (collab-server). O servidor mantém salas, documentos CRDT (diamond-types) e estado de presença. Protocolo JSON sobre WebSocket.

**Tech Stack:** Rust 1.80+, axum 0.8, tokio, diamond-types 1.0, serde_json, tokio-tungstenite, openidea-plugin CLI

**Spec:** `docs/superpowers/specs/2026-11-09-openidea-collab-design.md`

## Global Constraints

- Rust edition 2021 em todos os crates
- `cargo test` deve passar sem warnings em ambos os crates
- Sem `unwrap()` em código de produção — usar `?` ou `anyhow`
- Plugin declara capability `network: ["ws://localhost:4444"]` no `plugin.toml`
- Servidor escuta em `0.0.0.0:4444` por padrão, configurável via env `COLLAB_PORT`
- Room IDs: 8 chars alfanuméricos gerados com `rand`
- User IDs: UUID v4

## Review Focus

- **Edições concorrentes no mesmo arquivo**: duas ops chegam antes de cada cliente ver a do outro → ambas devem ser aplicadas sem perda; testar com dois clientes mock enviando simultaneamente
- **Desconexão abrupta**: cliente fecha sem enviar `leave` → servidor deve limpar membro via `on_close` do WebSocket; testar derrubando a conexão
- **Room inexistente no join**: cliente envia `ws://host/ws?room=ZZZZZZZZ` → servidor retorna close frame com código 4040; testar com room_id inválido
- **Op duplicada (reconexão)**: cliente reconecta e reenvia op já aplicada → diamond-types deve deduplicar via agent/seq; testar replay
- **Arquivo não aberto localmente**: servidor recebe `edit` para arquivo que nenhum membro abriu ainda → servidor cria DocState vazio; testar com file path novo

---

## Task 1: Bootstrap — Projetos e Git

**Files:**
- Create: `collab-server/Cargo.toml`
- Create: `collab-server/src/main.rs` (stub)
- Create: `openidea-collab/plugin.toml`
- Create: `openidea-collab/Cargo.toml`
- Create: `openidea-collab/src/lib.rs` (stub)
- Create: `Cargo.toml` (workspace)
- Create: `.gitignore`

**Interfaces:**
- Produces: workspace compilável com `cargo check` sem erros

- [ ] **Step 1: Verificar openidea-plugin instalado**

```bash
openidea-plugin --version
```

Se não instalado:
```bash
cargo install --locked --git https://github.com/l0ng-ai/openidea ide-host --bin openidea-plugin
```

Expected: versão impressa sem erro

- [ ] **Step 2: Scaffold do plugin**

```bash
cd /run/media/bugson/Work/Projetos/OpenIDEA
openidea-plugin new openidea-collab
```

Expected: diretório `openidea-collab/` com `plugin.toml`, `Cargo.toml`, `src/lib.rs`

- [ ] **Step 3: Criar collab-server**

```bash
cargo new --bin collab-server
```

Expected: `collab-server/Cargo.toml` e `collab-server/src/main.rs`

- [ ] **Step 4: Criar workspace Cargo.toml**

Criar `/run/media/bugson/Work/Projetos/OpenIDEA/Cargo.toml`:

```toml
[workspace]
members = ["collab-server", "openidea-collab"]
resolver = "2"
```

- [ ] **Step 5: Git init e primeiro commit**

```bash
git init
git add .
git commit -m "chore: scaffold openidea-collab plugin e collab-server"
```

---

## Task 2: Protocolo Compartilhado (collab-proto)

**Files:**
- Create: `collab-proto/Cargo.toml`
- Create: `collab-proto/src/lib.rs`
- Create: `collab-proto/src/msg.rs`
- Modify: `Cargo.toml` (workspace) — adicionar `collab-proto`

**Interfaces:**
- Produces:
  - `collab_proto::msg::ClientMsg` — enum serde para msgs cliente→servidor
  - `collab_proto::msg::ServerMsg` — enum serde para msgs servidor→cliente
  - `collab_proto::msg::OpData` — struct `{ agent: String, ops: Vec<u8> }` (bytes diamond-types)
  - `collab_proto::msg::MemberInfo` — `{ user_id: String, name: String, current_file: Option<String> }`

- [ ] **Step 1: Escrever teste de serialização**

Criar `collab-proto/tests/msg_roundtrip.rs`:

```rust
use collab_proto::msg::{ClientMsg, ServerMsg, OpData};

#[test]
fn edit_roundtrip() {
    let msg = ClientMsg::Edit {
        file: "src/main.rs".into(),
        op: OpData { agent: "a1".into(), ops: vec![1, 2, 3] },
    };
    let json = serde_json::to_string(&msg).unwrap();
    let back: ClientMsg = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, ClientMsg::Edit { .. }));
}

#[test]
fn welcome_roundtrip() {
    let msg = ServerMsg::Welcome {
        user_id: "u1".into(),
        members: vec![],
    };
    let json = serde_json::to_string(&msg).unwrap();
    let back: ServerMsg = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, ServerMsg::Welcome { .. }));
}
```

- [ ] **Step 2: Rodar para confirmar FAIL**

```bash
cargo test -p collab-proto 2>&1 | head -20
```

Expected: erro de compilação — `collab-proto` não existe ainda

- [ ] **Step 3: Criar collab-proto/Cargo.toml**

```toml
[package]
name = "collab-proto"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 4: Implementar collab-proto/src/lib.rs e msg.rs**

`lib.rs`:
```rust
pub mod msg;
```

`msg.rs` — implementar os enums e structs com `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`. Cada variante usa `#[serde(tag = "t", rename_all = "snake_case")]`. Variantes de `ClientMsg`: `Edit { file, op }`, `Cursor { file, pos: u32 }`, `Presence { file }`, `Leave`. Variantes de `ServerMsg`: `Welcome { user_id, members: Vec<MemberInfo> }`, `MemberJoined { user_id, name }`, `MemberLeft { user_id }`, `Edit { user_id, file, op }`, `Cursor { user_id, file, pos: u32 }`, `PresenceUpdate { user_id, file }`, `ConflictWarn { file, users: Vec<String> }`.

- [ ] **Step 5: Rodar testes**

```bash
cargo test -p collab-proto
```

Expected: 2 testes PASS

- [ ] **Step 6: Adicionar ao workspace e commit**

```bash
# Adicionar "collab-proto" em Cargo.toml workspace members
git add collab-proto/ Cargo.toml
git commit -m "feat: protocolo collab-proto com msgs serde"
```

---

## Task 3A: collab-server — WebSocket + Rooms

> **Pode ser executado em paralelo com Task 3B após Task 2 concluída.**

**Files:**
- Modify: `collab-server/Cargo.toml`
- Create: `collab-server/src/main.rs`
- Create: `collab-server/src/server.rs`
- Create: `collab-server/src/room.rs`
- Create: `collab-server/src/document.rs`
- Create: `collab-server/src/state.rs`
- Test: `collab-server/tests/room_test.rs`

**Interfaces:**
- Consumes: `collab_proto::msg::{ClientMsg, ServerMsg, MemberInfo, OpData}` (Task 2)
- Produces:
  - `state::AppState` — `Arc<Mutex<State>>` compartilhado pelo axum
  - `room::Room` — `{ id: String, members: HashMap<String, Member>, docs: HashMap<String, DocState> }`
  - `document::DocState::apply_op(op: &OpData) -> Result<()>` — aplica op CRDT ao log autoritativo; a op original é rebroadcastada sem transformação (diamond-types gerencia convergência via agent/seq)

- [ ] **Step 1: Escrever testes de room**

Criar `collab-server/tests/room_test.rs`:

```rust
use collab_server::room::{Room, Member};

#[test]
fn room_add_remove_member() {
    let mut room = Room::new("r1".into());
    room.add_member(Member { user_id: "u1".into(), name: "Ana".into(), current_file: None });
    assert_eq!(room.members.len(), 1);
    room.remove_member("u1");
    assert_eq!(room.members.len(), 0);
}

#[test]
fn room_member_list() {
    let mut room = Room::new("r1".into());
    room.add_member(Member { user_id: "u1".into(), name: "Ana".into(), current_file: None });
    room.add_member(Member { user_id: "u2".into(), name: "Bob".into(), current_file: None });
    let list = room.member_list();
    assert_eq!(list.len(), 2);
}

// Review Focus: room inexistente → State retorna None
#[test]
fn state_missing_room_returns_none() {
    use collab_server::state::State;
    let state = State::new();
    assert!(state.get_room("ZZZZZZZZ").is_none());
}
```

- [ ] **Step 2: Rodar para confirmar FAIL**

```bash
cargo test -p collab-server 2>&1 | head -20
```

- [ ] **Step 3: Adicionar dependências ao collab-server/Cargo.toml**

```toml
[dependencies]
axum = { version = "0.8", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
rand = "0.8"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
diamond-types = "1.0"
collab-proto = { path = "../collab-proto" }

[dev-dependencies]
tokio-test = "0.4"
```

- [ ] **Step 4: Implementar room.rs**

`room.rs` define `Member { user_id, name, current_file: Option<String> }` e `Room { id, members: HashMap<String, Member>, docs: HashMap<String, DocState> }`. Métodos: `new(id) -> Room`, `add_member(m)`, `remove_member(id: &str)`, `member_list() -> Vec<MemberInfo>`, `apply_edit(file: &str, op: &OpData) -> Result<OpData>` (delega a `DocState`).

- [ ] **Step 5: Implementar document.rs**

`DocState` wraps `diamond_types::list_fuzzer::SimpleOpLog`. Método `apply_op(op: &OpData) -> Result<OpData>` desserializa os bytes, aplica ao log, serializa o resultado merged para broadcast.

- [ ] **Step 6: Implementar state.rs**

`State { rooms: HashMap<String, Room> }`. Funções: `create_room() -> String` (gera ID 8 chars), `get_room(id: &str) -> Option<&Room>`, `get_room_mut(id: &str) -> Option<&mut Room>`, `get_or_create_room(id: &str) -> &mut Room`.

- [ ] **Step 7: Implementar server.rs e main.rs**

`server.rs`: handler axum `ws_handler(ws: WebSocketUpgrade, Query(params): Query<ConnParams>, State(state): State<AppState>) -> Response`. Lógica: add member → welcome → loop recv/send → on close remove member. `main.rs`: cria AppState, monta router `GET /ws` + `POST /room`, inicia `axum::serve` em `0.0.0.0:{COLLAB_PORT:-4444}`.

- [ ] **Step 8: Rodar testes**

```bash
cargo test -p collab-server
```

Expected: testes de room PASS, build sem warnings

- [ ] **Step 9: Commit**

```bash
git add collab-server/
git commit -m "feat: collab-server WebSocket com rooms e CRDT diamond-types"
```

---

## Task 3B: openidea-collab — Plugin WASM (cliente)

> **Pode ser executado em paralelo com Task 3A após Task 2 concluída.**

**Files:**
- Modify: `openidea-collab/plugin.toml`
- Modify: `openidea-collab/Cargo.toml`
- Create: `openidea-collab/src/lib.rs`
- Create: `openidea-collab/src/client.rs`
- Create: `openidea-collab/src/room_state.rs`
- Create: `openidea-collab/src/panel.rs`
- Test: `openidea-collab/tests/client_test.rs`

**Interfaces:**
- Consumes: `collab_proto::msg::{ClientMsg, ServerMsg}` (Task 2)
- Produces:
  - `client::CollabClient::connect(url: &str) -> Result<Self>`
  - `client::CollabClient::send(&self, msg: ClientMsg) -> Result<()>`
  - `room_state::RoomState` — estado local: `user_id, members, current_file`
  - `panel::render_panel(state: &RoomState) -> String` — HTML do painel de presença

- [ ] **Step 1: Escrever testes do panel**

Criar `openidea-collab/tests/client_test.rs`:

```rust
use openidea_collab::room_state::{RoomState, PeerInfo};
use openidea_collab::panel::render_panel;

#[test]
fn panel_shows_members() {
    let mut state = RoomState::new("u1".into(), "Ana".into());
    state.add_peer(PeerInfo { user_id: "u2".into(), name: "Bob".into(), file: Some("main.rs".into()) });
    let html = render_panel(&state);
    assert!(html.contains("Bob"));
    assert!(html.contains("main.rs"));
}

#[test]
fn panel_empty_room() {
    let state = RoomState::new("u1".into(), "Ana".into());
    let html = render_panel(&state);
    assert!(html.contains("Nenhum colaborador"));
}
```

- [ ] **Step 2: Rodar para confirmar FAIL**

```bash
cargo test -p openidea-collab 2>&1 | head -20
```

- [ ] **Step 3: Configurar plugin.toml**

Atualizar `openidea-collab/plugin.toml`:
```toml
[plugin]
name = "openidea-collab"
version = "0.1.0"
description = "Colaboração em tempo real para OpenIDEA"
authors = ["Seu Nome"]

[capabilities]
network = ["ws://localhost:4444", "ws://*:4444"]

[[commands]]
name = "Create Room"
id = "collab.create_room"

[[commands]]
name = "Join Room"
id = "collab.join_room"

[[commands]]
name = "Leave Room"
id = "collab.leave_room"

[[panels]]
name = "Collaborators"
id = "collab.panel"
position = "right"
```

- [ ] **Step 4: Adicionar dependências ao openidea-collab/Cargo.toml**

```toml
[dependencies]
collab-proto = { path = "../collab-proto" }
serde_json = "1"
diamond-types = "1.0"
anyhow = "1"
# WebSocket via WASM — usar a API do OpenIDEA para conexões de rede
```

Nota: dentro de WASM/OpenIDEA, conexões de rede usam a API do host. Consultar `openidea_plugin::net::WebSocket` ou equivalente da API real do plugin após scaffold.

- [ ] **Step 5: Implementar room_state.rs**

`PeerInfo { user_id, name, file: Option<String> }`. `RoomState { user_id, name, room_id: Option<String>, members: HashMap<String, PeerInfo> }`. Métodos: `new(user_id, name)`, `add_peer(p)`, `remove_peer(uid)`, `update_file(uid, file)`.

- [ ] **Step 6: Implementar panel.rs**

`render_panel(state: &RoomState) -> String` — retorna HTML simples: lista `<ul>` com um `<li>` por peer mostrando nome e arquivo atual. Se `members` vazio, retorna div com "Nenhum colaborador".

- [ ] **Step 7: Implementar lib.rs**

Registrar comandos `collab.create_room`, `collab.join_room`, `collab.leave_room` via API do OpenIDEA. Registrar panel `collab.panel` chamando `render_panel`. Hooks de editor: `on_text_change` → serializa op CRDT → `CollabClient::send(ClientMsg::Edit {...})`. `on_receive(ServerMsg::Edit { op, .. })` → aplica op ao buffer local.

- [ ] **Step 8: Implementar client.rs**

`CollabClient` wraps a WebSocket connection (via API do host OpenIDEA). `connect(url) -> Result<Self>`, `send(msg: ClientMsg) -> Result<()>`, `on_message(callback: Fn(ServerMsg))`.

- [ ] **Step 9: Rodar testes**

```bash
cargo test -p openidea-collab
```

Expected: 2 testes PASS (panel_shows_members, panel_empty_room)

- [ ] **Step 10: Build do plugin**

```bash
cd openidea-collab
openidea-plugin build
```

Expected: build WASM sem erros

- [ ] **Step 11: Commit**

```bash
git add openidea-collab/
git commit -m "feat: plugin openidea-collab com panel de presença e hooks de editor"
```

---

## Task 4: CRDT — Sincronização de Documento Bidirecional

> **Depende de Task 3A e 3B concluídas.**

**Files:**
- Modify: `collab-server/src/document.rs`
- Modify: `openidea-collab/src/client.rs`
- Create: `collab-server/tests/crdt_sync_test.rs`

**Interfaces:**
- Consumes: `document::DocState::apply_op`, `CollabClient::send/receive`
- Produces: sincronização convergente — dois clientes mock que editam simultaneamente chegam ao mesmo estado final

- [ ] **Step 1: Escrever teste de convergência CRDT**

Criar `collab-server/tests/crdt_sync_test.rs`:

```rust
use collab_server::document::DocState;
use collab_proto::msg::OpData;

#[test]
fn concurrent_edits_converge() {
    let mut doc_a = DocState::new();
    let mut doc_b = DocState::new();

    // A insere "hello" na posição 0
    let op_a = doc_a.local_insert(0, "hello").unwrap();
    // B insere "world" na posição 0 (antes de ver op_a)
    let op_b = doc_b.local_insert(0, "world").unwrap();

    // Troca de ops
    doc_a.apply_op(&op_b).unwrap();
    doc_b.apply_op(&op_a).unwrap();

    // Ambos devem ter o mesmo texto (ordem CRDT determinística)
    assert_eq!(doc_a.content(), doc_b.content());
}

#[test]
fn replay_op_is_idempotent() {
    let mut doc = DocState::new();
    let op = doc.local_insert(0, "hi").unwrap();
    doc.apply_op(&op).unwrap(); // replay
    // Não deve duplicar conteúdo
    assert_eq!(doc.content(), "hi");
}
```

- [ ] **Step 2: Rodar para confirmar FAIL**

```bash
cargo test -p collab-server crdt_sync 2>&1 | head -20
```

- [ ] **Step 3: Completar DocState com local_insert e content**

Adicionar a `document.rs`:
- `DocState::new() -> Self` — inicializa `SimpleOpLog` com agent ID UUID
- `local_insert(pos: usize, text: &str) -> Result<OpData>` — cria op local no log, serializa para `OpData`
- `content() -> String` — retorna texto atual do documento
- `apply_op(op: &OpData) -> Result<()>` — desserializa e merge no log (deduplicado pelo diamond-types)

- [ ] **Step 4: Rodar testes CRDT**

```bash
cargo test -p collab-server
```

Expected: todos os testes PASS incluindo `concurrent_edits_converge` e `replay_op_is_idempotent`

- [ ] **Step 5: Commit**

```bash
git add collab-server/src/document.rs collab-server/tests/
git commit -m "feat: CRDT bidirecional com convergência e idempotência via diamond-types"
```

---

## Task 5: Integração e Smoke Test

> **Sequencial — depende de todas as tasks anteriores.**

**Files:**
- Create: `collab-server/tests/integration_test.rs`
- Create: `scripts/dev.sh`

**Interfaces:**
- Consumes: servidor rodando em `localhost:4444`, plugin linkado

- [ ] **Step 1: Script de desenvolvimento**

Criar `scripts/dev.sh`:
```bash
#!/usr/bin/env bash
set -e
echo "▶ Iniciando collab-server..."
cargo run -p collab-server &
SERVER_PID=$!
trap "kill $SERVER_PID" EXIT
sleep 1
echo "▶ Buildando plugin..."
cd openidea-collab && openidea-plugin build && openidea-plugin link
echo "✓ Plugin linkado. Use Settings › Plugins › Reload no OpenIDEA."
wait $SERVER_PID
```

```bash
chmod +x scripts/dev.sh
```

- [ ] **Step 2: Teste de integração HTTP**

Criar `collab-server/tests/integration_test.rs`:

```rust
// Requer servidor rodando — skip em CI sem flag
#[tokio::test]
#[ignore]
async fn create_room_via_http() {
    let resp = reqwest::Client::new()
        .post("http://localhost:4444/room")
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["room_id"].as_str().unwrap().len() == 8);
}
```

Adicionar `reqwest = { version = "0.12", features = ["json"] }` em `[dev-dependencies]` do `collab-server/Cargo.toml`.

- [ ] **Step 3: cargo test completo**

```bash
cargo test --workspace
```

Expected: todos os testes unitários PASS; integração ignorada (marked `#[ignore]`)

- [ ] **Step 4: Build final do plugin**

```bash
cd openidea-collab
openidea-plugin build
openidea-plugin link
```

Expected: sem erros de build WASM

- [ ] **Step 5: Commit final**

```bash
git add scripts/ collab-server/tests/integration_test.rs
git commit -m "feat: script de dev e smoke test de integração"
```

- [ ] **Step 6: Tag v0.1.0**

```bash
git tag v0.1.0
```

---

## Parallelization Guide (para múltiplos agentes)

```
Sequência:
  Task 1 (Bootstrap)
      └── Task 2 (Protocol)
              ├── Task 3A (Server)   ← agente A
              └── Task 3B (Plugin)   ← agente B
                      └── Task 4 (CRDT — agente A ou B)
                              └── Task 5 (Integration)
```

Tasks 3A e 3B podem ser despachadas em paralelo após Task 2 concluída.
