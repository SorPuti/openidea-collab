# Collab: desenho, implementação e evidências

Atualizado em 09/11/2026. Este documento distingue especificação da plataforma, código implementado e testes executados. As capturas usam Chrome e uma ponte simulada, não são capturas do OpenIDEA.

## Desenho proposto

```text
                    ESCOLHA DE CONEXÃO                       ESCOLHA DE EDITOR
         ┌───────────────────────────────┐          ┌─────────────────────────────┐
         │ A. Servidor próprio           │          │ 1. Editor nativo            │
         │    HTTPS em host administrado │          │    requer prova das APIs    │
         │ B. Serviço gerenciado futuro  │          │    e possível PR no host    │
         │    HTTPS, sem ngrok no cliente│          │ 2. Editor personalizado    │
         │ C. Local para desenvolvimento │          │    protótipo *.collabtest  │
         └──────────────┬────────────────┘          └──────────────┬──────────────┘
                        │      opções independentes                 │
                        └────────────────┬─────────────────────────┘
                                         ▼
┌────────────────────────────────── OpenIDEA ──────────────────────────────────┐
│ Painel Collab: servidor, sessão, participantes, erros e ações [design alvo]  │
│                                                                              │
│ Editor personalizado experimental                                            │
│  [ Criar / Entrar ]  [ Servidor ]  [ Convite ]     Estado: conectado         │
│  ┌────────────────────────────────────────────────────────────────────────┐  │
│  │ documento compartilhado; edição versionada do documento da IDE        │  │
│  └────────────────────────────────────────────────────────────────────────┘  │
│       │ document.get / edit / onChange              │ mensagens PTY          │
│       ▼                                             ▼                        │
│  Documento no host                           Auxiliar Rust autorizado        │
└─────────────────────────────────────────────────────┬────────────────────────┘
                                                      │ HTTP local / HTTPS remoto*
                                                      ▼
                              ┌───────────────────────────────────────────────┐
                              │ Servidor experimental: salas, funções, CRDT   │
                              │ autêntica requisições; estado só em memória   │
                              └───────────────────────────────────────────────┘
```

\* O servidor experimental só aceita bind loopback e não fornece TLS: uma implantação remota futura precisa de proxy TLS e de trabalho adicional de segurança e persistência. Não existe serviço gerenciado provisionado. Sem túnel instalado no cliente é viável quando já existe um serviço remoto alcançável; alguém precisa operá-lo.

## O que o código existente faz

| Peça | Local | Estado |
|---|---|---|
| Registro e permissão | `openidea-collab/plugin.toml` | Custom editor opcional apenas para `*.collabtest`; processo auxiliar exige concessão. |
| Interface experimental | `openidea-collab/web/collab-editor/index.html` e `editor.js` | Criar/entrar, endereço, convite e área de texto; estados e mensagens de erro. Ainda é protótipo. |
| Documento no host | `editor.js` | Obtém versão, aplica alterações UTF-16 com `document.edit` e acompanha `onChange`; conflito bloqueia edição e preserva texto visível para cópia. |
| Transporte local | `collab-helper/src/main.rs` | Processo Rust, protocolo JSON por linhas via PTY, gerencia sessão e troca de atualizações; em Unix desativa eco no PTY. |
| Sincronização | `collab-helper/src/lib.rs`, `collab-core/src/lib.rs` | Cliente HTTP e documento Yrs; alterações pendentes ficam somente na memória do auxiliar. |
| Servidor | `collab-server/src/realtime.rs` | API experimental separada da legada, salas e convites com papéis; dados e credenciais somente em memória. |
| Instalação | `scripts/install-realtime.sh` | Modos experimentais local, remoto e gerenciado configuram o cliente; modo local pode iniciar servidor isolado. Não provisiona serviço remoto. |

O painel lateral existente e o editor experimental ainda não são uma experiência única: o painel atual usa a API legada de sincronização ao salvar. Ele não é prova de que criar sala no painel controla o editor em tempo real. Presença e cursores colaborativos não estão comprovados no novo fluxo.

## Prova reproduzível

Na raiz do projeto, com Chrome, Playwright e binários de debug já compilados:

```sh
cargo build --manifest-path collab-server/Cargo.toml --offline
cargo build --manifest-path collab-helper/Cargo.toml --offline
python3 tasks/provas/collab-real/editor_bridge.py
python3 tasks/provas/collab-real/editor_end_to_end.py
```

Em 09/11/2026, `editor_bridge.py` passou para offsets UTF-16, limpeza de convite e preservação visual de texto quando a versão fica obsoleta. `editor_end_to_end.py` passou com dois editores no Chrome, dois processos auxiliares reais em PTYs reais e um servidor real em loopback: criou uma sala e convite, conectou o segundo cliente, executou edições simultâneas e verificou que os dois textos e os documentos simulados convergiram.

Uma prova complementar de baixo consumo executa o **código real da ponte PTY do OpenIDEA v0.1.40** com o processo real do auxiliar: `TMPDIR=/var/tmp python3 tasks/provas/host-real/bridge_proof.py`. Ela passou ao receber `ready`, confirmar o recebimento (`pty-ack`) e enviar uma entrada que recebeu resposta de erro válida. A prova substitui somente imports de tipo e tradução da ponte; seu backend de processos é simulado e usa pipes, não o PTY nem o backend Rust do OpenIDEA. Requer Node com suporte a remoção de tipos TypeScript e o checkout da tag no cache do Cargo. Não valida permissão nem interação visual na IDE.

