# Arquitetura comum, ownership e decisões de aprovação

Data: 09/11/2026. **Proposta**, não código de produção. Complementa [pesquisa e snippets](snippets-referencia.md), [evidências executáveis](provas/README.md) e [instalação customizada](instalacao-customizada.md). OpenIDEA de referência: v0.1.40, API 1.4. `managed` e `self_hosted` identificam quem opera o serviço, não versões distintas do protocolo. Editor nativo e personalizado são outro eixo.

## Diagrama de responsabilidades

```text
            CAMADA DO USUÁRIO                  SERVIÇO ESCOLHIDO
┌────────────────────────────────────┐     ┌──────────────────────────┐
│ OpenIDEA (projeto l0ng-ai)         │     │ Gerenciado pelo projeto  │
│ ├ API pública atual e futura       │     │ OU instalado pelo usuário│
│ └ Permissões concedidas na IDE     │     │                          │
│              ↕                     │     │ MESMO contrato de sessão │
│ ┌──────────────────────────────┐   │     │ autenticação, ACL, sync  │
│ │ Adaptador de editor         │   │     │ snapshot, persistência  │
│ │ native | custom             │   │     │ limites + observabilidade│
│ └──────────────┬───────────────┘   │     └────────────▲─────────────┘
│                ↕                   │                  │
│ ┌──────────────────────────────┐   │    TLS/rede   ┌──┴────────────┐
│ │ Núcleo comum Collab          │◄──┼──────────────►│ Transport     │
│ │ sessão, sync, CRDT, presença │   │               │ host | helper │
│ │ protocolo, erros, recovery   │   │               └───────────────┘
│ └──────────────────────────────┘   │
└────────────────────────────────────┘
```

`host | helper` é uma decisão técnica sujeita à prova de viabilidade e concessão de capacidades. O editor personalizado pode oferecer um caminho sem mudança no OpenIDEA, mas PTY e instalação do helper ainda precisam de teste real. O editor nativo requer as APIs correspondentes; configurar `native` não cria APIs no host. Mesma API de transporte não torna viáveis WebSocket ou entrega contínua se o host não as oferece.

## Matriz de ownership e aprovação

