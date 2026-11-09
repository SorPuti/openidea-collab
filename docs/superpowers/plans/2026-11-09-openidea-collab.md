# openidea-collab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Plugin Rust/WASM para OpenIDEA que sincroniza arquivos entre colaboradores via HTTP REST, usando file-save events do IDE e polling do servidor.

**Architecture:** Plugin WASM escuta `file-saved`, POSTa texto completo ao collab-server. Background task faz long-poll de eventos do servidor e aplica mudanças remotas via `project.write-file`. Servidor Rust mantém salas, histórico de arquivos e faz merge CRDT (diamond-types) em conflitos.

**Tech Stack:** Rust 2021, axum 0.8, tokio, diamond-types 1.0, serde_json, openidea_plugin SDK (git), openidea-plugin CLI

**Spec:** `docs/superpowers/specs/2026-11-09-openidea-collab-design.md`

## Global Constraints

- Rust edition 2021 em todos os crates
- `cargo test` deve passar sem warnings (`-D warnings`)
- Sem `unwrap()` em código de produção — usar `?` ou `anyhow::bail!`
- Plugin capabilities: `required = ["project.read", "project.write", "events", "storage"]`, `optional = ["net:localhost", "net.http:*"]`
- Servidor em `0.0.0.0:4444` por padrão, `COLLAB_PORT` env override
- Room IDs: 8 chars alfanuméricos (`rand::Rng::sample_iter(Alphanumeric).take(8)`)
- User IDs: UUID v4
- Long-poll timeout no servidor: 5000ms
- Plugin polling interval: 3000ms entre tentativas

## Review Focus

- **Conflito simultâneo**: dois usuários salvam o mesmo arquivo antes de qualquer um receber o update do outro → servidor deve tentar 3-way merge via diamond-types; se falhar, retornar 409 com ambas versões; testar com dois clientes mock
- **Desconexão sem DELETE**: cliente para de polling (crash) → servidor deve expirar membro após 30s sem heartbeat (implementar `last_seen` + cleanup task); testar travando o clock
- **Room 404 em POST file**: cliente posta arquivo para room que foi deletada → servidor retorna 404; plugin mostra notificação de erro e para polling
- **Write-file com editor aberto**: `project.write-file` falha se editor tem unsaved changes (`invalid`); plugin deve mostrar notificação e NÃO substituir silenciosamente; testar com `apply-edits` como fallback
- **Cursor overflow**: `GET /events?cursor=999999` para room com 10 eventos → servidor retorna `{ events: [], next_cursor: 10 }`; testar limite superior

---

## Task 1: Bootstrap — Projetos e Git

**Files:**
- Create: `Cargo.toml` (workspace)
- Create: `collab-server/Cargo.toml`
- Create: `collab-server/src/main.rs` (stub)
- Create: `openidea-collab/plugin.toml`
- Create: `openidea-collab/Cargo.toml`
- Create: `openidea-collab/src/lib.rs` (stub)

**Interfaces:**
- Produces: workspace com `cargo check` passando sem erros

- [ ] **Step 1: Verificar openidea-plugin instalado**

```bash
openidea-plugin --version 2>/dev/null || cargo install --locked --git https://github.com/l0ng-ai/openidea ide-host --bin openidea-plugin
```

Expected: versão impressa sem erro (instalação pode demorar ~5min)

- [ ] **Step 2: Scaffold do plugin no worktree**

```bash
cd /run/media/bugson/Work/Projetos/OpenIDEA/.worktrees/openidea-collab
openidea-plugin new openidea-collab
```

Expected: `openidea-collab/plugin.toml`, `openidea-collab/Cargo.toml`, `openidea-collab/src/lib.rs`

- [ ] **Step 3: Criar collab-server**

```bash
cargo new --bin collab-server
```

- [ ] **Step 4: Criar workspace Cargo.toml**

```toml
[workspace]
members = ["collab-server", "openidea-collab"]
resolver = "2"
```

