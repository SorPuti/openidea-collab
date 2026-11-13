# Hospedagem gratuita e conexão sem auxiliar separado

Pesquisa em 10/11/2026. Objetivo atualizado: colaboradores instalam somente o plugin OpenIDEA; não instalam auxiliar, VPN, túnel ou servidor. O operador configura uma vez o servidor e domínio; os colaboradores usam convite. Manter servidor próprio como opção, mas não depender de Cloudflare Tunnel ou ngrok. **Esta é uma meta, ainda não há implementação nem prova de que a versão atual da IDE a suporte.**

```text
Máquina A                                            Máquina B
OpenIDEA + plugin                                   OpenIDEA + plugin
        │                                                  ▲
        └───────── WSS autenticado ─► relay ── WSS ─────────┘
                              collab.seu-dominio
                              HTTPS/TLS + persistência
```

## Hospedagens pesquisadas

### 1. Oracle Cloud Always Free — principal candidata para o servidor Rust existente

[Always Free](https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm) prevê VMs gratuitas limitadas na região inicial e armazenamento/rede com quotas, inclusive instâncias Ampere ARM. A VM pode operar o servidor e um proxy TLS e ter um domínio do operador apontado ao IP, sujeito à configuração e à comprovação prática. Pode não haver capacidade livre na região; ver condições de cadastro e cobrança antes de criar qualquer recurso. Um domínio próprio pode ter custo, independente da VM. Verificar recursos/tipos Always Free exatos no momento do provisionamento e evitar upgrade, serviços pagos ou excedentes sem consentimento. A oferta não garante disponibilidade permanente, e conta/rede, manutenção, atualizações, backups, privacidade, antiabuso e observabilidade são responsabilidade do operador.

**Encaixe técnico necessário:** a API realtime atual se recusa a fazer bind não loopback. Na VM usar proxy reverso TLS com suporte WebSocket e limites sobre `127.0.0.1:<porta>`, sem reduzir essa proteção; confirmar processo correto, health checks, timeouts, WebSocket upgrade, autenticação e persistência durable do journal. A versão atual ainda não implementa WSS, projeto inteiro, fila durável/reconnect completo ou revisão para exposição pública. Porta 443 e DNS serão configurados pelo operador, não pelo convidado.

**Provas pendentes:** região próxima ao usuário, capacidade real, arquitetura ARM/x86, build reproduzível, persistência após reinício, backup/restore, TLS, limites e medição das duas cidades. Não há VPS provisionada ou latência comprovada.

### 2. Render Free — prova de conceito rápida, não operação confiável