| Componente ou decisão | Onde editar / responsável | Precisa de aprovação externa? | Condição prática |
|---|---|---|---|
| Núcleo de sessão, CRDT, presença, protocolo, adaptadores, UI, manifesto, instalador, servidor | `SorPuti/openidea-collab`: mantenedores do próprio projeto | Não para editar ou distribuir no próprio repositório; usuários ainda aprovam permissões do host | Sujeito a licenças, segurança e compatibilidade; CI e revisão internos recomendados |
| API existente de custom editor e PTY | Uso no plugin próprio; implementação da API é do OpenIDEA | Não é necessário PR no OpenIDEA para **usar** API existente; host exige grants e projeto confiável | Não comprovado como solução integral; política do registro e consentimento do usuário continuam aplicáveis |
| Evento de digitação/seleção, aplicar por documento não focado, novas APIs WIT/SDK/host | `l0ng-ai/openidea` | Sim, para integrar no OpenIDEA oficial: revisão/merge e lançamento dos mantenedores | Fork próprio é alternativa separada; não implica suporte oficial |
| Endpoint gerenciado, credenciais, DNS, TLS, operação, retenção e custos | Operador de serviço a definir | Não requer PR no OpenIDEA por si só; requer autorização para recursos e concessão do usuário | Ainda não provisionado; endpoint e política são decisões abertas |
| Servidor self-hosted | Administrador do servidor | Sem PR no OpenIDEA por si só; consentimento de quem administra o host e do usuário | URLs arbitrárias podem não se enquadrar no grant do plugin; fluxo deve ser provado |
| Listagem de qualquer pacote no marketplace | `l0ng-ai/openidea-plugins`, [PR #102](https://github.com/l0ng-ai/openidea-plugins/pull/102) | Sim: checks e merge de mantenedor do registro | Pacote assinado, hash, fonte e capacidades conforme [regras](https://github.com/l0ng-ai/openidea-plugins#rules); aprovação de listagem não prova produto |
| Permissão `process:<helper>`, rede, editor e dados | Usuário no OpenIDEA | Consentimento e validação do host, mesmo sem PR upstream | Processo aprovado é capacidade de risco elevado; não contornar sandbox |
| Publicação de release, alteração de PR, deploy, serviços ou credenciais | Dono autorizado do repositório/infra | Requer instrução/autoridade para a ação concreta | Planejamento e testes locais não são deploy nem aprovação |

## Interfaces propostas (pseudocódigo)

```text
EditorAdapter:
  capabilities() -> { observe_changes, apply_versioned, presence, undo_semantics }
  open(document_id) -> snapshot { editor_version, text, encoding }
  changes() -> stream { document_id, editor_version, range_utf16, text, origin }
  apply(document_id, expected_version, edits, origin) -> Applied(version) | Stale | Denied
  close(document_id)

Transport:
  connect(endpoint, credentials, protocol_range) -> negotiated_session
  send(frame) -> local_enqueue_result
  receive() -> async bounded stream<frame>
  close(reason)

SyncEngine:
  local_change(document_id, change) -> CRDT update
  remote_update(document_id, update) -> editor transaction
  reconnect(state_vector) -> missing_updates / snapshot
  document_status() -> syncing | synced | offline_pending | conflict | error

AuthProvider:
  acquire_and_refresh() -> credential (secret handling, no logging)
  revoke_or_forget() -> result
```

Contratos reais devem especificar ownership/lifetimes, encerramento, cancelamento, versões, erro, acesso, isolamento de salas, tamanho, backpressure, reinício e relação durável de ACK. `receive()` é interface desejada; não demonstra que qualquer adaptador de host existente forneça streaming. `sendLocal` e `applyRemote` do teste JS são apenas simulação de seleção e **não** implementam protocolo, CRDT, segurança nem persistência.

### Configuração proposta: discriminada e sem segredos

```toml
# ESQUEMA PROPOSTO. Não é suportado pelo plugin atual.
version = 1
editor = "custom"                 # ou "native"
transport = "helper"              # ou "host", após prova

[connection]
kind = "managed"                  # ou "self_hosted"
endpoint = "https://collab.example.org" # placeholder não operacional

[security]
credential_ref = "os-secret:collab-account" # referência, nunca token em TOML
```

O endpoint gerenciado deve vir de configuração verificável e operador definido; self-hosted pode informar URL própria **somente se** a política de capabilities do host permitir. O teste de manifesto demonstrou somente um host específico, não concessão arbitrária. Versão da API da IDE, empacotamento de helper por plataforma, grants, CSP, diferenças de UI e protocolo negociado podem exigir builds/versões diferentes; configuração sozinha é suficiente **após** os adaptadores e suas dependências existirem e forem suportados. Não trocar servidor silenciosamente ou enviar dados para endpoint não consentido.

## Matriz de combinação e bloqueios

| Conexão | Editor | Dependências além da configuração | Situação |
|---|---|---|---|
| Gerenciada | Personalizado | Serviço operado, endpoint e grant, helper/canal contínuo comprovado | Protótipo com mocks; E2E não comprovado |
| Própria | Personalizado | Servidor seguro e acessível, grant para endpoint, helper/canal comprovado | Protótipo com mocks; E2E não comprovado |
| Gerenciada | Nativo | Serviço operado e APIs nativas necessárias lançadas ou versão compatível comprovada | Espera validação/upstream |
| Própria | Nativo | Servidor seguro e APIs nativas necessárias lançadas ou versão compatível comprovada | Espera validação/upstream |

Todos usam o mesmo motor e protocolo **se** as versões forem compatíveis; o serviço gerenciado não está hospedado hoje. Desenvolvimento self-hosted local é um subtipo de servidor próprio, sem deixar de exigir bind seguro.

## Pauta de projeto e PRs

1. Em `openidea-collab`, implementar abstrações versionadas em módulos próprios com testes de contrato comuns; separar adapters de UI/host e de transporte. Decidir localização do CRDT após prova WASM/helper. Não tratar credenciais nem caminho absoluto como identidade de documento.
2. Prova custom editor + helper/PTY com a API atual; reportar resultado e limites, sem rodar shell arbitrário ou ignorar os limites do host. A seguir prova de rede com dois dispositivos e autenticação.
3. Manter discussão e PRs para editor nativo no repositório upstream, baseados em casos genéricos e testes. Não esperar merge para desenvolver partes independentes, mas não anunciar rota native como disponível sem API.
4. Publicar pacote e solicitar listagem somente com testes e justificativa; o PR de registro e PRs da IDE têm revisões independentes.
5. Critério comum: matriz E2E com as combinações suportadas, testes de autorização/negação, concorrência, Unicode, conflitos, reconexão, atualização e migração de endpoint sem perda. Métricas medidas em vez de promessas.