- [ ] **Step 5: Ajustar openidea-collab/plugin.toml**

Substituir o conteúdo completo pelo abaixo (preservar o `wasm` path que o scaffold gerou):

```toml
id = "ai.l0ng.openidea-collab"
name = "OpenIDEA Collab"
version = "0.1.0"
description = "Real-time collaboration for OpenIDEA: sync files between developers."
authors = ["OpenIDEA Collab contributors"]
license = "Apache-2.0"
change-notes = "0.1.0 — first release: room creation, file sync, presence panel."
categories = ["tools"]
engines.openidea = "^1.0"
wasm = "dist/plugin.wasm"
activation = ["on-event:file-saved", "on-event:editor-focused", "on-command:*"]

[capabilities]
required = ["project.read", "project.write", "events", "storage"]
optional = ["net:localhost", "net.http:*"]

[[contributes.commands]]
id = "create-room"
title = "Collab: Create Room"
category = "Collab"

[[contributes.commands]]
id = "join-room"
title = "Collab: Join Room"
category = "Collab"

[[contributes.commands]]
id = "leave-room"
title = "Collab: Leave Room"
category = "Collab"

[[contributes.views]]
id = "collab-panel"
name = "Collaborators"
icon = "users"
location = "right"
```

- [ ] **Step 6: cargo check**

```bash
cd /run/media/bugson/Work/Projetos/OpenIDEA/.worktrees/openidea-collab
cargo check --workspace 2>&1 | tail -5
```

Expected: sem erros de compilação (warnings OK neste ponto)

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml collab-server/ openidea-collab/
git commit -m "chore: scaffold workspace collab-server + openidea-collab plugin"
```

---

## Task 2: collab-server — HTTP REST + Rooms

**Files:**
- Modify: `collab-server/Cargo.toml`
- Create: `collab-server/src/main.rs`
- Create: `collab-server/src/routes.rs`
- Create: `collab-server/src/room.rs`
- Create: `collab-server/src/events.rs`
- Create: `collab-server/src/document.rs`
- Test: `collab-server/tests/server_test.rs`

**Interfaces:**
- Produces:
  - `room::Room { id, members: HashMap<String,Member>, files: HashMap<String,FileState>, events: EventStore }`
  - `room::Member { user_id: String, name: String, current_file: Option<String>, last_seen: Instant }`
  - `room::FileState { version: u64, text: String, last_user: String }`
  - `events::EventStore::push(event: RoomEvent) -> u64` — returns event id
  - `events::EventStore::since(cursor: u64, timeout_ms: u64) -> Vec<EventEnvelope>` — long-poll, blocks up to timeout_ms
  - `document::merge3(base: &str, ours: &str, theirs: &str) -> Result<String, MergeError>` — diamond-types 3-way
  - `routes::router(state: AppState) -> Router` — axum router with all endpoints

- [ ] **Step 1: Escrever testes unitários de room e events**

Criar `collab-server/tests/server_test.rs`:

```rust
use collab_server::room::{Member, Room};
use collab_server::events::EventStore;
use std::time::Instant;

#[test]
fn room_add_remove_member() {
    let mut room = Room::new("ABCD1234".into());
    room.add_member(Member { user_id: "u1".into(), name: "Ana".into(),
        current_file: None, last_seen: Instant::now() });
    assert_eq!(room.members.len(), 1);
    room.remove_member("u1");
    assert_eq!(room.members.len(), 0);
}

#[test]
fn event_store_cursor() {
    let mut store = EventStore::new();
    store.push(collab_server::events::RoomEvent::MemberJoined {
        user_id: "u1".into(), name: "Ana".into() });
    let events = store.since(0, 0);  // timeout_ms=0: return immediately
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 0);
    let empty = store.since(1, 0);
    assert!(empty.is_empty());
}

#[test]
fn event_store_cursor_overflow() {
    let store = EventStore::new();
    let events = store.since(999999, 0);
    assert!(events.is_empty());
}

