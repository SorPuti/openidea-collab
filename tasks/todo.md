# Lista de execução — OpenIDEA Collab

A numeração define a ordem; divisão dos PRs pode mudar após feedback upstream. Cada item é verificável e pode ser quebrado em issues menores quando implementado.

## Diagnóstico e base

- [ ] **1. Reproduzir fluxo existente.** Dep.: nenhuma. Aceite: registrar versão da IDE, permissões, resultados de criar/entrar, salvar e receber em duas instâncias com arquivos descartáveis; incluir log sem segredos. Verificação: reprodução repetível e evidência de falhas.
- [ ] **2. Corrigir testes do plugin.** Dep.: nenhuma. Aceite: testes de presença e serialização atualizados com comportamento significativo. Verificação: `cargo test --manifest-path openidea-collab/Cargo.toml` passa.
- [ ] **3. Alinhar manifesto, release e documentação.** Dep.: 1. Aceite: permissões reais, versões e licença MIT consistentes; remover alegações não provadas e explicar diferenças entre 0.1.0 e código atual. Verificação: build, check do pacote, revisão cruzada de README e release.
- [ ] **4. Planejar situação do PR #102.** Dep.: 3. Aceite: texto corrigido e escolha explícita entre atualizar a release/entrada ou manter como prévia limitada; sem criar alegação de funcionalidade ausente. Verificação: comparar package, hash, licença e checklist. Publicação/comentários externos exigem instrução do usuário.

## API e arquitetura

- [ ] **5. Provar lacunas de API.** Dep.: 1. Aceite: exemplo mínimo demonstra eventos de edição, editor não focado e entrega de rede; separar indisponível de não testado. Verificação: testes contra versão fixada do OpenIDEA.
- [ ] **6. Preparar proposta upstream.** Dep.: 5. Aceite: contrato e testes propostos para identidade/versão/origem, mudança de documento, seleção, aplicação, segurança, compatibilidade e ciclo de vida. Verificação: demonstração vinculada à proposta e revisão das regras do upstream.
- [ ] **7. Validar transporte.** Dep.: 5. Aceite: comparar host nativo e sidecar com rede, IPC, ciclo de vida, permissões e entrega assíncrona; escolher apenas alternativa demonstrada. Verificação: protótipo com fluxo bidirecional e revogação.
- [ ] **8. Contribuir API upstream.** Dep.: 6–7 e entendimento dos mantenedores. Aceite: WIT/SDK/host e documentação coerentes, sem acesso excessivo; PRs pequenos e rastreados. Verificação: testes do OpenIDEA, compatibilidade e revisão; disponibilidade depende de merge e release.

## Fatias de produto

- [ ] **9. Definir protocolo de sessão e autenticação.** Dep.: 7. Aceite: esquema versionado, convite, papéis, revogação e limites escritos; IDs não são segredos. Verificação: testes de contrato e negação de acesso.
- [ ] **10. Provar CRDT e snapshot.** Dep.: 5. Aceite: escolher motor após teste de WASM ou sidecar, licenças, Unicode e concorrência; sincronizar diferenças e rejeitar estado inicial divergente sem decisão. Verificação: testes de convergência com duplicação/reordenação.
- [ ] **11. Editar um documento entre duas IDEs.** Dep.: 8–10. Aceite: alterações locais e remotas no buffer correto, controle de eco e undo testado. Verificação: teste real de duas IDEs com edição concorrente.
- [ ] **12. Recuperar após interrupção.** Dep.: 11. Aceite: reconexão e restart recuperam estado suportado; decisões e falhas aparecem ao usuário. Verificação: testes automatizados de interrupção, snapshots e persistência.
- [ ] **13. Presença e interface.** Dep.: 11. Aceite: participantes, conexão, cursor/seleção e saída com heartbeat, expiração e acessibilidade. Verificação: duas IDEs e casos de permissões negadas.
- [ ] **14. Multiarquivo e limites.** Dep.: 12. Aceite: escopo explicitamente escolhido, exclusões, rename/delete e arquivos não suportados têm comportamento seguro e documentado. Verificação: testes de caminhos, Unicode, arquivos grandes, binários e conflitos.

## Preparação para lançamento

