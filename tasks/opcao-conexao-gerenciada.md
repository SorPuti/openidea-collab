# Segunda opção: conexão gerenciada e editor colaborativo

Data: 09/11/2026. Complementa [o plano original](plan.md) e [a lista de tarefas](todo.md). Preserva a opção self-hosted e os PRs propostos ao OpenIDEA. É uma proposta, não um serviço existente ou uma integração já validada.

## O que o usuário faria

```text
Instalar plugin no OpenIDEA
          │
          ▼
Escolher «Conexão gerenciada» ──► Ver operador, dados e permissões ──► Aceitar
          │
          ▼
     Criar sessão ──► Copiar convite de curta duração ──► Convidado aceita
          │                                                   │
          └──────────────────────┬────────────────────────────┘
                                 ▼
                     Abrir documento colaborativo
                                 │
                                 ▼
                 Editar + ver participantes + reconectar
```

O usuário não instala ngrok, não abre portas nem provisiona um servidor. Os clientes iniciam conexões de saída para um endpoint TLS conhecido. **Alguém precisa operar e pagar esse serviço**, manter o domínio, a capacidade, segurança, suporte e política de dados; o serviço não está provisionado. Quando fora do ar, a opção gerenciada não funciona; erro e recuperação devem ser claros, sem mudar automaticamente para outro servidor.

## Dois eixos independentes

```text
                    COMO CONECTAR
            ┌────────────────────────────┐
            │ A. Servidor próprio        │  ← plano original preservado
            │ B. Serviço gerenciado      │  ← nova opção sem túnel manual
            └──────────────┬─────────────┘
                           │
                           ▼
                    COMO EDITAR
            ┌────────────────────────────┐
            │ 1. Editor nativo           │  ← APIs upstream se necessárias
            │ 2. Editor personalizado    │  ← possibilidade na API atual,
            │    do plugin               │     requer prova de viabilidade
            └────────────────────────────┘
```

As combinações devem ser avaliadas de forma independente. O servidor próprio pode operar com o editor personalizado; o serviço gerenciado também pode servir ao editor nativo. A opção gerenciada elimina o túnel do usuário, mas **não elimina por si só** as limitações de edição ou de entrega de eventos do plugin.

## Arquitetura da nova opção

```text
  PESSOA A                INFRAESTRUTURA GERENCIADA             PESSOA B
┌──────────────────┐       ┌──────────────────────────┐       ┌──────────────────┐
│ OpenIDEA         │ saída │ TLS + autenticação       │ saída │ OpenIDEA         │
│ ┌──────────────┐ │──────►│ Convites e acesso        │◄──────│ ┌──────────────┐ │
│ │ Editor       │ │       │ Sessões / relay          │       │ │ Editor       │ │
│ │ colaborativo │ │       │ Estado e recuperação     │       │ │ colaborativo │ │
│ └──────┬───────┘ │       │ Quotas e monitoramento   │       │ └──────┬───────┘ │
│        ↕         │       └──────────────────────────┘       │        ↕         │
│ Plugin + CRDT*   │                                          │ Plugin + CRDT*   │
│ Canal de rede*   │                                          │ Canal de rede*   │
└──────────────────┘                                          └──────────────────┘

* Local e canal sujeitos à prova do host: plugin WASM ou processo auxiliar autorizado.
  Uma webview de plugin não tem acesso direto à rede no contrato consultado.
```

O endpoint gerenciado permite escopo de permissão de rede explícito e conhecido, sujeito às regras de concessão do OpenIDEA. Não pressupor que uma permissão permita automaticamente qualquer URL configurada. O serviço usa TLS, autenticação e autorização por sala; TLS protege o trajeto, mas **não é E2EE**: o operador pode ter acesso ao conteúdo conforme o desenho do servidor. Documentar isso antes do usuário aceitar. E2EE é uma evolução separada sujeita a desenho e revisão.

## Possibilidade sem PR para a IDE: editor personalizado