// Review Focus: member heartbeat expiry
#[test]
fn member_expired() {
    use std::time::Duration;
    let mut member = Member { user_id: "u1".into(), name: "Ana".into(),
        current_file: None, last_seen: Instant::now() - Duration::from_secs(31) };
    assert!(member.is_expired(30));
}
```

- [ ] **Step 2: Rodar para confirmar FAIL**

```bash
cargo test -p collab-server 2>&1 | head -15
```

Expected: erro de compilação — módulos não existem

- [ ] **Step 3: Adicionar dependências collab-server/Cargo.toml**

```toml
[package]
name = "collab-server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
rand = { version = "0.8", features = ["small_rng"] }
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
diamond-types = "1.0"

[dev-dependencies]
tokio-test = "0.4"
```

- [ ] **Step 4: Implementar room.rs**

Struct `Member { user_id: String, name: String, current_file: Option<String>, last_seen: Instant }`. Método `is_expired(timeout_secs: u64) -> bool`. Struct `FileState { version: u64, text: String, last_user: String }`. Struct `Room { id: String, members: HashMap<String,Member>, files: HashMap<String,FileState>, events: EventStore }`. Métodos: `new(id) -> Room`, `add_member(m)`, `remove_member(uid: &str)`, `update_presence(uid: &str, file: Option<String>)`, `expire_members(timeout_secs: u64)`.

- [ ] **Step 5: Implementar events.rs**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoomEvent {
    FileUpdated { path: String, version: u64, user_id: String },
    MemberJoined { user_id: String, name: String },
    MemberLeft { user_id: String },
    PresenceChanged { user_id: String, file: Option<String> },
    ConflictWarn { path: String, user_a: String, user_b: String },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventEnvelope { pub id: u64, pub event: RoomEvent }

pub struct EventStore { events: Vec<EventEnvelope>, next_id: u64 }
```

`EventStore::push(event) -> u64` incrementa `next_id`, armazena `EventEnvelope { id, event }`, retorna id. `EventStore::since(cursor: u64, timeout_ms: u64) -> Vec<EventEnvelope>` retorna todos os eventos com id >= cursor. Com `timeout_ms=0` retorna imediatamente. Com `timeout_ms>0` usa `std::thread::sleep(Duration::from_millis(min(timeout_ms, 5000)))` antes de retornar eventos — suficiente para o MVP.

Nota: produção usaria `tokio::sync::Notify` para long-poll sem sleep. MVP usa sleep simples.

- [ ] **Step 6: Implementar document.rs**

```rust
pub enum MergeError { Conflict }
pub fn merge3(base: &str, ours: &str, theirs: &str) -> Result<String, MergeError>
```

Usar `diamond_types::list_fuzzer::SimpleOpLog` para aplicar ops de `base→ours` e `base→theirs` e tentar merge. Se divergência não resolve, retornar `Err(MergeError::Conflict)`.

- [ ] **Step 7: Implementar routes.rs e main.rs**

`routes.rs` define `AppState = Arc<Mutex<HashMap<String, Room>>>` e handlers:
- `POST /rooms` → gera room_id (8 chars Alphanumeric), insere Room, retorna `{"room_id":"..."}`
- `GET /rooms/{id}` → retorna `{"room_id", "members":[...]}` ou 404
- `POST /rooms/{id}/join` body `{"name":"..."}` → gera user_id UUID, `add_member`, retorna `{"user_id":"..."}`
- `DELETE /rooms/{id}/members/{uid}` → `remove_member`, push MemberLeft event
- `POST /rooms/{id}/files/{path}` body `{"user_id","version","text"}` → atualiza FileState, detecta conflito (se mesmo arquivo foi salvo por outro usuário na última janela de 5s), push FileUpdated ou ConflictWarn, retorna `{"event_id"}`
- `GET /rooms/{id}/files/{path}` → retorna FileState atual
- `GET /rooms/{id}/members` → retorna lista de membros
- `GET /rooms/{id}/events` query `cursor: u64` → chama `events.since(cursor, 5000)`, retorna `{"events":[...],"next_cursor":N}`