Uma segunda prova conecta dois exemplares do próprio `editor.js` à ponte original do OpenIDEA, a dois auxiliares reais e ao servidor real: `TMPDIR=/var/tmp python3 tasks/provas/host-real/editor_bridge_actual.py`. Criar sala, gerar convite, entrar e fazer edições simultâneas com Unicode convergiu. O DOM e a API de documento são simulados e a ponte usa adaptadores de processo por pipes; o PTY do host, a concessão de permissões e o layout na IDE não são testados. Nesta máquina, `/usr/bin/time` registrou pico de 83.808 KiB por processo no teste; não é uma medição da memória total dos processos simultâneos.

O mesmo teste passou com os arquivos de release preparados em `/home/bugson/.local/share/openidea-collab-experimental/releases/.staging.t7ddkJxS`, usando `--release` seguido desse diretório. O pacote passou em `openidea-plugin check`; a preparação teve limite de memória de 1.200 MiB, um processo de compilação, e não vinculou a nova versão nem iniciou servidor permanente. O link anterior e o serviço `collab-server.service` continuavam existentes após a prova. A aprovação do executável e o teste em uma IDE real ainda são necessários.

Posteriormente, o vínculo de desenvolvimento foi trocado para essa versão com `scripts/switch-dev-link.py`; a cópia anterior permanece em `/home/bugson/.local/share/openidea-collab-plugin` e o mesmo script permite restaurá-la com `--apply`. O serviço experimental transitório `openidea-collab-experimental.service` foi iniciado somente em `127.0.0.1:4445`, com limite de 128 MiB e journal em `/home/bugson/.local/share/openidea-collab-experimental/data/journal.sqlite`. A saúde respondeu com identidade `realtime-experimental`, e o serviço legado permaneceu ativo. O journal contém documentos em texto não criptografado; a unidade transitória não foi habilitada para reinício automático. A interface nativa não estava acessível à automação, portanto a aprovação do auxiliar e o teste com duas janelas continuam sem prova.

O teste **simula** os métodos e mensagens do host OpenIDEA. Ele não comprova abertura na IDE, concessão da permissão, integração com o editor nativo, undo/save reais, comportamento em redes distintas, latência, recuperação após reinício, ou segurança de produção. A imagem [editor-experimental.png](editor-experimental.png) registra o resultado do teste; [design.png](design.png) é uma proposta separada para o painel, sem ligação com o servidor.

## Como o desenho se traduz em código

Registro efetivo, extraído do manifesto atual:

```toml
[capabilities]
optional = ["process:openidea-collab-helper"]

[[contributes.customEditors]]
id = "collab-experimental"
title = "Collaborative Editor (experimental)"
patterns = ["*.collabtest"]
priority = "option"
web = "web/collab-editor/index.html"
```

Para o documento, a implementação real calcula o intervalo `diff` em unidades UTF-16 e usa a versão obtida do host:

```js
const state = await openidea.document.get();
const change = { from: 0, to: 0, insert: 'Olá' }; // exemplo de intervalo UTF-16
const result = await openidea.document.edit(state.version, [change]);
// result.version é a versão seguinte; tratar rejeição stale e preservar texto.
```

O editor envia ao host a solicitação de processo e depois mensagens JSON terminadas em nova linha. No código real, também processa `pty-data`, confirma o recebimento com `pty-ack`, e trata saída e erro:

```js
openidea.postMessage(JSON.stringify({
  op: 'pty-spawn', sessionId, program: 'openidea-collab-helper',
  args: [], cols: 100, rows: 28
}));
```

Estes fragmentos mostram o contrato; não substituem `editor.js`, tratamento de mensagens, inicialização do servidor, permissão do usuário ou testes. A documentação da IDE descreve [documentos e web views](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#web-views) e [PTYs, projeto confiável e concessão de processo](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#web-frame-ptys-api-13). A [documentação do Yjs](https://docs.yjs.dev/api/document-updates) descreve as propriedades das atualizações CRDT; isso não comprova durabilidade da nossa implementação.

## Próximos passos e propriedade

1. **Repositório Collab:** integrar painel e editor ao mesmo controlador de sessão; exibir endereço/operador, participantes, permissões, falhas e alterações pendentes; manter acessibilidade por teclado e estados textuais. Definir editor completo, atalhos, undo e recursos de linguagem.
2. **Repositório Collab:** implementar persistência transacional do servidor e fila local recuperável; testar reinício, mensagens repetidas, revogação durante espera, limites agregados, abuso e proteção de segredos. TLS protege o trajeto; não foi implementada criptografia de ponta a ponta.
3. **OpenIDEA real:** instalar em ambiente descartável, conceder a permissão, abrir `*.collabtest` pelo editor opcional e testar duas janelas reais com edição, Unicode, salvar, undo, desconexão e retomada. Registrar versão, comandos, resultado e limitações.
4. **Repositório OpenIDEA:** somente propor PR para integração nativa se a investigação demonstrar APIs ausentes; registrar caso mínimo, contrato e testes. A existência do custom editor não prova paridade com o editor nativo.
5. **Registro openidea-plugins:** após pacote, fontes, permissões e release estarem prontos, atualizar a proposta de registro existente. A [política do registro](https://github.com/l0ng-ai/openidea-plugins#rules) prevê revisão dos mantenedores; o resultado não pode ser garantido.
6. **Serviço gerenciado:** definir operador, custos, domínio, política de dados, disponibilidade e implantação antes de apresentá-lo como disponível. A opção de servidor próprio continua no plano.

O diretório `tasks/` está ignorado pelo Git neste checkout: estes documentos são locais e precisam ser incluídos explicitamente ou movidos para um caminho versionado antes de acompanhar um PR.
