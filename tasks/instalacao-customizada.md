# Plano de instalação e configuração — OpenIDEA Collab

Data: 09/11/2026. Este guia é um **plano para implementação e execução por um agente**; as opções, comandos propostos, serviço gerenciado e garantias abaixo não existem necessariamente hoje. Complementa [o plano principal](plan.md), [a opção gerenciada](opcao-conexao-gerenciada.md) e [as tarefas](todo.md). Não executar o instalador atual como se implementasse este plano. Não instalar, alterar serviços, publicar ou provisionar infraestrutura durante uma tarefa apenas de planejamento.

## 1. Opções independentes que o instalador deve oferecer

```text
                INSTALADOR EXTERNO OPENIDEA COLLAB
                              │
                 ┌────────────┴─────────────┐
                 ▼                          ▼
           CONEXÃO                       EDITOR
     ┌───────────┼───────────┐     ┌────────┴─────────┐
     ▼           ▼           ▼     ▼                  ▼
  Gerenciada  Próprio     Local   Nativo        Personalizado
              remoto       dev    (API/PR)      (prova prévia)
     │           │           │     │                  │
     └───────────┴───────────┴─────┴──────────────────┘
                              │
                Instalar só o que foi escolhido
                              │
       Permissões → configurar → validar → resumo e diagnóstico
```

- **Gerenciada:** plugin conecta a endpoint de serviço operado pelo projeto; nenhum servidor, túnel ou porta local deve ser criado por padrão. Depende de operador, endpoint, política de dados, serviço disponível e implementação autenticada; não está hoje provisionado.
- **Servidor próprio remoto:** instalar plugin e configurar URL HTTPS de servidor já provisionado pelo usuário. Instalar o servidor em outro host é um fluxo administrativo separado, no host correto, com autorização e acesso existentes. Não instalar automaticamente localmente.
- **Servidor local para desenvolvimento:** instalar e iniciar servidor no dispositivo somente por escolha explícita; loopback por padrão. Serviço de login é opção separada e não deve iniciar sem escolha. Acesso remoto exige infraestrutura TLS e autorização apropriada; não recomendar expor o protótipo atual sem autenticação.
- **Editor nativo:** exige versão oficial da IDE com as APIs necessárias, ou experimentalmente um fork identificado. Detectar incompatibilidade antes da instalação e oferecer alternativa, sem alegar edição realtime.
- **Editor personalizado:** investigar as APIs existentes de custom editor e canal de entrega em tempo real; somente oferecer como funcional após prova e matriz de compatibilidade. Explicar que é uma experiência de edição diferente.

Combinações possíveis: três modos de conexão × dois modos de editor, sujeitos a compatibilidade efetivamente demonstrada. Permitir instalar somente o servidor numa máquina separada, quando isso for implementado. Não instalar automaticamente ngrok, cloudflared, VPN ou abrir portas/firewall. A opção self-hosted e os PRs upstream permanecem no plano.

## 2. Levantamento do `install.sh` existente

O script atual é interativo em Bash; detecta Linux/macOS; baixa Rust se necessário, adiciona alvo `wasm32-wasip2`, instala CLI `openidea-plugin` se ausente, clona a branch padrão, compila servidor e plugin, copia plugin para `~/.local/share/openidea-collab-plugin` e usa `openidea-plugin link`. Instala servidor em `~/.local/bin` por padrão e habilita serviço de usuário systemd ou launchd, configurando porta 4444. Modifica arquivos de inicialização do shell para PATH e sugere ngrok. Não há escolha de conexão/editor nem fluxo de desinstalação completo.

### Problemas constatados ou a validar

