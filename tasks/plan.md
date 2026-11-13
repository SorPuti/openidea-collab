# Plano de produto: OpenIDEA Collab

Data: 09/11/2026. Complementa [a pesquisa com evidências](../docs/pesquisa-colaboracao-openidea.md). Escopo: **OpenIDEA**, projeto de l0ng-ai, não IntelliJ IDEA da JetBrains. Um plugin para IntelliJ exigiria integração e avaliação separadas.

## Objetivo e limites

Permitir que duas ou mais pessoas editem o mesmo documento aberto em instâncias distintas do OpenIDEA, vejam presença e cursores, recuperem a sessão após desconexão e mantenham suas alterações concorrentes. O plano mantém a opção self-hosted e acrescenta uma opção de serviço gerenciado, com permissões explícitas, acesso por convite e documentação de operação. A experiência gerenciada exige um operador e infraestrutura disponíveis; não há serviço provisionado neste plano. Não prometer colaboração completa antes da validação ponta a ponta. Agentes, terminais compartilhados, execução remota, binários, E2EE e edição offline prolongada ficam fora da primeira versão; dependem de proposta específica.

O objetivo é preparar uma contribuição revisável, não garantir aprovação ou funcionamento perfeito. Mudanças no host dependem da aceitação e da versão lançada pelo OpenIDEA. Não supor que APIs existentes bastam: os limites de eventos, agendamento e editor estão documentados na pesquisa.

## Ownership e desenho dos adaptadores

A [matriz de ownership e arquitetura comum](arquitetura-ownership.md) distingue aprovação de upstream, aprovação do registro e consentimento do usuário; define os adaptadores para conexão e editor e as limitações de seleção por configuração. [Provas executáveis](provas/README.md) registram exatamente o que foi testado e o que ainda requer IDE real.

## Prova do Collab e proposta visual

A [evidência do painel com SDK simulado e servidor HTTP isolado](provas/collab-real/RESULTADOS.md) inclui [prévia visual interativa](provas/collab-real/design.html). São provas parciais, não teste da IDE ou colaboração ponta a ponta.

## Referências para implementação

Os [snippets e fontes pesquisados](snippets-referencia.md) incluem editor personalizado, canal PTY candidato, permissões, health check validado em cópia isolada, Yrs/UTF-16, protocolo e instalador. Cada trecho identifica sua versão e grau de validação.

## Duas opções complementares

1. **Original — servidor próprio:** o usuário ou sua equipe opera o servidor. O plano continua incluindo as propostas de API ao OpenIDEA para integração no editor nativo.
2. **Nova — conexão gerenciada:** o plugin conecta automaticamente a um serviço operado pelo projeto, sem que o usuário instale ngrok ou configure túneis. Pesquisar também um editor colaborativo personalizado para funcionar em versões existentes da IDE sem PR de host, sujeito à prova descrita abaixo.

Conexão e editor são decisões independentes: o serviço gerenciado também poderá usar o editor nativo quando as APIs existirem; o editor personalizado poderá usar um servidor próprio. Nenhuma das combinações está implementada ou comprovada. Detalhes e desenhos completos: [opção de conexão gerenciada](opcao-conexao-gerenciada.md). A instalação externa, atualização e remoção para cada modo estão em [instalação customizada](instalacao-customizada.md).

## Arquitetura proposta

```text
  Computador A                         Servidor self-hosted                       Computador B
┌─────────────────────┐              ┌─────────────────────────┐              ┌─────────────────────┐
│ OpenIDEA + editor   │              │ TLS / proxy de entrada  │              │ OpenIDEA + editor   │
│  ├─ API de mudanças │              │ Autenticação e ACL      │              │  ├─ API de mudanças │
│  └─ API de edição   │              │                         │              │  └─ API de edição   │
│          ↕          │   WSS/HTTPS  │ Sessões e protocolo     │  WSS/HTTPS   │          ↕          │
│ Plugin/adaptador    │◄────────────►│ Relay + limites         │◄────────────►│ Plugin/adaptador    │
│          ↕          │              │ Estado/snapshots duráveis│              │          ↕          │
│ CRDT + transporte*  │              │ Telemetria sem conteúdo │              │ CRDT + transporte*  │
└─────────────────────┘              └─────────────────────────┘              └─────────────────────┘
      * localização do CRDT e transporte depende da prova da API/sidecar
```

A posição do motor CRDT é decisão a validar: no plugin WASM, se compatível e dentro dos limites; ou no processo auxiliar, com fronteira IPC e permissões adicionais. Uma conexão WebSocket persistente é candidata, não está disponível no contrato WIT consultado; a alternativa de sidecar precisa demonstrar recebimento de dados no plugin sem polling lento. IPC ou processo auxiliar **não resolve** sozinho ausência de eventos de documento e entrega assíncrona. Não contornar o sandbox ou usar APIs privadas como solução de produção.

