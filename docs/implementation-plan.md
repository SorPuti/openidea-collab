# OpenIDEA Collab: plano para colaboração nativa

O domínio `collab.synthosia.com` respondeu com HTTP 204 em `/healthz`; `/v1/info` anuncia protocolo 1 e o endereço WSS. Isso comprova disponibilidade HTTP, não sessão autenticada nem edição entre dois IDEs.

## Estado

- Plugin v0.3.0 prepara sessões e credenciais, ainda não transmite documentos.
- Relay 0.2.0 encaminha conteúdo binário por WSS; conteúdo é transitório.
- Núcleo Yrs/checkpoint existe, mas não está ligado ao editor. Política lexical não basta para escrita segura no filesystem.
- Protótipo inbox está em main do Collab, quatro testes locais; não conectado ao socket/runtime OpenIDEA.
- CI remoto foi bloqueado antes dos passos pela conta GitHub. Integração recusou criar issue upstream (403); mantenedores ainda não aprovaram APIs.

## PR A — transporte WSS nativo no OpenIDEA

Título: `feat(plugins): add bounded background WebSocket capability`.
Arquivos candidatos: `wit/io.wit` em nova versão draft; `crates/ide-plugin/src/runtime.rs`, `wit_host.rs`; cópias e wrappers em `sdk/rust/openidea-plugin`; novo módulo no `crates/ide-host/src/plugins/`. Confirmar paths/padrões na implementação.

Host valida WSS/TLS/origem concedida/subprotocolo/limites; socket não é exposto ao WASM. Segredos usam secret store, nunca query/logs. Worker async, filas limitadas em bytes e mensagens, timeouts/cancelamento. Overflow fecha conexão e notifica erro. Wakeups coalescidos acordam ator sem polling. Handle pertence ao plugin e projeto, encerra em revoke/unload/fechamento; reclaim não perde assinatura. Preservar APIs WIT lançadas.

Aceite: testes mock e WSS real; TLS inválido, origem recusada, segredo, timeout, frame excessivo, overflow, cancelamento, revoke, unload, fechamento de projeto e reclaim; sem deadlock ou log de credencial; compatibilidade de mundos existentes; fmt/clippy/testes/build relevantes.

## PR B — documentos nativos versionados no OpenIDEA

Título: `feat(plugins): expose scoped versioned document subscriptions`.
Arquivos candidatos: `crates/ide-plugin/src/types.rs`, WIT/conversores, `crates/ide-host/src/plugins/api.rs`, adapters/editor em `src/features/plugins/`; regenerar bindings por `pnpm gen:bindings`. Resolver issue #466; não duplicar a discussão existente.

Somente documentos explicitamente selecionados no projeto. Snapshot/version, deltas UTF-16 e conflito expected-version; observar documento desfocado. Aplicação remota sai do histórico Undo; undo/redo local gera eventos. Sem acesso arbitrário a paths ou escrita em arquivo. Qualquer futura operação em arquivo requer grant e no-follow host-side.

Aceite: UTF-16/emoji, editor focado/desfocado, projetos múltiplos, versão obsoleta, ranges inválidos, edits simultâneos, Undo/Redo, fechamento/unload; testes Rust/TS e build da IDE.

## PR C — sincronização plugin/CRDT

Título: `feat(plugin): sync explicitly shared editor documents`.
Arquivos: `src/plugin/src/{client,state}.rs`, `src/core/src/{document,policy,transfer}.rs`, protocolo, manifest/locales/testes. Depende de A/B em runtime disponível. Ligar Yrs a documentos e transporte; checkpoint/outbox antes de ACK de acordo com durabilidade real do host; recuperar e reenviar após queda. Troca de projeto exige consentimento novo.

Aceite: dois clientes, merge simultâneo, ACK/retry/409, crash/restart, credencial inválida/revogada, duplicata, limites/backpressure. Evidenciar que `.env`, chaves, `.git`, `.openidea`, symlinks e docs não compartilhados não são lidos/enviados. Reportar persistência e perfil da hot path.

## PR D — segurança e operação do relay

Título: `security(relay): gate public room creation and document deployment`.
Definir antiabuso antes de criação irrestrita; avaliar privacidade de rate por IP e não usar fingerprint oculto. Teste independente, rotação/revogação, limites proxy, logs sem payload e restore de metadata. Validar autorização por papel/documento em WSS.

Aceite: DNS/TLS/WSS autenticado, quotas/abuso/carga/restart/revogação, RSS relay+proxy, logs sanitizados, política retenção e runbook. Health endpoint sozinho não comprova isso.

## PR E — marketplace

Título: `release: prepare collab plugin v0.4.0` após A–D. Atualizar README/CHANGELOG/manifest, URL/SHA256 gerados exatamente do tag; incluir NOTICE/licenças/trust model; CLI check, build reproduzível, assinatura, instalar/upgrade/uninstall. Enviar tag e pacote novos para revisão, nunca artefato de commit diferente.

## Segurança, privacidade e provas

Threat model inclui operador do relay, membro malicioso, convite vazado, plugin comprometido, projeto trocado, symlink, queda/restart e fila esgotada. TLS protege cliente-relay, mas operador pode observar conteúdo encaminhado enquanto não houver E2E; documentar claramente e não afirmar criptografia ponta a ponta. Credenciais individuais/revogáveis; identidade deriva do token autenticado, nunca de user_id arbitrário. Filesystem protegido pelo host.

PRs anexam matriz de autorização positiva/negativa, logs redigidos, testes symlink/dirs de controle/projeto trocado, scan de segredos, fuzz de parser/limites, SBOM, builds/testes reproduzíveis, teste de dois IDEs, recuperação e revisão independente pré-marketplace.

## Licenças e marcas

Repo e quatro crates declaram MIT; preservar aviso/copyright. SDK Rust OpenIDEA é Apache-2.0; NOTICE atual preserva atribuição e deixa claro que não relicencia SDK. Yrs é indicado Apache-2.0: confirmar o release exato e incluir licença. Cargo.lock não substitui auditoria: gerar SBOM CycloneDX/SPDX e revisar licenças transitivas WASM/native; arquivar attribution. Não copiar código/assets do IDE sem preservar NOTICE/direitos. Nomes OpenIDEA/IDEA identificam compatibilidade; manter não afiliação e não incluir logos JetBrains/VS Code sem autorização. Pacote inclui LICENSE, NOTICE, SBOM e textos aplicáveis.

README descreve instalação, conteúdo transmitido, relay/operador, limites, confiança, vulnerabilidades e remoção. `docs/security.md` descreve fluxo/retenção/ameaças; `docs/operations.md` deploy/upgrade/rollback/TLS/quotas/restore/monitoramento/incidente; SECURITY.md canal/processo; CHANGELOG comportamento e limitações. PR liga issue e declara riscos, testes e limites com evidências sem tokens/conteúdo privado.

## Dependências externas para considerar concluído

1. Mantenedores aceitam/planejam APIs WSS e documento; upstream não oferece hoje o runtime público necessário.
2. Proprietário resolve bloqueio GitHub Actions; teste local não é check CI verde.
3. Operador prova WSS autenticado, antiabuso e monitoramento de produção.
4. Marketplace aprova pacote novo após as provas.

Realtime fica desabilitado até isso. Não declarar “seguro”, “E2E” ou “sem perda” sem evidências correspondentes.