O [Render Free](https://render.com/docs/free) documenta domínios próprios, TLS, WebSocket e 750 horas por mês de instância gratuita por workspace. Apaga dados locais (incluindo SQLite) após restart/redeploy/spindown; instância dorme após 15 minutos sem tráfego recebido, inclusive mensagens WS; próxima conexão pode levar cerca de um minuto para acordar. Sem disco persistente gratuito; banco gratuito expira em 30 dias. O próprio provedor diz para não usar o Free em produção. Revisar condições atuais, limite de rede e cobranças/excedentes antes de provisionar. A documentação de tutorial do provedor conflita sobre domínios no Free; confirmar elegibilidade efetiva na conta antes de prometer suporte. Não usar armazenamento local efêmero como ACK durável. Evitar como alvo de desempenho rápido entre sessões.

### 3. Koyeb Free — alternativa de laboratório

A [documentação de instâncias](https://www.koyeb.com/docs/reference/instances) lista 512 MB RAM, 0,1 vCPU, uma instância por organização, apenas Frankfurt ou Washington, sem volumes e scale-to-zero depois de uma hora. Diz não utilizar em produção. [Scale-to-zero](https://www.koyeb.com/docs/run-and-scale/scale-to-zero) documenta limitações de conexão que acorda o serviço. A [página de preços](https://www.koyeb.com/pricing) menciona domínios próprios e HTTPS; confirmar disponibilidade do domínio no plano gratuito efetivo, limites, condições da conta e custo do tráfego. Falta persistência comprovada. Latência para Brasil deve ser medida; não inferir que será rápida.

### 4. Deno Deploy Free — alternativa exige prova e possível novo servidor

[Preços publicados](https://deno.com/deploy/pricing) incluem uma camada Free com limites de solicitações, tráfego e domínios. A [documentação de domínios](https://docs.deno.com/deploy/reference/domains/) prevê DNS e certificados. Deno documenta WebSocket em seu [runtime](https://docs.deno.com/runtime/fundamentals/http_server/), mas isso não comprova que o servidor Rust/Axum existente rode no Deploy ou que WebSockets duradouros com estado compartilhado e armazenamento persistente estejam disponíveis nas condições necessárias. Fazer PoC com versão atual, limite de vida/conexão, reconexão, identidade de instância e persistência antes de considerá-lo. Se exigir reimplementação, manter o mesmo protocolo e suíte de segurança; não presumir portabilidade nem velocidade.

## Bloqueio real: WebSocket sem auxiliar no host atual

No código atual da IDE [o frame customizado é isolado sem rede](https://github.com/l0ng-ai/openidea/blob/3e3088c525660c457518631dce6cf679340235c7/plugins/README.md#L1489-L1507); a documentação descreve `host::fetch` e `net:ask`, mas não comprova WebSocket persistente no sandbox. O Collab experimental usa processo nativo concedido. Logo: trocar a hospedagem **não remove** o requisito local de transporte por si só. Não ligar sockets diretos ao iframe nem afrouxar CSP/permissões como atalho.

Trabalhar nesta ordem:

1. Pesquisar no SDK/WIT/host **da versão da IDE alvo** se há socket WSS com recebimento assíncrono e grant limitado ao domínio. Demonstrar teste em plugin instalado, sem processo externo, incluindo projeto ocioso, reload, revogação e troca de projeto.
2. Se não existir, preparar PR genérico no `l0ng-ai/openidea` para API WebSocket baseada em host: `net:<domínio>`/modelo de grant existente quando aplicável, origem e TLS validados, cabeçalhos seguros sem segredos em logs, limites de conexão e frames, backpressure, ping/close, revogação imediata, isolamento por plugin/projeto, lifecycle e testes adversariais. Verificar compatibilidade com rede remota e políticas da plataforma; aguardar aceite e versão lançada da IDE.
3. Se upstream não aceitar ou versão instalada ainda não tiver a API, alternativa explicitamente comunicada é usar **auxiliar aprovado** na prévia ou HTTP suportado pelo host onde comprovadamente viável. Isso não satisfaz a meta de 'sem auxiliar' e não será anunciado como tal. HTTP polling não é equivalente automático de uma conexão WSS nem garante realtime.
4. Na versão compatível: endpoint gerenciado configurado no manifesto/serviço por padrão, onboarding e convite com credencial por membro, TLS obrigatório, escopo explícito e revogação, sem instalação de sistema pelo usuário. O usuário ainda instala o próprio plugin e autoriza capacidades solicitadas; nenhuma instalação separada, servidor ou túnel local.

## Entregas e critérios adicionais

- [ ] PoC de API WSS na IDE sem auxiliar ou PR com testes do host; identificar versão mínima e decisão do mantenedor. Não contornar sandbox.
- [ ] Serviço Rust seguro: WSS por membro, autorização em cada operação, políticas por projeto, exclusões, limitações, persistência, backup/restore e integridade; tratar crash e repetir/reordenar mensagens com ACK durável.
- [ ] Selecionar Oracle apenas após verificar conta e disponibilidade, ou hospedar prova não sensível com Render/Koyeb sem alegar durabilidade. Configurar domínio escolhido pelo operador uma vez; sem configuração dos colaboradores.
- [ ] Medir p50/p95/p99 edição→render, CPU/RAM e perda/reconexão entre cidades, e comparar regiões. Nenhuma opção foi medida neste levantamento; rapidez não é garantida por ser gratuita.
- [ ] Testar duas IDEs instalando apenas pacote assinado pelo marketplace, conexão/convite/escopo, edição simultânea, troca de projetos, ciclo de vida, recuperação, revogação, ausência de binário/processo adicional e instalação em sistema limpo.
- [ ] Reavaliar revisão do [PR #102](https://github.com/l0ng-ai/openidea-plugins/pull/102#issuecomment-6756170352), licença, versão e origem do pacote; registrar evidências e aguardar aprovação humana.

## Direção adicional: relay leve sem armazenamento central de documentos — 10/11/2026

O usuário solicitou estudar um serviço público otimizado que retransmita dados, sem cache de documentos no servidor. A conta/VM ainda não foi criada: o navegador desta sessão retornou `browser access host is unavailable`. Não existem evidências de cadastro, endereço, serviço, domínio, WSS ou implantação em nuvem.

```text
Editor A → fila local durável → WSS → RELAY sem cache de conteúdo → WSS → fila local durável → Editor B
                                     ├ ACL/membro/revogação
                                     ├ limites/backpressure
                                     └ métricas sem texto/segredos
```

**Não confundir retransmissão com durabilidade:** um relay sem persistência de conteúdo não pode prometer armazenar mudanças quando todos os participantes estiverem desconectados, nem confirmar que a alteração está salva. O protocolo deve diferenciar `recebido pelo relay` de `gravado pelo destinatário`; só apresentar a entrega confirmada após ACK do destinatário na durabilidade acordada. O emissor mantém alterações em fila local durável e limitada até confirmação ou opção explícita de recuperação; usuários offline por muito tempo exigem tratamento claro (fila cheia, reenvio, expiração e permanência do emissor). Evitar descartar silenciosamente e preservar uma cópia local em caso de encerramento. Se todos perderem suas cópias ou quiserem entrar depois do emissor sair, recuperação de conteúdo é impossível sem armazenamento alternativo autorizado; expor essa restrição no produto.

A identidade e autorização por membro continuam necessárias; não guardar documentos no relay não dispensa dados mínimos de autenticação, revogação, quotas, métricas e configuração. Projetar sem logs de conteúdo ou segredos; TLS e WSS, autenticação por membro, autorização por documento e ação, convites de uso único, isolamento de sala/projeto, política de acesso, limites, backpressure, revogação, contenção de abuso e testes de falha. TLS protege trânsito até o servidor; confidencialidade contra operador exige desenho e auditoria de criptografia ponta a ponta específica, que ainda não foi provada. Testar restart, reconexão, duplicação, perda e alteração simultânea. A demanda por projeto inteiro, seleção segura de arquivos, links e conflitos continua aplicável.

**Correção de capacidade:** a [Oracle](https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm) informa atualmente 1.500 OCPU-h e 9.000 GB-h/mês, equivalentes a 2 OCPUs/12 GB em tenancy Always Free; não assumir quatro OCPUs contínuos. Há possibilidade de ausência de capacidade e regras de elegibilidade; não contratar recursos pagos automaticamente. Os recursos reais, região, rede e desempenho precisam ser verificados no cadastro.

**Próximos passos:** restaurar acesso ao navegador para cadastrar conta usando autenticação e verificação realizadas pelo usuário quando necessário; identificar termos e confirmar custos na tela; escolher recursos elegíveis de forma revisável. Trabalhar o protocolo e avaliar API WebSocket do host antes de implantar algo acessível publicamente. Validar localmente e com duas máquinas, inclusive persistência do cliente e segurança, antes de divulgar um endpoint.
