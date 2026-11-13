# Evidências executáveis dos snippets

Executar na raiz do projeto; nenhuma prova instala plugin, executa helper real ou provisiona servidor. Resultados de 09/11/2026.

| Seção em snippets-referencia.md | Prova realizada | Resultado e limite |
|---|---|---|
| 1 manifesto | `python3 tasks/provas/host_e_servidor.py` | `openidea-plugin check` aceitou cópia do pacote 0.2.0 com custom editor e host fixo de teste. Não abriu editor nem testou onChange/undo. |
| 1 bridge de editor | `node --test tasks/provas/contratos.test.mjs` | Teste com mock confere passagem de stale sem retry. Não é host real nem verificação de UI. |
| 2 PTY | mesmo teste Node | Mock testa ACK depois do consumo, separação de sessão e falha. Não inicia processo, PTY, IDE, framing ou rede; spawn/input/kill ficam pendentes. |
| 3 manifesto de rede | prova `host_e_servidor.py` | CLI aceitou `net:collab.example.org`; não prova consentimento ou acesso real, tampouco grants para URL arbitrária. |
| 4 health e bind | `host_e_servidor.py` | Rota 204 testada por requisição local em cópia temporária. Alteração de bind compila; teste usa listener criado pelo teste e não verifica bind do binário principal. |
| 5 Yrs | comando Cargo abaixo | Yrs 0.28.0 compila, UTF-16 para A😀B e convergência de dois documentos em memória passaram. Sem WASM, rede, recuperação, persistência ou integração de editor. |
| 6 protocolo | não executável, pseudocódigo | Pendente especificação, autenticação, implementação e testes E2E. |
| 7 instalador | não executável, pseudocódigo | Pendente implementação; atual `install.sh` não fornece os modos. |
| 8 referências upstream | links documentais | Não é código executável; aprovação e release não foram solicitadas. |

```sh
node --test tasks/provas/contratos.test.mjs
CARGO_TARGET_DIR=/tmp/openidea-yrs-proof-target cargo test --manifest-path tasks/provas/yrs/Cargo.toml --locked --offline
python3 tasks/provas/host_e_servidor.py
```

O script Python exige `openidea-collab/dist/plugin.wasm`, `openidea-plugin` no PATH e dependências do servidor no cache Cargo; usa temporário e não altera código do produto. O teste Cargo do Yrs exige dependências no cache (a primeira execução baixou a versão 0.28.0). As provas locais simulam exclusivamente as fronteiras documentadas. Para demonstrar o produto, faltam testes de editor e PTY na IDE real, conexões reais, autenticação, dados e testes de segurança.