A [documentação do OpenIDEA v0.1.40](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#views-and-custom-editors) descreve um editor personalizado com o mesmo documento da IDE e histórico compartilhado. A [interface web](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#web-views) fornece `document.get`, `document.edit` com versão e `document.onChange` para mudanças feitas em outros lugares. A página é isolada, sem rede; mensagens com o plugin existem, mas **a documentação citada não prova que esse caminho fornece entrega remota contínua enquanto o plugin estiver inativo ou bloqueado**.

```text
Digitação local ─► Editor personalizado ─► documento da IDE
                           │                   ▲
                           ▼                   │
                   Plugin / CRDT* ───────► edição versionada
                           ↕
                Rede / serviço gerenciado*

* A comunicação contínua webview ↔ plugin ↔ rede é a principal hipótese a testar.
```

O editor personalizado é uma experiência diferente do editor nativo: avaliar realce, atalhos, acessibilidade, conclusão/LSP, múltiplos cursores, desempenho, seleção, undo e múltiplas janelas. Não prometer equivalência antes de testar. Abrir por **Open With** deixa a escolha explícita. Usar arquivos empacotados e assinados; não carregar código remoto nem escapar da CSP/sandbox. Validar licença de qualquer componente de edição.

### Candidato de entrega assíncrona: PTY de webview

A API [Web frame PTYs 1.3](https://github.com/l0ng-ai/openidea/blob/37c2a6048eea89d4f6db645622612358b643862b/plugins/README.md#web-frame-ptys-api-13) é um candidato concreto para canal direto entre editor web e auxiliar aprovado, com permissões de processo, controle de fluxo e ciclo de vida. Não foi testada para sincronização; exige prova de framing, segurança, instalação, recursos e desempenho. Há exemplos e limites em [snippets-referencia.md](snippets-referencia.md). Ela não garante integração no editor nativo.

### Prova de viabilidade — antes de comprometer a arquitetura

1. Construir editor mínimo com duas janelas reais no OpenIDEA v0.1.40: digitação local, `onChange` externo, `document.edit`, versão obsoleta, save e undo.
2. Fazer um evento remoto atravessar a rede e chegar ao editor sem modificação do host, medindo latência e comprovando entrega enquanto a IDE/plugin fica ocioso. Verificar budgets, serialização e ciclo de vida com WIT/host e teste real.
3. Testar duas máquinas, desconexão, reconexão, atualizações concorrentes, Unicode e divergência inicial; verificar que alterações não são perdidas.
4. Se a etapa 2 não for possível via APIs públicas e permissões, registrar o bloqueio. Prosseguir com PR upstream ou oferecer apenas sincronização ao salvar; não alegar realtime nem contornar isolamento.

## Instalação externa para todas as opções

O [plano de instalação customizada](instalacao-customizada.md) inclui fluxos gerenciado, servidor próprio remoto, servidor local e somente servidor, além da escolha de editor, validação, atualização e remoção. O instalador atual ainda não oferece esses fluxos.

## Instalação automática — condicional

A opção preferida é um pacote completo, quando tecnicamente permitido, ou conexão via APIs oficiais existentes. Se um auxiliar nativo for indispensável, verificar primeiro se o OpenIDEA permite sua distribuição/instalação e execução. Usar somente consentimento explícito e as permissões do host; mostrar origem, finalidade e versão. Verificar assinatura e integridade, instalar em diretório apropriado, suportar atualizações seguras, desinstalação e recusa. Não executar instaladores remotos arbitrários, solicitar privilégios de administrador sem necessidade ou iniciar serviço persistente oculto. Dependendo do host, uma etapa do usuário ainda pode ser necessária: instalação inteiramente automática não é garantida.

## Segurança, operação e custo

- TLS, credenciais aleatórias, expiração e revogação de convites; controle por sala e papel, isolamento de locatários e limites de recursos.
- Não registrar conteúdo nem tokens; armazenamento de segredos apropriado, criptografia em repouso quando aplicável, backup testado, retenção e exclusão documentadas.
- Transparência sobre operador, região, custo, política de privacidade, disponibilidade, exportação/migração e encerramento. Sem decisão de operador e orçamento, o produto não pode prometer serviço gerenciado gratuito ou permanente.
- Sem dependência de ngrok ou túneis temporários; a opção self-hosted continua disponível e pode usar outra infraestrutura escolhida pelo administrador.
- Desempenho é meta a medir: registrar p50/p95 edição-a-renderização, reconexão, CPU e memória em LAN, internet e rede degradada; publicar metodologia e resultados. Um relay próximo pode reduzir latência, mas não há garantia antes do teste.
- Uma alternativa P2P com relay de fallback é pesquisa futura: NAT/firewalls podem impedir conexões diretas; não prometer que P2P sempre funcionará. [Referência sobre tipos de conexão e fallback](https://tailscale.com/docs/reference/connection-types).

## Repositórios e PRs

```text
SorPuti/openidea-collab
 ├─ PRs: editor personalizado, experiência de conexão, CRDT e cliente
 ├─ PRs: servidor/relay open source, autenticação, operação e testes
 └─ Release assinada + documentação + licença de dependências
             │
             ├──► l0ng-ai/openidea: PRs originais continuam opcionais para
             │    esta trilha, mas necessários se a prova apontar lacunas
             │    ou para integração plena no editor nativo.
             │
             └──► l0ng-ai/openidea-plugins: atualizar PR #102 com o pacote
                  real e justificar as permissões; aprovação independente.
```

O serviço hospedado é infraestrutura de operação separada dos PRs de código e não é criado por este plano. Não há necessidade de PR upstream se, e somente se, a prova demonstrar a funcionalidade desejada usando a API atual; o marketplace continua sujeito às suas regras e revisão.

## Aceite da segunda opção

- Dois usuários em redes distintas instalam somente o plugin e aceitam permissões; entram por convite sem instalar ngrok, abrir portas ou configurar servidor.
- Caso haja auxiliar, instalação e inicialização são autorizadas, verificadas, reversíveis e testadas em cada plataforma suportada.
- Edição simultânea, seleção, undo, reconexão e persistência funcionam nos cenários documentados; incompatibilidades do editor personalizado são claras.
- Operador, política de privacidade, retenção, custos e ausência ou presença de E2EE são explícitos; acesso indevido e revogação são testados.
- Latência é medida, não prometida; indisponibilidade do serviço não causa perda silenciosa ou troca de destino sem consentimento.
