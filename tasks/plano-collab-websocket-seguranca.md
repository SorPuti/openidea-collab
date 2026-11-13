# Plano de execução: Collab seguro, instalável e rápido

Data: 2026-11-10. Estado: plano; nenhum dos critérios pendentes abaixo é uma funcionalidade entregue por este documento. Complementa [pesquisa e evidências](PESQUISA-TRANSPORTE-COLLAB.md), [plano anterior](plan.md), [ownership](arquitetura-ownership.md) e [projeto inteiro](../PROJECT_SYNC.md). Manter trabalho existente e releases históricas. O pedido do usuário é instalável pelo OpenIDEA, colaboração em tempo real, sincronização de projeto, servidor gerenciado ou próprio, baixo uso de RAM e segurança conforme [PR #102](https://github.com/l0ng-ai/openidea-plugins/pull/102#issuecomment-6756170352).

## Estado comprovado

- Funciona em duas máquinas com editor customizado `.collabtest`, auxiliar, servidor e túnel manual. A distribuição preview.3 é não assinada, registra vínculo de desenvolvimento e exige aprovação do auxiliar. **Não é instalação pelo marketplace.**
- Projeto inteiro, editor nativo para qualquer arquivo, WebSocket, serviço gerenciado, instalação automática do túnel e recuperação automática da fila não foram entregues.
- A licença atual é MIT; a tag antiga não é coerente com o pacote. O PR #102 continua apontando à versão antiga. Não alterar a tag antiga nem declarar resolvidos pontos ainda não demonstrados.
- O túnel rápido é temporário e público para quem souber a URL; o servidor não foi aprovado para produção. Não usar como serviço permanente.

## Direção atualizada: sem instalação extra na máquina

A prioridade do usuário é usar serviço hospedado com domínio próprio e colaboradores instalando somente o plugin, sem Cloudflare/ngrok ou outro executável local. Ver [pesquisa de provedores e bloqueio do host](hospedagem-gratuita-sem-agente.md). A arquitetura com auxiliar e túneis, descrita abaixo, permanece apenas alternativa compatível/experimental; não confundir com o objetivo preferido. A documentação da IDE consultada não prova WebSocket de longa duração para plugins sem processo externo: investigar e, se necessário, propor PR upstream e aguardar suporte. Nenhum serviço gratuito/provedor/latência é garantido antes dos testes.

## Arquitetura alvo e opções

```text
                [ USUÁRIO: instala pelo OpenIDEA + concede permissões ]
                                       │
                         [ PLUGIN / UI DE COLABORAÇÃO ]
                            │ sessão vinculada ao projeto
                      adaptador de editor (custom | nativo*)
                            │ versão, offsets, eventos
                    [ AUXILIAR APROVADO PELO USUÁRIO ]
                    CRDT + fila local + compressão limitada
                            │ WSS autenticado / fallback
             ┌──────────────┴────────────────┐
             ▼                               ▼
 [ SERVIÇO GERENCIADO ]             [ SERVIDOR PRÓPRIO ]
 operador, TLS, quotas, dados        local ou remoto, TLS
             └──────────────┬────────────────┘
                            ▼
                [ OUTRA MÁQUINA / OUTRO EDITOR ]

* Editor nativo depende de prova das APIs públicas; se insuficientes,
  propor API genérica à OpenIDEA. A seleção em configuração não cria APIs.
```

Os dois modos usam o mesmo contrato versionado e sem migração silenciosa. Nenhum serviço gerenciado está provisionado; seu operador, custo, retenção, autenticação e endereço precisam ser definidos antes do lançamento. Servidor próprio mantém loopback por padrão; acesso remoto exige TLS e autorização. Túnel opcional é alternativa de teste acionada explicitamente, com instalação verificável, estado, tempo de vida e botão de parar; não é necessário no convidado. Nunca ligar túneis públicos em silêncio.

## Evidências revisadas e fronteiras de confiança

Leituras de 10/11/2026: [código e regras do registro, commit b958f4c](provas/registro-upstream-2026-11-10.md) e [código e documentação da IDE, commit 3e3088c](provas/openidea-host-2026-11-10.md). A IDE efetivamente fixa o executável aprovado por caminho e hash; isso não demonstra confinamento do auxiliar nativo. O host resolve alguns symlinks, inclusive internos: o Collab deve cumprir a exigência mais forte do mantenedor de **não escrever por symlinks existentes**, incluindo races e hardlinks testados por plataforma. A API `net:ask` existe na documentação atual para HTTP, requer API 1.5 e não comprova WebSocket no iframe ou disponibilidade na instalação local. O scheduler documentado tem intervalo mínimo de 60 s e não é transporte realtime. O registrador não garante sozinho imutabilidade da URL/hash de uma versão: preservar a tag histórica e lançar versão nova é requisito de procedência e desta revisão, não uma propriedade afirmada do CI.

## Instalação pelo OpenIDEA: prova antes de prometer

1. Verificar com o host e as regras de listagem se um pacote assinado pode incluir/distribuir executável auxiliar por plataforma, onde se instala, como atualizar/remover e que permissões são possíveis. Testar a instalação real por **Settings → Plugins** com um pacote em registro de teste; preservar a concessão explícita `process:openidea-collab-helper` e confiança do projeto. Não baixar ou executar binário adicional silenciosamente.
2. Se não houver mecanismo aceito de empacotamento/instalação do auxiliar, registrar evidências e escolher uma das alternativas: (a) instalador externo explícito que verifica assinatura e versão; (b) PR genérico no `l0ng-ai/openidea` para entrega verificável de auxiliares; (c) transporte disponível no host se a API e as permissões o suportarem. O marketplace permanece sujeito a `l0ng-ai/openidea-plugins`.
3. Assistente no plugin: escolher **gerenciado**, **servidor próprio** ou **túnel temporário** (quando suportado); explicar o que será compartilhado, pedir aprovação necessária, mostrar conectado/pendente/offline, convites e parar. Atualização e desinstalação preservam alterações pendentes, sem credenciais em logs.

## Contrato de protocolo v1 proposto — ainda não implementado

- `wss://` fora de loopback; autenticação no handshake por credencial de membro no cabeçalho, nunca em URL. Depois autorizar sala, projeto, documento e operação em cada mensagem, inclusive durante revogação; criação de sala exige controles de abuso e autenticação conforme o modo de implantação. Credenciais não aparecem em logs, métricas ou arquivo de configuração em texto.
- Negociar versão, capacidades, limites e compressão no handshake. Frames binários com tipo e comprimento validados: `HELLO`, `STATE_VECTOR`, `UPDATE`, `ACK`, `PRESENCE`, `ERROR` e encerramento; IDs de sala/projeto/documento opacos, sem caminhos absolutos. Esquema final e compatibilidade devem ser testados antes de fixar wire format.
- Deltas Yrs incrementais, micro-lotes com tempo máximo e tamanho limite, envio e recepção independentes e filas com backpressure. ACK de durabilidade só após persistência bem-sucedida; IDs/revisões e operações idempotentes; estado da fila mostrado. Na reconexão, backoff com jitter e troca de vetor de estado; quando não houver continuidade, snapshot validado. Evitar a disputa atual de entrada de PTY com rede que pode cancelar futures.
- Presença e cursores são efêmeros, limitados e não bloqueiam edições. Alterações pendentes sobrevivem a reinício conforme política de armazenamento aprovada; se não for possível, bloquear fechamento silencioso e oferecer recuperação verificável. Testar Unicode UTF-16, Undo, editor desfocado e concorrência.
- Long polling existente fica como fallback negociado após teste de comportamento equivalente; falha para fallback não deve duplicar nem perder alterações. Não prometer melhoria antes de medir.

## Compressão e limites

- Para snapshot inicial de projeto, compressão em fluxo com limites de tamanho por entrada, quantidade de arquivos, tamanho comprimido/descomprimido, proporção de expansão, tempo e memória. Calcular integridade em chunks, permitir cancelamento e resumir quando protocolo comprovar consistência. Não montar o projeto inteiro na RAM.
- Para edição, enviar deltas pequenos sem compressão por padrão; avaliar compressão negociada apenas para blocos maiores com benchmark CPU/RAM/latência. A [RFC 7692](https://www.rfc-editor.org/rfc/rfc7692) define extensão WebSocket, mas **não comprova** suporte do Axum/dependências usadas: verificar implementação antes de adotar. Evitar compressão de segredos com conteúdo controlável por atacante no mesmo contexto; limitar expansão e desabilitar em caso de dúvida.
- Os limites atuais (helper 4 KiB, documentos servidor 1 MiB e journal finito) são restrições existentes; novos limites precisam ser configuráveis, testados e documentados antes de prometer projetos grandes.

## Segurança e integridade exigidas no PR

| Pedido | Implementação a provar | Teste de aceite |
|---|---|---|
| Escopo de escrita | Compartilhamento consentido por projeto/subárvore; validar caminhos relativos, sem seguir links; negar `.git`, `.openidea`, hooks e metadados de controle | Traversal, links, hardlinks, corrida de substituição, nomes especiais e acesso fora da raiz não criam/escrevem nada |
| Projeto correto | Identidade de projeto e sala vinculada a caminho resolvido; mudança requer nova autorização | Abrir projeto B após A não envia/escreve em A ou em sala antiga |
| Segredos/exclusões | `.gitignore`, `.collabignore`, exclusões seguras `node_modules`, `.venv`, builds, credenciais e regras editáveis; prévia dos arquivos e motivos | Nenhum arquivo fora da seleção é enviado; avaliar `.env`, chaves e negações; ignorar não detecta todos os segredos |
| Participante e transporte | Credencial individual forte, autorização por mensagem, convites com expiração/uso único, revogação efetiva e TLS | Impersonação, replay, acesso entre salas, downgrade TLS e revogação durante socket/poll são rejeitados |
| Não perder trabalho | Fila durável, ACK de persistência, backup/merge, conflitos explícitos, reenvio seguro e armazenamento limitado | Queda após envio/antes do ACK, restart, duplicata, 409/507, disco cheio, duas edições e buffer não salvo não causam perda silenciosa |
| Lifecycle e rotas | Ativação compatível e rotas Axum atuais; testar pacote final, não apenas código em desenvolvimento | Início limpo e entrega ociosa real na IDE; rotas não entram em panic |
| Licença e origem | MIT em fonte/manifests e notices; nova tag imutável com ID correto; pacote gerado e assinado do commit da tag | Verificar assinatura, hash, manifesto, licença, dependências e reprodução/compatibilidade antes de mudar entrada PR |

Além disso: armazenamento no servidor não implica criptografia ponta a ponta; documentar operador e retenção. Negar arquivo especial e extração insegura, limitar taxa e recursos por membro/sala, não sobrescrever, apagar ou executar arquivos remotos sem decisão apropriada. Revisão independente e testes adversariais antes de expor projeto real.

## Sequência de implementação com dependências e testes

Cada etapa deve resultar em fatia revisável, executar testes focados com `CARGO_BUILD_JOBS=1`, recursos limitados e não interromper IDE nem serviços em uso. Não executar builds paralelos pesados. Testes específicos abaixo são critérios, não resultado já obtido. Antes de cada alteração, adicionar evidência reproduzível com commit, ambiente, comando e resultado em `tasks/provas/`, sem texto de projeto ou tokens; indicar explicitamente testes ainda não executados.

| ID | Entrega pequena e dependências | Aceite e verificação |
|---|---|---|
| 01 | Baseline e telemetria sem conteúdo; nenhuma | Reproduzir em 2 clientes; medir edição→render p50/p95/p99, RTT, CPU/RAM, bytes e cancelamentos local/túnel; dados não expõem texto/token |
| 02 | Prova do pacote auxiliar e permissões no OpenIDEA; nenhuma | Testar instalação, atualização, remoção e aprovação em IDE real; registrar impedimentos e API/PR necessários |
| 03 | Contrato WSS + negociação em `collab-core`; 01 | Testes de esquema, limites, incompatibilidade e mensagens inválidas; não expor publicamente |
| 04 | Socket autenticado servidor + revogação; 03 | Integração real com dois membros, permissão por operação, desconexão/revogação e limites; sem tokens em URL |
| 05 | Auxiliar WSS em segundo plano; 03–04 | Dois auxiliares editam simultaneamente; envio/recepção não bloqueiam entrada e fila limitada; comparar baseline |
| 06 | ACK persistente e reconexão; 05 | Matar cliente/servidor em pontos selecionados, repetir/reordenar mensagens, recuperar sem perda e sem falso ACK |
| 07 | Fallback e interface de estado; 05–06 | Testar proxy que bloqueia WSS, escolha explícita, reconnect, falhas e operação em editor customizado real |
| 08 | Compressão condicional; 01, 05 | Testar dados grandes/incompressíveis, limites de expansão e ataques; apresentar CPU/RAM/latência antes/depois |
| 09 | Identidade do projeto e política de compartilhamento; 02 | Trocar de projeto não reutiliza sessão sem consentimento; permissões e prévia verificadas |
| 10 | Seleção, ignore e staging de projeto; 09 | Arquivos omitidos auditáveis; testes de `.git`, hooks, `.env`, links, traversal e limites; staging privado |
| 11 | Transferência inicial em chunks e integridade; 08–10 | Duas máquinas, convidado sem cópia, interrupção/retomada e memória limitada; validação antes da aplicação |
| 12 | Aplicação e conflitos; 10–11 | Nunca sobrescrever mudanças/buffer não salvo sem decisão; recuperar após falha parcial, backups/rollback e teste de concorrência |
| 13 | Mudanças contínuas de árvore; 12 | Criar/renomear/apagar com autorização, ordem e reconexão; mudanças em exclusões não trafegam |
| 14 | Gerenciado e self-hosted; 04–07 | Mesma suíte contratual em ambos; TLS, limites, operação e custos documentados; gerenciado só após operador definido |
| 15 | UI/instalação completa e túnel opcional; 02, 07, 14 | IDE instala pacote aceito, permissões explícitas, assistente funcional; convidado não instala túnel |
| 16 | Editor nativo; 02, 05 | Provar API existente; caso insuficiente, implementar e testar PR no upstream; não anunciar suporte antes de release compatível |
| 17 | Licenças, supply chain e nova release; 06, 09–15 | Auditoria de dependências/notices, tag MIT com ID correto, compilação da tag, assinatura/hash e teste do pacote instalado |
| 18 | Atualização do PR #102; 17 | Atualizar pacote/hash e descrição com evidências por pedido; checks reais verdes e revisão do mantenedor |

**Checkpoints:** 01–02 valida premissas; 03–07 prova um arquivo seguro ponta a ponta; 08–13 prova projetos; 14–18 prova distribuição e reavaliação. Itens 11–13 não são dependência para afirmar que a prova por arquivo funciona, mas são obrigatórios para anunciar colaboração em projetos inteiros. Não pedir revisão do mantenedor como se concluída antes dos critérios do PR.

## Critérios de performance e experiência

Antes da implementação, definir carga repetível e metas realistas; apresentar p50/p95/p99 em localhost, túnel e duas cidades, com computadores e arquivos identificados por tamanho (sem conteúdo). Comparar em mesma rede/persistência: baseline, polling otimizado e WSS, incluindo digitação contínua, reconnect, CPU, RAM, bytes e filas. A meta é melhorar resultados medidos sem regressão de integridade, não garantir um número baseado em suposição. Documentar plataformas, versões de IDE, limites e testes que não foram executados.

## Fontes primárias

- [Revisão do mantenedor](https://github.com/l0ng-ai/openidea-plugins/pull/102#issuecomment-6756170352) e [regras do registro](https://github.com/l0ng-ai/openidea-plugins#rules).
- [Guia da API OpenIDEA, revisão inspecionada](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md).
- [Axum WebSocket](https://docs.rs/axum/latest/axum/extract/ws/index.html), [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455), [RFC 7692](https://www.rfc-editor.org/rfc/rfc7692) e [Yjs updates](https://docs.yjs.dev/api/document-updates).
- [Cloudflare Quick Tunnels](https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/) e [suporte a WebSocket](https://developers.cloudflare.com/cloudflare-one/faq/cloudflare-tunnels-faq/).

## Rastreabilidade de revisão e pacote final

- Para cada item do [comentário de revisão](https://github.com/l0ng-ai/openidea-plugins/pull/102#issuecomment-6756170352), manter entrada separada de **código alterado + teste que reproduz a falha antiga + resultado + limite residual**. Não trocar o pedido de testar integridade por mera aprovação do CI.
- `SorPuti/openidea-collab`: código e release; uma nova tag cujo commit contenha `LICENSE` MIT, README, manifestação e ID/versão iguais ao pacote; compilar e assinar dessa fonte; `openidea-plugin check` com a versão adequada, comparar assinatura, ID, engine e SHA-256. Não reescrever tag histórica nem inserir chave privada.
- `l0ng-ai/openidea-plugins`: somente depois de evidências e release assinada, atualizar a entrada de versão/URL/hash e PR #102 conforme [schema e CI](provas/registro-upstream-2026-11-10.md). `description` não pertence ao TOML desse índice; documentação vai ao README/manifesto/descrição do PR. CI verde e merge dependem dos mantenedores.
- `l0ng-ai/openidea`: PR separado somente quando uma capacidade de host necessária, como editor nativo ou distribuição segura do auxiliar, não estiver disponível na API comprovada. Incluir caso genérico, análise de segurança, testes de compatibilidade e aguardar merge/release; não tratar o fork como suporte upstream.
- Preview atual é um teste com vínculo de desenvolvimento, não publicação aprovada. Não publicar novos binários como prontos até cumprir a etapa de procedência, auditoria de dependências e validação de instalação.

## Instalação prática e opções gratuitas entre cidades — pesquisa de 10/11/2026

### Objetivo de UX: instalar uma vez e criar ou entrar

```text
ANFITRIÃO                                      CONVIDADO
OpenIDEA → instalar Collab                     OpenIDEA → instalar Collab
       ↓                                             ↓
Conceder execução do auxiliar                  Conceder execução do auxiliar
       ↓                                             ↓
Criar sessão → escolher modo de conexão        Entrar → aceitar convite
       ↓                                             ↓
Plugin inicia servidor/túnel apenas se         Plugin usa endereço e credencial
esta opção for escolhida e suportada           contidos no convite validado
       └───────────── WSS + CRDT ───────────────────┘
```

**Meta, não capacidade comprovada:** instalação do pacote no marketplace, preparação do auxiliar/servidor/túnel por plataforma, integridade dos executáveis e aprovação pelo usuário; funcionamento após atualização, com restart e desinstalação. O host atual pede seleção e aprovação com caminho e hash. Não afirmar que instalar o pacote já instala/configura automaticamente os três binários: o teste do pacote oficial, política de distribuição e capacidade do host são pendentes. Executar PoC nos principais sistemas escolhidos, verificar assinatura/digest para cada artefato, procedência/licenças, permissões de execução e comportamento quando o arquivo mudar. Se o host não oferecer mecanismo nativo e seguro, propor PR upstream ou fluxo de instalação separado explícito; jamais contornar grant. No convidado, servidor/túnel não são necessários.

**Convite:** formato versionado com endereço validado, ID de sala e credencial individual de curta duração/uso único ou troca segura; nunca apenas código curto como autorização. Mostrar destino e escopo antes de entrar, não colocar segredo em query string, arquivos de log ou argumentos de processo; proteger clipboard e dar possibilidade de revogar. Detalhes do modo de envio precisam ser validados contra a API da IDE.

### Alternativas gratuitas e restrições

| Modo | Como conecta cidades | Custo/requisitos/limites | Situação neste projeto |
|---|---|---|---|
| **Cloudflare Quick Tunnel** | Anfitrião mantém servidor loopback e `cloudflared`; convidado se conecta via HTTPS/WSS ao endereço gerado | A [documentação oficial](https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/) informa que não requer conta/domínio e é para teste; URL muda, sem garantia de disponibilidade, limite de 200 requests em voo. Qualquer pessoa com URL pode acessar o serviço; autenticação por e-mail requer navegador interativo e não funciona diretamente para o auxiliar sem adaptar o fluxo. | Dois computadores usaram túnel HTTPS manualmente. Integração automática, modo WSS e desempenho extremo a extremo **não comprovados**. O anfitrião precisa ligar o computador e manter servidor/túnel vivos. |
| **Cloudflare Tunnel com domínio** | Anfitrião conecta por túnel de saída a hostname próprio acessível via TLS | [Tunnel disponível em todos os planos](https://developers.cloudflare.com/tunnel/); [configuração](https://developers.cloudflare.com/tunnel/get-started/) pede conta, domínio e máquina/VM com cloudflared. Domínio/hospedagem podem custar; não prometer custo total zero ou SLA. | Opção auto-hospedada planejada; provisioning, política e suporte WSS do plugin não testados ponta a ponta. |
| **ngrok Free** | Anfitrião roda agente e recebe endpoint HTTPS; convidado acessa endpoint | [Limites oficiais](https://ngrok.com/docs/pricing-limits): plano gratuito tem cotas e pode exigir conta/autenticação; não prometer uso ilimitado. A disponibilidade e limites devem ser conferidos no momento de implantação. | Alternativa, não integrada; pode exigir configuração de credenciais no anfitrião. |
| **Tailscale Personal** | Ambos integram a mesma rede privada; acessam servidor por rota privada | [Plano pessoal gratuito](https://tailscale.com/pricing) é para uso não comercial; ambas as máquinas precisam instalar/configurar cliente. Um IP privado sozinho não satisfaz o requisito atual de HTTPS remoto: projetar TLS confiável ou acesso loopback/túnel seguro e testar. | Alternativa voluntária, não exigida no modo gerenciado; nenhum teste do Collab por Tailscale foi realizado. |
| **Servidor hospedado gratuito ou próprio** | Ambos usam endereço público WSS de relay com TLS | É possível operar em infraestrutura própria; camada grátis de terceiros varia, pode encerrar/dormir, exigir conta/cartão ou cobrar excedentes. Não há provedor, cota, disponibilidade ou custo gratuito garantido. | Serviço gerenciado ainda não existe; selecionar provedor e verificar preço, política de dados, persistência e limites antes de prometer. |

### Entregas e testes adicionais obrigatórios

- [ ] Implementar adaptadores de conexão `managed`, `self_hosted`, `quick_tunnel` e, quando comprovado, `private_network`, todos sobre o mesmo protocolo autenticado. Abstrações são propostas, não configurações já existentes.
- [ ] Garantir escolha explícita de exposição pública, indicador ativo, URL/endereço, encerramento, erro e reinício. Não exibir token em log; usar limites de recursos. Não depender de Quick Tunnel para produção.
- [ ] Produzir pacote por plataforma e testar: instalação via marketplace, grant inicial, pin de hash, bloqueio de binário modificado, upgrade e rollback, IDE sem privilégios elevados, desinstalação, reinstalação e idioma. Documentar, caso falhe, se exige instalador separado ou PR upstream.
- [ ] Testar em duas redes reais NAT/firewall: criar sala, convite de uso único, editar simultaneamente, WSS através do túnel, fallback, queda/reconexão, desligar host, reiniciar túnel com novo endereço, revogar e relatar p50/p95/p99 e memória. Não apresentar o teste HTTP antigo como prova de WSS.
- [ ] Antes de publicar, comparar obrigações de redistribuição e licenças dos executáveis e bibliotecas; respeitar termos dos provedores. O atalho gratuito é uma opção para testes, não substitui revisão de segurança nem disponibilidade de um serviço mantido.
