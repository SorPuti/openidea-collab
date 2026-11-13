# Pesquisa e snippets para acelerar a implementação

Data: 09/11/2026. Referência principal: OpenIDEA v0.1.40, commit [`37c2a6048eea89d4f6db645622612358b643862b`](https://github.com/l0ng-ai/openidea/tree/37c2a6048eea89d4f6db645622612358b643862b). Plugin local `0.2.0`; servidor usa `axum 0.8.9`, `tokio 1.54.1` e `reqwest 0.12.28` conforme Cargo.lock. Os exemplos abaixo são auxiliares de planejamento, não implementação pronta para produção. Preserve as duas opções do plano: servidor próprio e conexão gerenciada; editor nativo e personalizado são eixos independentes.

**Evidências:** ver [resultados reproduzíveis e limites](provas/README.md). A prova Yrs 0.28.0 foi posteriormente compilada e testada; manifesto de editor e permissão passaram no check em cópia isolada.

**Níveis:** `TESTADO` = teste isolado indicado executado; `BASEADO EM FONTE` = contrato/exemplo oficial, não testado na IDE; `PSEUDOCÓDIGO` = desenho a implementar. Nenhum exemplo comprova colaboração em duas IDEs ou segurança completa.

## 1. Editor personalizado na API atual

**BASEADO EM FONTE.** [Manifesto real Graphviz](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/official/graphviz/plugin.toml), [API de web editor](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#web-views) e [uso de get/onChange](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/official/graphviz/web-src/src/main.ts). Declaração parcial a integrar ao manifesto, não substituição integral:

```toml
[[contributes.customEditors]]
id = "collab-editor"
title = "Collaborative Editor"
patterns = ["*.collabtest"] # apenas arquivo de teste; decidir padrões reais
priority = "option"          # usuário escolhe Open With
web = "web/index.html"
```

Em uma página empacotada, como base para protótipo do documento:

```js
const editorBridge = window.openidea.document;
const initial = await editorBridge.get();
// initial: { version, text, readOnly, name }; não exibir conteúdo em log.
editorBridge.onChange((state) => {
  // Mudou em outro lugar: atualizar modelo sob controle de concorrência.
  // Reconciliar state.version e state.text; não sobrescrever digitação pendente.
});

async function applyEdit(baseVersion, from, to, insert) {
  // from/to são offsets UTF-16, não posições em bytes Rust.
  // Em erro/stale: parar, obter snapshot e reconciliar; nunca retry cego.
  return editorBridge.edit(baseVersion, [{ from, to, insert }]);
}
```

Verificar chamadas exatas, `readOnly`, eventos próprios versus externos, persistência, atalhos, undo e mudanças de foco em duas janelas. Não bloquear nem recriar conteúdo por snapshot sem mesclagem. O conteúdo local e remoto deve entrar no CRDT por origem definida. `onChange` é documentado para mudanças em outros lugares; não assumir que cobre todos os eventos da interface personalizada. Página é isolada e não faz rede diretamente.

## 2. Canal PTY de webview até processo auxiliar — hipótese concreta

**BASEADO EM FONTE; NÃO TESTADO.** A API [Web frame PTYs 1.3](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#web-frame-ptys-api-13) permite canal assíncrono webview ↔ processo aprovado. O [fixture oficial completo](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/crates/ide-plugin/tests/fixtures/web-pty/web/index.html) usa shell apenas para demonstração; o produto deve usar auxiliar dedicado, aprovado e sem shell. O manifesto necessita capacidade `process:<program>` específica, de alto risco, e o usuário deve concedê-la; o projeto deve ser confiável. O host executa o arquivo aprovado com identidade fixada, sem fallback por PATH.

```js
const sessionId = 'collab-probe-1';
const send = (op, fields = {}) =>
  window.openidea.postMessage(JSON.stringify({ op, sessionId, ...fields }));

window.openidea.onMessage((raw) => {
  let msg;
  try { msg = JSON.parse(raw); } catch { return; }
  if (msg.sessionId !== sessionId) return;
  if (msg.op === 'pty-data') {
    // POC: consumir e validar msg.data, incluindo framing e limites.
    // Confirmar somente após consumo; em erro interromper a sessão.
    send('pty-ack', { sequence: msg.sequence });
  } else if (msg.op === 'pty-error' || msg.op === 'pty-exit') {
    // Suspender sincronização e informar estado; reconectar com segurança.
  }
});
// Exemplo de protocolo, não um executável existente ou instalado:
// send('pty-spawn', { program: 'openidea-collab-helper', args: [] });
// send('pty-input', { data: framedPayload });
// send('pty-kill');
```

Restrições do contrato: até 8 sessões/plugin; entrada pendente até 64 Ki unidades UTF-16; saída até 256 Ki; um chunk não confirmado por vez; sessão encerrada em overflow, desmontagem, recarga, mudança de grants/projeto ou fechamento. Projetar framing limitado e backpressure, lidar com modo terminal, echo, newline, split de mensagens, controle e dados não confiáveis; POC deve verificar se PTY é adequado para dados binários (ex.: enquadramento textual), sem presumir transparência. Não colocar tokens em argumentos/env/log. Não usar shell arbitrário, burlar CSP ou afirmar viabilidade sem teste com rede real.

**Próxima prova:** helper de teste sem rede ecoa mensagens fragmentadas, enquanto duas janelas exercitam mudanças no documento; depois testar recepção de servidor autenticado, desconexão, revogação, idle e latência. Se o host não permitir instalação/concessão do helper, explicitar o passo manual e alternativa upstream.

## 3. Diagnóstico do manifesto de rede

**BASEADO EM FONTE.** [Capacidades e rede do OpenIDEA](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#network-and-secrets); [exemplo oficial de capacidades opcionais](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/official/ai-completion/plugin.toml). O manifesto atual não declara `net:`. Exemplo de sintaxe para **host fixo hipotético**, não endpoint real:

```toml
[capabilities]
required = ["project.read", "events", "storage", "editor.read"]
optional = ["net:collab.example.org"]
```

Não substituir o manifesto atual sem decidir as demais capacidades. HTTPS/HTTP loopback usam `net:<host>` correspondente; HTTP não loopback requer `net.http:<host>` em 1.4. `net:localhost` não autoriza servidor arbitrário. Se um helper faz a rede por fora, documentar o alcance real da autorização `process:` e submeter a revisão; não apresentá-lo como permissão de rede equivalente. Endpoint remoto editável e concessão dinâmica devem ser provados no host, sem recorrer a `net:*` não suportado.

## 4. Health check e bind local para o servidor

**TESTADO ISOLADAMENTE.** O servidor existente registra `POST /rooms`, não `GET /rooms`. Na cópia em `/tmp/openidea-collab-snippet-check`, acrescentamos rota a `src/routes.rs`, mudamos bind para loopback em `src/main.rs` e executamos o teste abaixo com dependências locais. O teste de `healthz` passou (1/1). Não alteramos o servidor do projeto. O teste comprova apenas a rota HTTP na cópia; não confirma execução de serviço, autenticação ou sincronização.

Em `routes.rs` após `Router::new()`:

```rust
.route("/healthz", get(|| async { StatusCode::NO_CONTENT }))
```

No `main.rs`, para o modo local de desenvolvimento:

```rust
let addr = SocketAddr::from(([127, 0, 0, 1], port));
```

Teste isolado, com `reqwest` já em dev-dependencies:

```rust
use std::{collections::HashMap, sync::{Arc, Mutex}};
use collab_server::routes::router;

#[tokio::test]
async fn health_is_read_only() {
    let state = Arc::new(Mutex::new(HashMap::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router(state)).await.unwrap()
    });
    let response = reqwest::get(format!("http://{address}/healthz")).await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    handle.abort();
}
```

Para produção, endereço configurável com validação e mudança intencional para bind externo deve vir acompanhado de TLS/autorização e política operacional. `/healthz` não deve divulgar conteúdo nem ser confundido com readiness autenticada ou teste funcional. [Axum 0.8.9 Router](https://docs.rs/axum/0.8.9/axum/struct.Router.html) e [StatusCode 204](https://docs.rs/http/1/http/status/struct.StatusCode.html).

## 5. CRDT e índices Unicode

**TESTADO ISOLADAMENTE, NÃO INTEGRADO AO PLUGIN.** A prova com Yrs 0.28.0 confirmou UTF-16 e convergência em memória; consulte [resultados](provas/README.md). [Yrs Text e opções de offset](https://docs.rs/yrs/0.28.0/yrs/types/text/trait.Text.html); [protocolo de updates do Yjs](https://docs.yjs.dev/api/document-updates). `yrs` ainda não é dependência do projeto: fixar versão após prova de WASM, licença e compatibilidade. O editor web documenta offsets UTF-16; Yrs por padrão usa bytes, então decidir explicitamente como converter ou configurar.

```rust
// Exemplo para uma prova com API Yrs 0.28.0; não assume dependência instalada.
use yrs::{Doc, GetString, OffsetKind, Options, Text, Transact};

let doc = Doc::with_options(Options {
    offset_kind: OffsetKind::Utf16,
    ..Default::default()
});
let text = doc.get_or_insert_text("content");
{
    let mut txn = doc.transact_mut();
    text.push(&mut txn, "A😀B");
    assert_eq!(text.len(&txn), 4); // A=1, 😀=2, B=1 em UTF-16
}
assert_eq!(text.get_string(&doc.transact()), "A😀B");
```

Testar par substituto, combinações, CRLF, posições inválidas, traduções de offsets e concorrência. Reutilizar identidade de cliente de forma incorreta pode corromper sincronização; seguir semântica da biblioteca. Cursores precisam de posições estáveis ou transformação sob edição concorrente; inteiros puros não bastam. CRDT não oferece por si só autenticação, persistência ou transporte.

## 6. Modelo de protocolo — pseudocódigo

**PSEUDOCÓDIGO, NÃO É CONTRATO IMPLEMENTADO.** O servidor deve autenticar antes de associar sessão, e validar autorização por documento em cada ação. Não colocar convites, conteúdo de arquivos ou tokens em logs. IDs devem ser opacos; identidade do cliente não pode ser confiada somente por campo JSON.

```text
cliente  → servidor: HELLO { protocol_version, client_version }
cliente  → servidor: AUTH { credencial efêmera em canal protegido }
servidor → cliente: AUTH_OK { session_id, role, expires_at }
cliente  → servidor: OPEN { document_id, state_vector }
servidor → cliente: SYNC { update_incremental_ou_snapshot }
ambos    ↔ ambos:    UPDATE { document_id, update_id, payload_limitado }
ambos    ↔ ambos:    PRESENCE { estado_temporário, revisão }
servidor → cliente: ACK { update_id, durability_level }
```

Definir ACK somente após o nível prometido de persistência, negociação/versão, limites, revogação, replay, backpressure, idempotência, retenção, migração e comportamento de falhas. TLS protege o transporte, não é E2EE. Para WebSocket, a [documentação Axum 0.8.9](https://docs.rs/axum/0.8.9/axum/extract/ws/index.html) exige feature `ws`; upgrade sozinho não implementa autenticação, autorização ou política de origem. Não incluir endpoint WS inseguro como código pronto.

## 7. Esqueleto do instalador externo — pseudocódigo

**PSEUDOCÓDIGO.** Complementa [o guia de instalação](instalacao-customizada.md), não substitui o `install.sh` atual. Flags são propostas.

```text
parse_args()
config = collect_explicit_choices(mode, editor, version, endpoint, service)
preflight = inspect_os_ide_permissions_ports_and_existing_install(config)
plan = build_plan(preflight, config)
show(plan without secrets)
if dry_run: return success_without_changes
verify_or_build_staged_artifacts(plan)  # versões e procedência fixadas
if mode == managed or mode == remote:
    install_plugin_only(plan)
else if mode == local:
    install_local_server_loopback_only(plan)
    if service_explicitly_selected: configure_user_service(plan)
request_host_capabilities_via_official_flow()
verify_package_host_permission_endpoint_and_optional_service_separately()
report_installed_pending_ide_or_failed_with_recovery()
```

Atualização: armazenar config anterior e estado de serviço; preparar binário antes de interromper serviço; substituir e verificar; restaurar em falha. Desinstalação só dos arquivos e serviços comprovadamente criados pelo instalador; dados preservados por padrão. Criar testes em pastas temporárias com mocks para cada ramo, caminhos com espaços, falha em cada etapa e recusa. Endpoint remoto e operador gerenciado ainda precisam ser definidos; não instalar software ou serviço em background sem consentimento necessário.

## 8. Upstream e registro: pontos de partida

- WIT de eventos: [events.wit v0.1.40](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/wit/events.wit); documento/aplicação versionada: [workspace.wit](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/wit/workspace.wit). O desenho de eventos de digitação é proposta, não interface existente.
- Marketplace: [regras oficiais](https://github.com/l0ng-ai/openidea-plugins#publishing-a-plugin); o [PR #102](https://github.com/l0ng-ai/openidea-plugins/pull/102) trata da listagem, não de novas APIs.
- CI: [orientação GitHub sobre permissões mínimas e fixação de actions por SHA](https://docs.github.com/en/actions/reference/security/secure-use). Fixar somente hashes obtidos e auditados; não copiar SHA imaginado.

## Próximos experimentos rastreados

Tarefas 19–20, 28–35 de [todo.md](todo.md): primeiro POC do editor e PTY, depois servidor de teste e instalador; testar duas IDEs, latência, segurança e comportamento de falhas. Registrar versões, comandos, resultado e limitações. Atualização desta pesquisa não faz deploy nem instala componentes.