- [ ] **15. Endurecer servidor.** Dep.: 9, 12. Aceite: loopback padrão, TLS remoto, ACL, revogação, limites, logs sem conteúdo e política de dados. Verificação: testes adversariais e revisão de ameaças.
- [ ] **16. Criar governança open source.** Dep.: 3. Aceite: contribuição, conduta, segurança, manutenção, changelog, suporte, inventário de licenças/avisos e documentação de operação. Verificação: seguir guia a partir de ambiente limpo.
- [ ] **17. CI e distribuição.** Dep.: 2, 3, 15–16. Aceite: build e testes de ambos componentes, WASM, `check`, licenças e pacote assinado vinculado a tag/commit; segredos protegidos. Verificação: release candidata e instalação limpa.
- [ ] **18. Beta e marketplace.** Dep.: 8, 11–17. Aceite: matriz de compatibilidade preenchida, teste com participantes, limitações publicadas, métricas medidas, artefato e hash corretos. Verificação: instalar e testar pacote da release; revisão/merge do registro depende de mantenedores.

## Checkpoints

- Depois de 1–4: base reproduzida e alegações alinhadas com o artefato.
- Depois de 5–8: capacidades e rota de implementação comprovadas ou bloqueio externo documentado.
- Depois de 9–14: teste de edição concorrente, reconexão e segurança com evidências.
- Depois de 15–18: beta rastreável, documentação e revisão de comunidade.

## Nova opção: conexão gerenciada e editor sem alteração da IDE

As tarefas anteriores permanecem válidas. Essa trilha não depende do merge do item 8 **se** a prova do editor personalizado e da entrega contínua for bem-sucedida; conexão gerenciada e escolha de editor são decisões separadas.

- [ ] **19. Provar editor personalizado existente.** Dep.: 1, 5. Aceite: abrir arquivo por Open With, receber edição local e alterações externas, aplicar `document.edit` com versão e demonstrar undo/save sem perda. Verificação: duas janelas reais no OpenIDEA alvo, incluindo conflitos de versão e Unicode; documentar diferenças do editor nativo.
- [ ] **20. Provar canal contínuo sem alterar host.** Dep.: 19. Aceite: avaliar PTY webview com helper autorizado conforme [snippets](snippets-referencia.md) e demonstrar como update remoto chega ao editor por APIs públicas, sem acesso de rede da webview nem polling lento; medir limites de chamada, ciclo de vida e isolamento. Verificação: dois clientes em rede real e teste de interrupção. Caso não seja possível, registrar bloqueio sem contornar sandbox; manter a trilha upstream.
- [ ] **21. Definir operação do serviço gerenciado.** Dep.: 9. Aceite: identificar operador, domínio, região, custos, orçamento, capacidade, manutenção, SLO, retenção, suporte, encerramento e migração/exportação; servidor open source também executável por terceiros. Verificação: documento operacional e teste de restauração. Sem operador e serviço provisionados, não anunciar disponibilidade.
- [ ] **22. Sessão de um clique.** Dep.: 20–21. Aceite: instalação do pacote, consentimento de rede e opção Criar/Entrar; convite expira, é revogável e não revela credencial nos logs. Verificação: instalação limpa em duas máquinas, sem ngrok, configuração de portas ou servidor pelo usuário.
- [ ] **23. Instalação auxiliar opcional e segura.** Dep.: 20, somente se necessária e permitida pelo host. Aceite: aprovação explícita, plataforma compatível, procedência, assinatura/hash, localização privada, atualizações e remoção; recusa não causa instalação oculta. Verificação: teste em ambiente limpo, artefato adulterado, recusa, atualização, desinstalação e autorização pelo host. Se host não suportar o fluxo, documentar dependência e não prometer instalação automática.
- [ ] **24. Segurança do serviço gerenciado.** Dep.: 21–22. Aceite: TLS, identidade, convite, papéis, isolamento entre salas, quotas, revogação, criptografia em repouso conforme arquitetura, segredos em armazenamento seguro e logs sem conteúdo; declarar acesso do operador e ausência de E2EE até auditada. Verificação: revisão de ameaças e testes de isolamento, roubo de convite e expiração.
- [ ] **25. Desempenho e resiliência de rede.** Dep.: 20, 22, 24. Aceite: teste com relay em rede LAN, internet e conexões ruins; métricas p50/p95 de edição-a-renderização, reconexão, CPU, memória e perda. Verificação: metodologia, infraestrutura, dados e resultados reproduzíveis publicados; metas definidas antes do beta, sem garantia de velocidade antecipada.
- [ ] **26. Escolha, fallback e privacidade.** Dep.: 22–24. Aceite: interface mostra serviço gerenciado ou servidor próprio, versão do editor, dados enviados, operador e status; fallback não troca destino sem consentimento. Verificação: testes de UI, indisponibilidade e troca de servidor sem perda de alterações não sincronizadas.
- [ ] **27. Licenças e revisão do pacote.** Dep.: 19–24. Aceite: verificar licença do editor embutido, motor CRDT, sidecar e dependências; publicar fontes, avisos e justificativas das capacidades, com build rastreável. Verificação: inventário automatizado/manual e `openidea-plugin check`; PR do registro continua sujeito à revisão.