1. `_do_verify` consulta `GET /rooms`, mas o servidor registra `POST /rooms`; além disso, essa função não é chamada no fluxo principal. O passo chamado verificação na atualização chama `_do_restart`, não `_do_verify`, e ignora falhas de restart. Não declarar serviço verificado apenas por isso.
2. `_do_server` para o serviço **antes** de compilar; falha na compilação pode deixá-lo parado. O binário antigo é removido antes da substituição terminar; não há rollback transacional.
3. Detecção de atualização considera a presença do diretório ou binário suficiente; pula configuração e usa porta padrão ou variável da execução atual, sem carregar a configuração anterior. O fluxo de atualização não recria o serviço. Assim, serviço e porta exibida/configurada podem divergir.
4. Clone de branch padrão e instalação de CLI a partir de Git sem versão fixada não ligam o resultado a release rastreável. `curl | sh` do rustup e link de desenvolvimento têm implicações de procedência; não equivalem à verificação de pacote assinado do marketplace.
5. O serviço atual se conecta a um servidor que, no código analisado, escuta `0.0.0.0`; não é adequado para exposição pública sem autenticação e controle de rede. O script sugere ngrok, mas não configura proteção de acesso.
6. `COLLAB_DEV=1` tem comentário sobre `build.rs`; verificar a existência e efeito real antes de depender dele. `cp -r` sobre diretório existente pode manter arquivos antigos.
7. O caminho de atualização ignora a checagem de CLI e a falha de início; o fluxo não comprova que o plugin foi carregado na IDE nem que duas instâncias sincronizam.
8. `menu_pick` e `read` dependem de entrada interativa: o uso via pipe precisa teste de TTY/EOF; não presumir funcionamento não interativo. Testar também cancelamento e terminais sem suporte a `tput`.
9. Caminhos interpolados em unit systemd e XML de launchd exigem tratamento correto de espaços e caracteres especiais; entradas de porta e diretório precisam validação. PATH é modificado em diversos arquivos sem inventário nem reversão.
10. `/tmp/collab-install.log` tem nome fixo e contém saída de ferramentas; usar diretório privado, permissões restritas, limpeza e redação de dados sensíveis.
11. Verificar compatibilidade Linux/systemd de usuário e macOS/launchd em plataformas reais; outros sistemas devem ser explicitamente não suportados até testados. Não inferir que um build em Linux valida macOS ou Windows.

Esses achados são leitura estática, não execução ponta a ponta do instalador. Nenhuma alteração foi feita no script durante esta etapa de planejamento.

## 3. Contrato do novo instalador externo

### Entrada e planejamento

- Oferecer interface interativa e uma interface não interativa documentada com as mesmas opções; falhar claramente em falta de TTY ou dados. Os nomes de flags abaixo são **propostas**, não comandos existentes.
- Escolhas explícitas: modo de conexão, editor, versão/canal, método de instalação, endpoint (se próprio), diretórios e serviço opcional. Identificar OpenIDEA, versão de API, SO/arquitetura, usuário, ferramentas e instalações existentes.
- Mostrar plano prévio: arquivos e serviços a criar/modificar, destino de rede, permissões do plugin e de processo/IPC, procedência, dados transmitidos e riscos. Se uma ação exigir privilégio ou consentimento do host, solicitar de forma explícita; não contornar o controle da IDE.
- Fornecer `--dry-run` sem efeitos e saída legível por máquina que não contenha segredos; opção não interativa deve exigir todos os parâmetros relevantes e não aceitar automaticamente permissões não autorizadas.
- Validar URL, esquema, host, porta, paths, caracteres, acesso ao diretório e versão. Credenciais nunca por argumento de comando ou log; usar fluxo interativo seguro ou armazenamento suportado pelo host.

### Instalação e procedência

