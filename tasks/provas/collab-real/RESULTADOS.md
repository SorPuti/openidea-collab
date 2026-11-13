# Provas Collab e estudo do painel — 09/11/2026

> Registro histórico das primeiras provas. Desde então foi implementado um editor experimental com auxiliar e servidor reais; consulte [prova e desenho atualizados](DESIGN-E-PROVA.md) para os resultados mais recentes. As observações abaixo sobre o processo da IDE e o ping descrevem aquele momento, não necessariamente o estado atual.

## O que foi testado

1. **Código real do painel Rust com host simulado:** numa cópia temporária de `openidea-collab`, o teste `sdk-test.rs` injeta `ViewEvent::Message` de criar sala. `MockHost` simula as respostas HTTP: a implementação real emite POST `/rooms`, POST `/rooms/{id}/join`, guarda a sala e emite mensagem de estado para `collab-panel`. Dois testes passaram, incluindo reprodução do falso positivo do ping. Reproduzir: `python3 tasks/provas/collab-real/sdk.py`. Exige dependências no cache. Não é IDE real e os endpoints são simulados.
2. **Servidor real isolado:** `python3 tasks/provas/collab-real/servidor.py` copia o servidor atual a um diretório temporário, altera apenas a cópia para bind loopback e inicia em porta temporária; usa HTTP real para criar sala, entrar, enviar/ler um arquivo e receber evento. Passou. Não testa plugin, login seguro, rede pública ou concorrência em duas IDEs.
3. **Instalação e processos:** `openidea-plugin links` mostrou um link local de desenvolvimento. O servidor existente escutava em todas as interfaces na porta 4444. Não alteramos esse processo. Não havia processo OpenIDEA detectado; não foi validado UI/host na IDE.
4. **Bug comprovado no servidor existente:** `GET /health` respondeu 404 e `GET /rooms` respondeu 405; `CollabClient::ping` aceita todo status `<500`. O teste simulado reproduz que 404 é tratado como sucesso. Corrigir requires endpoint de saúde real e aceite apenas da resposta esperada, além de testar identidade do serviço; não confundir alcance com colaboração funcional.
5. **Prévia de design:** [design.html](design.html) foi aberta em Chrome headless usando Playwright; selecionar servidor próprio, preencher nome/URL, abrir e sair de sessão simulada passou. [Captura](design.png). Esta página é independente, com dados simulados e sem conexão de rede. Não é captura da IDE nem implementação do plugin.

## Fluxo demonstrado e ainda faltante

```text
Clique do painel ─► ViewEvent ─► CollabPlugin ─► host::fetch ─► Servidor
    ▲                  [teste com MockHost]              [HTTP real isolado]
    └──── post_view_message ◄── resultado
```

As duas provas validam partes diferentes; **não foi provada a ligação completa** painel na IDE real → host Wasm → servidor real → duas IDEs, nem colaboração por digitação. Para isso: iniciar uma IDE de teste e projeto descartável, conferir versão/consentimento, testar criar/entrar e solicitar segundo cliente. Medir, reproduzir erro e documentar resultados. Não alegar tempo real sem eventos contínuos, CRDT e reconexão comprovados. O canal PTY e auxiliar também continuam não validados no host real.

## Direção do design

Painel lateral compacto, com status textual, seleção de serviço gerenciado ou servidor próprio, nome e ações criar/entrar; ao conectar, mostrar sessão, participantes, conexão e saída. Mostrar permissões e comportamento de erro sem depender apenas de cor. Respeitar tokens de tema do OpenIDEA e navegação por teclado. Seleção gerenciada é proposta: nenhum serviço está configurado. Endereço próprio só funciona se a permissão necessária estiver disponível. Ao integrar: mapear componentes ao `page.html` e `Views::handle`, não interpolar entradas não escapadas no HTML/JS; validar mensagens e tratar erros de rede. Convite seguro e clipboard exigem implementação e permissão adequadas; não indicar cópia ou conexão bem-sucedida sem confirmação.

## Fontes

- [Views e mensagens do OpenIDEA v0.1.40](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#views-and-custom-editors).
- [Web views, isolamento e custom editor](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#web-views).
- [Rede e permissões](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#network-and-secrets).
- [Canal PTY e limites](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#web-frame-ptys-api-13).
