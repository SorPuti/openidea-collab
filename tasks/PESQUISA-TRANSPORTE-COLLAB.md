# Pesquisa: colaboração, transporte e API OpenIDEA

Data: 2026-11-10. Escopo: versão experimental local, sem alterar o sistema em uso. Os tempos observados pelo usuário em duas máquinas não foram instrumentados; os gargalos abaixo são propriedades do código ou hipóteses, não uma atribuição comprovada da latência.

## Fluxo implementado e evidências

```text
Editor web → document.edit → mensagem PTY → auxiliar Yrs
             → GET snapshot → POST de cada alteração → GET snapshot
             → servidor ← long poll de quem recebe ← auxiliar ← PTY ← editor
```

* O editor faz um diff de strings usando `Array.from`, aplica no documento via `openidea.document.edit`, passa eventos JSON pela PTY e repõe o texto integral no textarea: [`editor.js`](../openidea-collab/web/collab-editor/editor.js). A complexidade e a frequência precisam ser medidas com arquivos e edições reais.
* `Client::sync` faz dois GETs de snapshot por rodada e um POST por atualização pendente em sequência. Quando a notificação de alteração chega, `wait_for_change` chama `sync`, mesmo que não haja edição local: [`collab-helper/src/lib.rs`](../collab-helper/src/lib.rs#L198). Isso introduz múltiplas idas e voltas e transferência de snapshots no fluxo quente.
* O long poll não espera sempre cinco segundos: acorda quando `changes.changed()` é sinalizado, com timeout máximo de cinco segundos, e revalida o acesso após esperar: [`realtime.rs`](../collab-server/src/realtime.rs#L686). No erro, o auxiliar agenda nova tentativa após um segundo: [`main.rs`](../collab-helper/src/main.rs#L411).
* O `tokio::select!` disputa entrada da PTY com a future de rede: ao chegar entrada primeiro, a outra future é descartada e recriada na próxima iteração ([`main.rs`](../collab-helper/src/main.rs#L361)). Potencial postergação/cancelamento da sincronização sob digitação contínua é hipótese prioritária para reprodução controlada; não foi comprovada como causa da queixa real.
* Há suporte a alterações incrementais e vetor de estado no CRDT Yrs já usado pelo projeto ([`collab-core/src/lib.rs`](../collab-core/src/lib.rs)). Isso não significa que o transporte atual usa apenas deltas no caminho quente. A especificação/documentação do [Yjs sobre atualizações](https://docs.yjs.dev/api/document-updates) descreve deltas, vetores de estado, idempotência e operações de merge. Validar a compatibilidade da versão/formato Yrs escolhida com o protocolo efetivamente negociado.

## O que a OpenIDEA fornece

Na revisão local `37c2a6048eea89d4f6db645622612358b643862b`, o [guia oficial dos plugins](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md) documenta `customEditors`, `document.edit(version, edits)` com offsets UTF-16 e rejeição de versão antiga, `document.onChange` e PTY com programa aprovado. O iframe do editor web é isolado e não possui acesso de rede direto; o processo auxiliar é a rota demonstrada aqui. A permissão `process:<program>` é de alto risco e exige confiança/aprovação. O mesmo guia lista eventos de abrir/salvar/foco e mudanças de arquivos em disco; isso, por si só, não demonstra uma API pública de observação de cada edição em todos os editores nativos. O editor atual está limitado ao padrão `*.collabtest`; suporte ao editor nativo geral precisa de prova em API pública ou contribuição upstream acordada.

## Opção proposta

```text
Editor/bridge ↔ auxiliar local ↔ WSS autenticado ↔ relay ↔ WSS ↔ auxiliar ↔ Editor/bridge
                                  ↑ deltas CRDT + confirmações ↑
```

* Manter a renderização local imediata e operações pequenas como atualizações CRDT; agregar brevemente em lote limitado para reduzir frames sem aguardar uma pausa longa na digitação.
* Usar um WebSocket persistente por sessão, envio e recebimento independentes, filas limitadas com backpressure, sequência/IDs e ACK de persistência, reenvio idempotente; autenticar o upgrade em cabeçalho e autorizar cada operação e revogação. Não colocar tokens em URL/logs. Não interpretar ACK de rede como persistência antes da confirmação do servidor.
* Na reconexão, negociar vetor de estado e enviar somente diferenças necessárias; limitar memória, tamanho de frame e fila, e expor alterações pendentes/erros. Adotar snapshot só para carga inicial, recuperação ou compaction controlada. Preservar CRDT, permissões e rejeição de versões inválidas; resolver limites de armazenamento, exclusão e política para edições locais.
* Não bloquear a entrada do usuário em operação longa de rede: controlar cancelamento e estado com ator de documento ou filas separadas; comprovar ausência de perda/reordenação com testes.
* Manter long polling como opção negociada/fallback até comprovar a alternativa. WebSocket reduz pedidos e permite comunicação bidirecional persistente, porém não garante menor latência por si só: caminhos de bridge, CRDT, persistência, proxy e rede também contam.
* [Axum documenta WebSocket e leitura/escrita concorrentes](https://docs.rs/axum/0.8.9/axum/extract/ws/index.html) com recurso `ws`. A [Cloudflare informa suporte a WebSocket em Tunnel](https://developers.cloudflare.com/cloudflare-one/faq/cloudflare-tunnels-faq/); testar o túnel e infraestrutura reais com reconnect/idle, pois compatibilidade geral não prova esse fluxo específico. O túnel rápido atual continua experimental e seu endereço muda ao reiniciar.

## Plano de prova com RAM limitada

1. Instrumentar horários monotônicos, IDs aleatórios sem conteúdo e métricas por trecho: entrada, bridge, fila, envio, ACK, entrega e aplicação. Medir RTT/bytes, cancelamentos, fila pendente e recursos de auxiliar/servidor; não registrar texto nem tokens.
2. Reproduzir digitação contínua em dois clientes e emular rede com atraso/perda controlados; medir p50/p95/p99 da edição local à remota, volume, tamanho de fila e recuperação após reconexão. Separar localhost, túnel e redes distantes. `tc/netem` apenas em ambiente isolado, nunca alterar rede da máquina de trabalho.
3. Comparar baseline atual, long polling otimizado e WebSocket com mesma carga, hardware e regras de persistência. Definir metas antes da avaliação e informar números medidos, sem prometer latência hipotética.
4. Validar dois editores, várias edições simultâneas, Unicode UTF-16, perda/repetição de mensagens, revogação durante conexão, proxy que fecha socket, recuperação de fila e integridade de documentos.
5. Sincronização de projetos inteiros é uma camada separada: bootstrap seguro, exclusões e consentimento, operações de arquivos e conflitos. Não está implementada pelo WebSocket.

## Amostra de rede observada em 2026-11-10

Dez requisições GET sequenciais ao endpoint `/health`, com o mesmo processo `curl` e reaproveitamento de conexão quando disponível, originadas nesta máquina: localhost mediana 0,0 ms (arredondada a 0,1 ms), máximo 3,5 ms; túnel rápido mediana 53,9 ms, máximo 982,8 ms. É uma amostra pequena, inclui potencial custo de primeira conexão, não mede a outra cidade nem edição-a-renderização, e não comprova a causa da lentidão. Nenhum conteúdo de documento ou credencial foi registrado.

## Revisão do marketplace e licença

A [revisão do mantenedor no PR 102](https://github.com/l0ng-ai/openidea-plugins/pull/102#issuecomment-6756170352) se refere ao pacote histórico 0.1.0, não à arquitetura experimental posterior. Verificado no Git: a tag v0.1.0 não contém `LICENSE` e o manifesto dela declara `Apache-2.0` e ID `ai.l0ng.openidea-collab`; o índice aponta para o pacote ID `sorputi.openidea-collab`. O HEAD atual contém licença MIT e manifesto MIT; foi acrescentada a declaração MIT ao Cargo.toml do servidor. A descrição do PR foi corrigida, mas o pacote e a tag antigos continuam divergentes: não alterar a tag histórica. Preparar nova tag e pacote assinado construído desse commit somente após cumprir o restante da revisão.

| Pedido do mantenedor | Encaminhamento e situação |
|---|---|
| Escritas remotas restritas, exclusão de VCS/IDE, hooks e symlinks | Necessário no recurso futuro de projeto inteiro. Protótipo de extração em staging não é aplicação de arquivos no projeto. Pendente de integração e testes adversariais. |
| Vincular sessão a projeto e confirmar troca | A prova atual é por arquivo `.collabtest`, não demonstrou fluxo completo de projeto. Pendente. |
| Ignorar segredos e regras de exclusão | Protótipo de snapshot tem filtros e gitignore; exige prévia, consentimento e auditoria. Pendente no produto. |
| Credenciais por participante, autorização, loopback e TLS | A versão experimental tem convites e credenciais por participante, verificação por rota, bind loopback e HTTPS remoto; falta auditoria independente e teste de revogação no transporte proposto. O serviço/túnel atual é experimental. |
| Não perder alterações, retry, conflitos | CRDT, fila em memória e recuperação opcional são implementados na prévia; não demonstram durabilidade completa, reconexão em todos os casos nem resolução para projeto inteiro. Pendente comprovar sem perda. |
| Agendamento e rotas Axum do pacote antigo | A prévia usa outro ciclo e rotas distintas; não comprova correção da versão 0.1.0 indexada. Testar a versão efetivamente submetida. |
| README, licença, ID, pacote reproduzível e hash | Descrição do PR corrigida e metadata MIT do servidor ajustada localmente. Falta release nova, avisos de terceiros, assinatura, verificar tag/ID/hash e atualizar a entrada do marketplace. |

Nenhuma correção do registro substitui a aprovação do mantenedor. O arquivo `tasks/` é ignorado por Git neste checkout: esta pesquisa é local até ser explicitamente incluída em um artefato de revisão.