- **Preferir pacote de release assinado**: fixar versão, origem, hash e assinatura/chave de publicador; validar por mecanismo oficial do OpenIDEA e confirmar método de instalação disponível. Exibir identidade e versão. Um link de desenvolvimento é modo explícito de desenvolvedor e não prova a mesma cadeia de confiança.
- Identificar que instalação por fora da loja ainda exige consentimento do OpenIDEA e aprovação das capacidades; nunca alterar os controles de sandbox. Se a versão do host não fornecer um comando oficial para instalar, orientar a instalação pela interface e verificar o resultado; não inventar comando.
- Build de fonte é fallback explícito para desenvolvimento: commit/tag fixo e verificado, lockfiles, dependências e ferramentas fixadas, ambiente conhecido; evitar executar scripts baixados diretamente sem inspeção. Não instalar Rust, CLI ou dependências do sistema sem informar e obter consentimento correspondente ao plano de instalação escolhido.
- Auxiliar nativo somente se comprovadamente necessário e suportado: plataforma/arquitetura, assinatura/hash, origem e licença; aprovação explícita, permissões host, diretório privado, execução não privilegiada, ciclo de vida e remoção. Se mecanismo de instalação automática não existir no host, instrução manual clara é fallback, não garantia de instalação silenciosa.
- `gerenciado` e `remoto` não instalam servidor local. No modo local, o servidor deve usar loopback por padrão após implementação de bind configurável. Não escrever shell rc nem serviço por padrão.

### Serviço, atualização e reversão

- Serviço de usuário somente para modo local ou auxiliar se efetivamente necessário, com escolha de iniciar agora e/ou no login; guardar o estado anterior. Unit/plist gerados corretamente para paths com espaços e caracteres especiais. Detectar porta em uso antes de iniciar; não parar outro processo.
- Preparar e verificar artefatos antes de alterar a instalação ativa. Fazer substituição atômica quando possível, manter versão/configuração anterior e restaurá-las, incluindo estado de serviço, em falha. Não apagar o plugin ou configuração de um usuário sem confirmação adequada.
- Atualizar somente componentes escolhidos e gerenciados pelo instalador; preservar endpoint, portas, dados e permissões. Se atualização pedir mais capacidades, deixar o host solicitar consentimento. Não substituir automaticamente artefatos assinados por build dev.
- Registrar um manifesto de instalação sem segredos: versão, hash, arquivos de propriedade do instalador, serviços e alterações feitas. Desinstalar somente o que o manifesto comprova pertencer à instalação; dados e credenciais devem ter opção separada de preservação/remoção. Não apagar Rust, CLI compartilhada, outros plugins, shell config ou arquivos preexistentes.
- Opcional: iniciar/encerrar componente auxiliar com a sessão da IDE, evitando daemon permanente. Não fazer persistência oculta.

### Verificação e diagnóstico

- Verificar separadamente: artefato e assinatura; compatibilidade API; instalação/link; permissões concedidas; disponibilidade e identidade TLS do endpoint; saúde do serviço local quando escolhido; versão de protocolo; e estado efetivo do plugin na IDE. Health endpoint deve ser **implementado e documentado**, e não confundir `GET /rooms` com health.
- Nenhuma verificação automática deve criar sala pública ou divulgar dados. Para comprovar colaboração, fazer teste opt-in com duas instâncias e arquivos descartáveis; health ou build não é teste ponta a ponta.
- Apresentar: instalado / não instalado, autorizado / pendente / negado, serviço saudável / indisponível, funcionalidades disponíveis e passos concretos. Sem afirmar sucesso se o host ainda não carregou o plugin; avisar se reinício for necessário.
- Modo diagnóstico deve sanitizar URLs sensíveis, tokens, convites, conteúdo e nomes de arquivos privados. Arquivos temporários privados, sem seguir symlinks inseguros, e limpeza somente de caminhos controlados pelo instalador.

## 4. Fluxos de instalação para implementação