## Instalador externo por modo

Detalhes, evidências da leitura do script e procedimentos para o agente: [instalação customizada](instalacao-customizada.md). Essas tarefas são planejamento; flags propostas não existem hoje.

- [ ] **28. Inventariar e testar instalador atual.** Dep.: 1. Aceite: testes reproduzem seleção, atualização, falha de build, porta e verificação sem instalar serviços reais; registrar plataformas não testadas. Verificação: suíte com diretórios temporários e mocks.
- [ ] **29. Selecionar modo e planejar instalação.** Dep.: 28. Aceite: escolhas gerenciado, remoto próprio, local e editor; checagem de versão, consentimento e `dry-run` sem efeitos. Verificação: testes interativos e sem TTY, incluindo caminhos com espaços e modo incompatível.
- [ ] **30. Instalar pacote verificável.** Dep.: 29. Aceite: release rastreável, assinatura/hash e fluxo oficial do host; modo dev explícito; sem instalar servidor no modo cliente. Verificação: pacote válido/adulterado, consentimento negado e instalação limpa.
- [ ] **31. Configurar servidor opcional.** Dep.: 29, 15. Aceite: local só por escolha, bind loopback, porta validada e serviço opcional; servidor remoto instalado apenas no host autorizado. Verificação: porta ocupada, permissões e caminhos com espaços; testes systemd/launchd onde disponíveis.
- [ ] **32. Tornar atualização reversível.** Dep.: 30–31. Aceite: build/verificação antes de parar serviço, preservação de config customizada e rollback de falha. Verificação: simular falha em cada etapa e confirmar binário, config e estado de serviço anteriores.
- [ ] **33. Verificar e diagnosticar por modo.** Dep.: 30–31 e health implementado. Aceite: checks distintos de assinatura, instalação, concessão, endpoint e saúde; não declarar realtime apenas com health. Verificação: endpoint ausente, TLS inválido, permissão negada e plugin não carregado.
- [ ] **34. Desinstalar seletivamente.** Dep.: 30–32. Aceite: manifesto identifica apenas recursos do instalador; dados e credenciais têm escolha separada; dependências compartilhadas ficam intactas. Verificação: instalação prévia de terceiros, remoção repetida e preservação de dados.
- [ ] **35. Testar experiência completa de instalação.** Dep.: 19–27 e 29–34 conforme cada modo. Aceite: matriz por modo, editor e SO inclui teste real em duas IDEs quando suportado, sem túnel manual no gerenciado. Verificação: documentar sucesso, falhas e plataformas não testadas, sem afirmar disponibilidade antecipada.

## Contratos comuns e evidências

- [ ] **36. Especificar adaptadores de produção.** Dep.: 5, 7, 19–20. Aceite: contratos de editor, transporte, sessão, segurança, versão, erro e cancelamento conforme [matriz](arquitetura-ownership.md). Verificação: testes de contrato em implementações reais, sem fallback silencioso. O protótipo JS em `provas/` é simulado e não é o produto.
- [ ] **37. Validar as quatro combinações.** Dep.: 8 ou prova custom, 9–15, 19–27, 36. Aceite: managed/self-hosted × native/custom com o mesmo protocolo quando suportado; detectar versões e capacidades e falhar claramente. Verificação: matriz E2E com IDEs reais, concessões, instabilidade e segurança; configuração não cria adaptador ou API faltante.
- [ ] **38. Provar snippets pendentes no host real.** Dep.: 19–20. Aceite: editor custom, stale, undo, PTY, encerramento e concessão testados na IDE, incluindo rede real e segurança. Verificação: registro reproduzível e limites; pseudocódigo de protocolo/instalador permanece pendente até implementar e testar.

## Continuação: WebSocket, instalação e segurança

- [ ] Executar as etapas 01–18 de [plano-collab-websocket-seguranca.md](plano-collab-websocket-seguranca.md), respeitando dependências, checkpoints e testes; registrar conclusão de cada etapa com evidência no próprio plano.