Separar documento, presença e transporte: [Yrs](https://github.com/y-crdt/y-crdt) é candidato Rust para um CRDT de texto, sujeito a prova de compatibilidade com WASM, performance, licenças e protocolo. O [modelo de atualizações Yjs](https://docs.yjs.dev/api/document-updates) prevê convergência quando todas as atualizações são entregues; esse fato não garante preservação de dados na integração inteira. [Awareness](https://docs.yjs.dev/api/about-awareness) é temporário e exige heartbeat; não misturar presença com persistência do documento. Não reinventar o protocolo de sincronização sem necessidade.

```text
Edita A → evento versionado do editor → CRDT A → atualização autenticada → servidor
                                                                                ↓
Editor B ← aplicação transacional com versão ← CRDT B ← atualização recebida ←─┘
           └─ origem remota evita reenvio; edição local simultânea é incorporada ─┘
```

Contrato a definir: identificação de workspace/documento independente de caminho absoluto; snapshot e vetor de estado; atualizações, autenticação e renovação, autorização de sala, presença efêmera, versionamento e negociação; ordenação/idempotência, reconexão, limites/backpressure e rejeição de versão incompatível. A abertura inicial deve detectar conteúdo local divergente e pedir resolução explícita; nunca substituir silenciosamente. Política de `save`, undo/redo, encoding/BOM, Unicode/UTF-16, rename/delete e múltiplas janelas precisa ser especificada e testada.

## Entrega ao upstream

```text
Prova mínima + discussão de API
          │
          ▼
PR 1 no openidea: eventos de documento e seleção ─────┐
          │                                            │
PR 2 no openidea: aplicar edição versionada por doc ────┤ → release do OpenIDEA
          │                                            │
PR 3, somente se necessário: entrega assíncrona e/ou    │
     transporte seguro com escopo e limites ───────────┘
          │
          ▼
Plugin + servidor: integração, testes, documentação, release assinada
          │
          ▼
Atualizar PR do registro #102 → validação CI → revisão/merge de mantenedor
```

PRs sugeridos são cortes de revisão, sujeitos a mudança após diálogo; evento e aplicação podem ser um único PR se os mantenedores preferirem. Cada proposta deve conter caso de uso também aplicável a outros plugins, contrato WIT, SDK, host, permissões, compatibilidade, testes, documentação e demonstração executável. Não pedir permissão genérica para rede nem introduzir capacidade não limitada. Eventos devem incluir documento, versão, intervalos, origem e ciclo de vida; não transmitir conteúdo a plugins não autorizados. Aplicações precisam evitar escrita em versão obsoleta, loops, corrupção e afetar undo. Especificar limites, backpressure e coalescência. Usar as [instruções de contribuição do OpenIDEA](https://github.com/l0ng-ai/openidea/blob/main/CONTRIBUTING.md). Um protótipo funcional e feedback são pré-requisitos para afirmar que a proposta é suficiente; revisão por mantenedores continua indispensável.

Se upstream recusar, testar primeiro o editor personalizado com a API existente e entrega assíncrona demonstrada; ele pode ser uma alternativa independente, com diferenças de experiência. Se a prova falhar, oferecer somente sincronização ao salvar com limites explícitos ou um experimento em fork identificado. Não alegar realtime antes da prova ponta a ponta.

## Fases e critérios de saída

1. **Base confiável e PR existente.** Alinhar versão, licença, manifesto e README; corrigir testes; registrar limitações do 0.1.0 e condição do [PR #102](https://github.com/l0ng-ai/openidea-plugins/pull/102). Testar build WASM, verificação do pacote e teste em duas IDEs com dados descartáveis. Check de marketplace não é teste funcional.
2. **Prova de integração e proposta upstream.** Reproduzir ausência das capacidades no release alvo; desenvolver teste mínimo de evento de edição, aplicação remota e entrega assíncrona; propor contratos e colher feedback. Saída: decisão explícita sobre APIs e transporte, com demonstração ou bloqueio documentado.
3. **Fatia vertical: um documento.** Dois clientes autenticados abrem o mesmo documento e editam simultaneamente com convergência, aplicação no buffer, feedback de conexão e testes de concorrência e undo. Projeto local divergente nunca é sobrescrito silenciosamente.
4. **Rede e recuperação.** Convites revogáveis, autorização por sala e função, TLS, proteção de credenciais, reconexão com estado e persistência; restart do servidor e interrupções são testados. Sem prometer E2EE: servidor autorizado pode ver conteúdo.
5. **Experiência e robustez.** Presença com expiração/heartbeat, cursores/seleções, multiarquivo e limites, comportamento para rename/delete, indicadores acessíveis e internacionalização básica. Sem sincronização automática de arquivos secretos; seleção explícita de escopo, política de exclusão documentada.
6. **Beta aberto e marketplace.** Documentos operacionais e de contribuição, política de suporte, CI, testes nas plataformas suportadas e release assinada. Testes reais com usuários, correção de bugs e revisão externa antes de declarar estável.

Cada tarefa em [todo.md](todo.md) tem evidência de conclusão e dependências. As fases são marcos, não garantias de prazo; a dependência externa de API é o maior risco de cronograma.

## Segurança e dados

Identificar convidados e operações com credenciais aleatórias de alta entropia; IDs de sala não são credenciais. Autenticar cada conexão e autorizar ler/escrever por documento e sala; separar host/editor/leitor e revogar convites. Criar limites de mensagens, sessões, recursos, tempo e taxa; validar nomes e caminhos, inclusive traversal/symlinks, antes de qualquer acesso. Loopback por padrão; exposição remota só via TLS autenticado e documentação de proxy. Segredos fora de logs e URLs quando possível; usar mecanismos de segredos do host quando disponíveis. Apresentar quem recebe código, retenção, deleção, backups e comportamento offline; logs sem conteúdo. Modelar ameaças, proteção de dependências e divulgar vulnerabilidades em privado por SECURITY.md. Testar acesso indevido, replay, vazamento de token, revogação e recuperação.

## Padrão open source e licença

O repositório do plugin declara [MIT](https://github.com/SorPuti/openidea-collab/blob/master/LICENSE); o [OpenIDEA usa Apache-2.0](https://github.com/l0ng-ai/openidea). Manter MIT é a direção inicial, sem mudar licença unilateralmente; corrigir a descrição Apache do PR. Confirmar propriedade e termos de cada contribuição e dependência, preservando avisos exigidos; código submetido ao OpenIDEA deve seguir as regras de contribuição e licença daquele projeto. Não afirmar compatibilidade de todas as dependências sem inventário.

Adicionar CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, política de suporte e versões, CHANGELOG, documentação de instalação e servidor, permissões e limitações, arquitetura/protocolo, modelo de ameaça, templates de issue/PR e licença/avisos de terceiros. Definir mantenedores e processo de revisão. CI deve testar plugin e servidor, compilação WASM, formatação/lint, testes de integração e segurança/licenças; fixar ferramentas e dependências, revisar ações e gerar artefato rastreável a commit/tag. Release deve conferir versões, assinar pacote e registrar hash; nunca incluir chave privada. A [política do registro](https://github.com/l0ng-ai/openidea-plugins#rules) exige fonte pública licenciada, justificativa de capacidades de risco e revisão; novo pacote exige nova entrada de versão e merge.

## Critérios de aceite do primeiro beta

- Duas IDEs reais editam simultaneamente o mesmo texto, inclusive Unicode, e convergem sem perda; arquivo inicialmente divergente não é substituído sem decisão do usuário.
- Desconexão, duplicação/reordenação de mensagens, reinício do cliente e servidor e teste de acesso não autorizado são reproduzíveis; nenhuma atualização reconhecida como persistida é perdida nos cenários declarados suportados.
- Limites de latência e capacidade são **metas de teste**, não garantias: estabelecer ambiente, rede, amostra e percentis; medir antes de publicar números.
- Permissões correspondem ao uso real; usuário sabe quais dados saem do computador, para onde e como sair; rejeição de permissão é tratada.
- Build, testes, `openidea-plugin check`, compatibilidade da versão mínima e instalação da release são comprovados; matriz de plataformas registra aprovado, não testado ou não suportado.
- PR upstream mesclado e disponível em release compatível quando o plugin depender dele; para o editor personalizado sem alterações na IDE, prova ponta a ponta equivalente na versão suportada; aprovação do registro ocorre por decisão dos mantenedores. Sem isso, rotular claramente pré-release/fork e não anunciar disponibilidade oficial.

## Atualização: WebSocket, instalação e segurança

O plano de execução e critérios detalhados estão em [plano-collab-websocket-seguranca.md](plano-collab-websocket-seguranca.md). Preservar os planos e tarefas existentes.

As opções de conexão gratuita, requisitos e prova da instalação do auxiliar foram detalhados em [plano-collab-websocket-seguranca.md](plano-collab-websocket-seguranca.md#instalação-prática-e-opções-gratuitas-entre-cidades--pesquisa-de-10112026).