```text
(A) GERENCIADO
  Verificar IDE + versão → Escolher serviço e editor → Mostrar operador/permissões
  → Instalar pacote verificado → Consentimento da IDE → Verificar endpoint TLS
  → Abrir IDE e criar/entrar por convite → Teste opcional entre duas IDEs

(B) PRÓPRIO REMOTO
  Verificar IDE + versão → Informar URL HTTPS e editor → Verificar identidade TLS
  → Instalar pacote verificado → Consentimento → Autenticar → Teste opcional
  [Servidor já existe; instalação no host remoto é procedimento separado.]

(C) LOCAL PARA DESENVOLVIMENTO
  Verificar IDE → Escolher editor e diretórios → Compilar/verificar binários
  → Configurar bind loopback e porta livre → Escolher serviço opcional
  → Instalar/linkar no modo escolhido → Verificar saúde + permissão → Teste
  [Servidor atual deve primeiro ser alterado: hoje escuta em todas interfaces.]

(D) SOMENTE SERVIDOR EM HOST PRÓPRIO
  No host autorizado: validar SO e versão → Escolher usuário sem privilégios
  → Preparar binário verificado + config TLS/ACL → Serviço opcional
  → Verificar saúde, autenticação, limites e backup → Documentar endpoint.
```

Exemplo **de interface futura, não disponível agora**: `install.sh --mode managed --editor custom`, `install.sh --mode remote --server https://… --editor native` e `install.sh --dry-run`. A sintaxe final deverá ser definida e testada; não executar estes exemplos como se fossem comandos implementados. O guia do usuário deve documentar comandos reais de cada plataforma depois da implementação.

## 5. Guia para o agente executor

1. Ler `AGENTS.md` aplicáveis, este documento, `install.sh`, código do servidor/plugin, testes, versão da IDE e estado Git. Identificar instalações e serviços existentes **sem alterá-los**. Não expor segredos no diagnóstico.
2. Determinar objetivo autorizado (apenas plano, desenvolver instalador ou instalar de fato), modo, editor, host, versão e serviço opcional. Não interpretar este plano como autorização permanente para deploy, download arbitrário, mudança global ou exposição de rede. Se faltarem dados indispensáveis à execução, perguntar somente o necessário e continuar testes independentes.
3. Implementar em fatias: validação/dry-run, seleção e compatibilidade, pacote verificável, configuração por modo, serviço opcional, verificação, atualização/rollback e remoção. Preservar as duas arquiteturas e manter dados existentes.
4. Criar testes em diretórios temporários e executáveis simulados: sem TTY, recusa, usuário cancela, paths com espaços, porta ocupada, falhas em cada etapa, repetição, atualização com config customizada, assinatura inválida e reversão. Testes não devem instalar componentes nem habilitar serviços reais.
5. Verificar cenário real somente no modo e ambiente autorizados; relatar exatamente o que mudou, versões, diretórios e estado. Para sistema operacional não disponível, marcar não testado.
6. Antes de marcar realtime, executar prova em duas IDEs e redes distintas, validar concorrência, reconexão e segurança; medir latência. Se o canal assíncrono, o serviço gerenciado ou o host não estiverem prontos, reportar o bloqueio e não simular sucesso.
7. Não abrir PR, comentar, publicar release, provisionar servidor, mudar DNS ou expor porta sem instrução correspondente; preparar diff e descrição revisável quando apropriado.

## 6. Critérios de aceite por modo

- **Gerenciado:** instalação limpa e convite entre dois clientes sem servidor local/túnel; endpoint real, política de dados, TLS e acesso testados; indisponibilidade tratada.
- **Remoto próprio:** endpoint escolhido é preservado, TLS/credenciais são verificados e nenhum servidor local é instalado; erro claro para endpoint inválido ou não autorizado.
- **Local:** somente quando escolhido; bind loopback, porta validada, serviço opcional e removível, sem ngrok. O binário e os dados são preservados numa atualização que falhe.
- **Todos:** dry-run sem efeitos, caminhos com espaços, idempotência, consentimento de capacidades, rollback, remoção seletiva, logs sem segredo e testes específicos por sistema suportado. Editor escolhido é compatível ou o instalador bloqueia e explica por quê.
- **Realtime:** somente aprovado depois de edição concorrente em duas IDEs, recepção contínua, preservação de dados e métricas reproduzíveis. Editor personalizado sem PR de host é hipótese até teste.