`main.rs` cria AppState, spawna task de limpeza de membros expirados (a cada 30s, `expire_members(30)`), inicia axum em `0.0.0.0:{COLLAB_PORT:-4444}`.

- [ ] **Step 8: Rodar testes**

```bash
cargo test -p collab-server
```

Expected: 4 testes PASS

- [ ] **Step 9: Commit**

```bash
git add collab-server/
git commit -m "feat: collab-server REST API com rooms, events e merge CRDT"
```

---

## Task 3: openidea-collab — Plugin Rust/WASM

> **Pode ser executado após Task 1 concluída; independente de Task 2 para testes unitários.**

**Files:**
- Modify: `openidea-collab/Cargo.toml`
- Create: `openidea-collab/src/lib.rs`
- Create: `openidea-collab/src/client.rs`
- Create: `openidea-collab/src/room.rs`
- Create: `openidea-collab/src/sync.rs`
- Create: `openidea-collab/src/presence.rs`
- Test: `openidea-collab/tests/plugin_test.rs`

**Interfaces:**
- Consumes: `net.fetch` via `openidea_plugin::host::net`, `events` via `Event::FileSaved`, `project.read-file`/`write-file`, `views` tree
- Produces:
  - `room::RoomState { room_id, user_id, name, server_url }` — estado local persistido via `storage`
  - `client::CollabClient::post_file(path, version, text) -> Result<u64>` — POST ao servidor
  - `client::CollabClient::poll_events(cursor: u64) -> Result<PollResult>` — GET events long-poll
  - `presence::PresenceState::to_tree_items() -> Vec<TreeItem>` — para o view panel
  - `sync::SyncState::on_file_saved(path: &str, text: &str) -> Result<()>` — envia arquivo ao servidor

- [ ] **Step 1: Escrever testes unitários de presence e room**

Criar `openidea-collab/tests/plugin_test.rs`:

```rust
use openidea_collab::presence::{PresenceState, PeerPresence};
use openidea_collab::room::RoomState;

#[test]
fn presence_to_tree_items() {
    let mut state = PresenceState::new("u1".into(), "Ana".into());
    state.update_peer("u2".into(), "Bob".into(), Some("src/main.rs".into()));
    let items = state.to_tree_items();
    assert!(items.iter().any(|i| i.label.contains("Bob")));
    assert!(items.iter().any(|i| i.detail.as_deref() == Some("src/main.rs")));
}

#[test]
fn presence_remove_peer() {
    let mut state = PresenceState::new("u1".into(), "Ana".into());
    state.update_peer("u2".into(), "Bob".into(), None);
    state.remove_peer("u2");
    let items = state.to_tree_items();
    assert!(!items.iter().any(|i| i.label.contains("Bob")));
}

#[test]
fn room_state_roundtrip() {
    let rs = RoomState { room_id: "ABCD1234".into(), user_id: "u1".into(),
        name: "Ana".into(), server_url: "http://localhost:4444".into() };
    let json = serde_json::to_string(&rs).unwrap();
    let back: RoomState = serde_json::from_str(&json).unwrap();
    assert_eq!(back.room_id, "ABCD1234");
}
```

- [ ] **Step 2: Rodar para confirmar FAIL**

```bash
cargo test -p openidea-collab 2>&1 | head -15
```

- [ ] **Step 3: Adicionar dependências openidea-collab/Cargo.toml**

```toml
[dependencies]
# SDK — versão exata determinada pelo scaffold (manter a do scaffold)
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"

# O scaffold já inclui openidea_plugin; não adicionar duplicado.
# Verificar que Cargo.toml gerado tem:
# openidea_plugin = { git = "https://github.com/l0ng-ai/openidea", tag = "vX.Y.Z" }
```

