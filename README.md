# Compartilhamento de tela P2P

Site para compartilhar a tela com até 5 amigos ao mesmo tempo, direto do navegador
(Windows e Linux) — sem instalar nada, sem contas, sem áudio/chat. O vídeo trafega
P2P via WebRTC; o servidor só faz a sinalização inicial (troca de offer/answer/ICE).

## Rodando localmente

Pré-requisitos:

- Rust + `rustup target add wasm32-unknown-unknown`
- `cargo install cargo-leptos`

```bash
cargo leptos watch
```

Abra `http://127.0.0.1:3000/`.

## Testes automatizados

```bash
cargo test --features ssr
```

Cobre a lógica de sinalização (protocolo, registro de salas, endpoint WebSocket).
A captura de tela e o handshake WebRTC só existem dentro de um navegador real —
são validados manualmente (checklist abaixo).

## Checklist de teste manual (fluxo completo)

1. Abra `/` numa aba, clique "Iniciar compartilhamento", escolha uma janela/tela.
2. Confirme que aparece um link `/r/<código>`.
3. Abra esse link em outra aba (ou outra máquina) — confirme que o vídeo aparece
   em poucos segundos.
4. Abra o mesmo link numa terceira aba — confirme que ambos os espectadores
   recebem o vídeo simultaneamente.
5. Feche a aba de um espectador — confirme que os demais continuam recebendo
   vídeo normalmente.
6. Pare o compartilhamento (feche a aba de quem compartilha) — confirme que os
   espectadores restantes veem "O compartilhamento foi encerrado."
7. Abra um link com um código inexistente — confirme "Sessão não encontrada ou
   já terminou."

## Deploy no Fly.io

O projeto já vem pronto pra isso: `Dockerfile` (build multi-stage: compila com
`cargo-leptos --release`, imagem final só com o binário + assets) e
`fly.toml` já configurado (porta fixa 8080, região `gru` — São Paulo,
`shared-cpu-1x` / 256MB, e as máquinas ficam paradas quando ninguém está
usando — `auto_stop_machines`/`min_machines_running = 0` — pra não consumir
saldo à toa). Testado localmente com `docker build` + `docker run` antes de
configurar.

1. Instale o `flyctl` (se ainda não tiver):
   ```bash
   curl -L https://fly.io/install.sh | sh
   ```
2. Faça login (abre o navegador):
   ```bash
   fly auth login
   ```
3. Na raiz do projeto, suba o app (a primeira vez cria o app no Fly com o
   nome do `fly.toml`; se `hiroshi-screen-share` já estiver em uso por outra
   conta, troque o campo `app` no `fly.toml` antes):
   ```bash
   fly deploy
   ```
4. Pronto — o Fly te dá uma URL tipo `https://hiroshi-screen-share.fly.dev`,
   já com HTTPS. É esse link que você manda pros seus amigos.

Pra rodar de novo depois de qualquer mudança no código, é só `fly deploy`
outra vez.

**Build incremental:** o `Dockerfile` usa cache mounts do BuildKit pro
registro do Cargo e pro `target/` de build. Só o primeiro deploy compila as
~250 dependências do zero (uns 5-7 min); deploys seguintes, quando só o
código do app muda, reaproveitam esse cache e ficam na casa de segundos —
testado localmente (rebuild após mudar uma linha em `src/` caiu de 7min pra
~18s). O cache persiste no builder do Fly entre execuções de `fly deploy`,
não só localmente.

## Deploy (geral)

Este projeto compila para um único binário Rust. Em produção:

- Sirva atrás de HTTPS (obrigatório para `getDisplayMedia` e WebSocket seguro
  fora de `localhost`) — por exemplo, um reverse proxy como Caddy com TLS
  automático, ou uma plataforma que já termina TLS (Fly.io, Render).
- Não é necessário banco de dados nem armazenamento persistente — todo o
  estado de salas vive em memória e é descartado quando o processo reinicia.
- Sem TURN configurado (só STUN público). Se algum amigo estiver numa rede
  muito restritiva (CGNAT, firewall corporativo) e não conseguir conectar,
  isso é uma limitação conhecida da v1 — um servidor TURN (`coturn`) resolveria,
  mas fica fora de escopo por agora.
- O `.cargo/config.toml` fixa `LEPTOS_OUTPUT_NAME=screen_share` — necessário
  para o `cargo-leptos` 0.3.7 e o `leptos` 0.8 concordarem no nome do arquivo
  `.wasm` gerado (sem isso, o navegador tenta buscar um arquivo que não existe
  e a página nunca hidrata). Mantenha esse arquivo se atualizar dependências.

**Não** rode o binário compilado diretamente (`./target/debug/screen_share`)
para testar localmente — nesse modo a página falha ao hidratar. Use sempre
`cargo leptos watch` (dev) ou `cargo leptos serve --release` (produção local).