Nota: o scaffold gera o Cargo.toml com o SDK. Adicionar apenas `serde`, `serde_json`, `anyhow`.

- [ ] **Step 4: Implementar room.rs**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomState { pub room_id: String, pub user_id: String,
    pub name: String, pub server_url: String }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemberInfo { pub user_id: String, pub name: String,
    pub current_file: Option<String> }
```

- [ ] **Step 5: Implementar presence.rs**

```rust
#[derive(Debug, Clone)]
pub struct PeerPresence { pub user_id: String, pub name: String,
    pub current_file: Option<String> }

pub struct PresenceState { pub self_id: String, pub self_name: String,
    peers: std::collections::HashMap<String, PeerPresence> }
```

Métodos: `new(self_id, self_name)`, `update_peer(uid, name, file)`, `remove_peer(uid)`. `to_tree_items() -> Vec<openidea_plugin::TreeItem>` — um item por peer com `label = name`, `detail = current_file`, `key = user_id`.

- [ ] **Step 6: Implementar client.rs**

`CollabClient { server_url: String }`. Usa `openidea_plugin::host::net::fetch(request)` para HTTP.

```rust
pub struct PollResult {
    pub events: Vec<serde_json::Value>,
    pub next_cursor: u64,
}

impl CollabClient {
    pub fn new(server_url: String) -> Self
    pub fn create_room(&self) -> Result<String, anyhow::Error>  // → room_id
    pub fn join_room(&self, room_id: &str, name: &str) -> Result<String, anyhow::Error>  // → user_id
    pub fn leave_room(&self, room_id: &str, user_id: &str) -> Result<(), anyhow::Error>
    pub fn post_file(&self, room_id: &str, user_id: &str, path: &str, version: u64, text: &str) -> Result<u64, anyhow::Error>  // → event_id
    pub fn poll_events(&self, room_id: &str, cursor: u64) -> Result<PollResult, anyhow::Error>
    pub fn get_members(&self, room_id: &str) -> Result<Vec<MemberInfo>, anyhow::Error>
    pub fn get_file(&self, room_id: &str, path: &str) -> Result<(u64, String), anyhow::Error>  // → (version, text)
}
```

Cada método usa `net::fetch` com timeout adequado (`post_file`: 5000ms, `poll_events`: 8000ms).

- [ ] **Step 7: Implementar sync.rs**

`SyncState { room: Option<RoomState>, presence: PresenceState, file_versions: HashMap<String,u64>, poll_cursor: u64 }`.

Funções:
- `load_from_storage() -> SyncState` — lê `room_state` do `storage::get`, desserializa
- `save_to_storage(&self)` — `storage::set("room_state", json)`
- `on_file_saved(&mut self, ctx: &Invocation, path: &str)` — lê texto via `ctx.project()?.read-file(path)`, POSTs ao servidor, atualiza `file_versions[path]`
- `on_poll_tick(&mut self, ctx: &Invocation) -> Vec<ViewUpdate>` — chama `client.poll_events`, processa cada evento, retorna lista de views para atualizar

- [ ] **Step 8: Implementar lib.rs**

Implementa `Plugin` trait para `CollabPlugin { sync: SyncState, client: CollabClient }`.

Métodos:
- `run_command(id, ctx, arg)`: despacha para `create-room`, `join-room`, `leave-room`. `create-room` chama `client.create_room()`, mostra notification com room_id. `join-room` usa `host::ui::input("Room ID", None)` para pedir ID, chama `client.join_room`, salva estado. `leave-room` chama `client.leave_room`, limpa estado.
- `handle_event(event, ctx)`: `Event::FileSaved(path)` → `sync.on_file_saved(ctx, &path)`. `Event::EditorFocused(path)` → atualiza presença via POST `/rooms/{id}/members/{uid}/presence`.
- `view(id, ctx)` (view-provider): retorna `View { toolbar: vec![], body: Body::Tree(sync.presence.to_tree_items()), status: Some(format!("Room: {}", room_id)) }`.

Scheduled event: na ativação, `events::subscribe("scheduled")` com período de 3s. Cada `Event::Scheduled` → `sync.on_poll_tick(ctx)` → aplica updates + atualiza view.

- [ ] **Step 9: Rodar testes unitários**

```bash
cargo test -p openidea-collab
```

Expected: 3 testes PASS

- [ ] **Step 10: Build WASM**

```bash
cd /run/media/bugson/Work/Projetos/OpenIDEA/.worktrees/openidea-collab/openidea-collab
openidea-plugin build
```

Expected: `dist/plugin.wasm` gerado sem erros

- [ ] **Step 11: Commit**

```bash
git add openidea-collab/
git commit -m "feat: plugin openidea-collab com sync HTTP, presence panel e comandos"
```

---

## Task 4: Integração e Scripts de Dev

> **Sequencial — depende de Tasks 1, 2 e 3 concluídas.**

**Files:**
- Create: `scripts/dev-server.sh`
- Create: `scripts/dev-plugin.sh`
- Create: `collab-server/tests/integration_test.rs`

**Interfaces:**
- Consumes: `collab-server` rodando em porta 4444, plugin buildado

- [ ] **Step 1: Script servidor de dev**

Criar `scripts/dev-server.sh`:
```bash
#!/usr/bin/env bash
set -e
echo "▶ Compilando collab-server..."
cargo build -p collab-server
echo "▶ Iniciando collab-server em :4444..."
RUST_LOG=info ./target/debug/collab-server
```

```bash
chmod +x scripts/dev-server.sh
```

- [ ] **Step 2: Script plugin de dev**

Criar `scripts/dev-plugin.sh`:
```bash
#!/usr/bin/env bash
set -e
echo "▶ Buildando plugin WASM..."
cd openidea-collab
openidea-plugin build
openidea-plugin link
echo "✓ Plugin linkado. Use Settings › Plugins › Reload no OpenIDEA."
```

```bash
chmod +x scripts/dev-plugin.sh
```

- [ ] **Step 3: Teste de integração HTTP**

Criar `collab-server/tests/integration_test.rs`:

```rust
// Requer servidor rodando — executar com: cargo test -p collab-server -- --ignored
#[tokio::test]
#[ignore = "requires running server on :4444"]
async fn create_and_join_room() {
    let client = reqwest::Client::new();
    
    // Criar sala
    let resp = client.post("http://localhost:4444/rooms")
        .send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let room_id = body["room_id"].as_str().unwrap();
    assert_eq!(room_id.len(), 8);
    
    // Entrar na sala
    let resp = client.post(format!("http://localhost:4444/rooms/{room_id}/join"))
        .json(&serde_json::json!({"name": "TestUser"}))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["user_id"].as_str().is_some());
}
```

Adicionar a `collab-server/Cargo.toml`:
```toml
[dev-dependencies]
tokio-test = "0.4"
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 4: cargo test completo**

```bash
cd /run/media/bugson/Work/Projetos/OpenIDEA/.worktrees/openidea-collab
cargo test --workspace 2>&1 | tail -20
```

Expected: todos os testes unitários PASS; integration test ignorado

- [ ] **Step 5: Commit final**

```bash
git add scripts/ collab-server/tests/integration_test.rs collab-server/Cargo.toml
git commit -m "feat: scripts de dev e smoke test de integração"
git tag v0.1.0
```

---

## Parallelization Guide

```
Sequência:
  Task 1 (Bootstrap)
      ├── Task 2 (Server)   ← agente A, pode começar após Task 1
      └── Task 3 (Plugin)   ← agente B, pode começar após Task 1
              └── Task 4 (Integração — agente A ou B, após Tasks 2 e 3)
```

Tasks 2 e 3 podem ser despachadas em paralelo após Task 1 concluída.
Seus testes unitários são independentes; Task 4 requer ambas completas.
