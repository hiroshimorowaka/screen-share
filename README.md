# Compartilhamento de tela P2P

Salas persistentes, com nome e protegidas por senha, pra compartilhar a tela
entre um grupo pequeno (até 10 pessoas), direto do navegador (Windows e
Linux) — sem instalar nada, sem contas, sem áudio/chat. Cada pessoa escolhe
um nick e uma cor ao entrar (vira um avatar redondo com a inicial do nick
sobre a cor escolhida). Qualquer pessoa na sala pode compartilhar a tela a
qualquer momento, mas compartilhar e assistir são coisas separadas, estilo
Discord: ligar o compartilhamento só acende um botão "Assistir
compartilhamento" no card de quem compartilhou — cada pessoa decide,
individualmente, quem quer assistir e quando parar, sem afetar quem mais
está assistindo a mesma pessoa. A sala não morre até o último membro sair.
O navegador lembra localmente (`localStorage`, nunca no servidor) das salas
que você criou ou entrou — código e nome, nunca a senha — e mostra isso como
"salas recentes" na home; abrir um link de sala já avisa na hora se ela não
existe mais, antes de pedir nick e senha. O vídeo trafega P2P via WebRTC; o
servidor só faz a sinalização inicial (autenticação da sala e troca de
offer/answer/ICE).

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

1. Abra `/`, crie uma sala com nick "Ana", uma cor à sua escolha, nome "Sala
   de teste" e senha "teste123" — confirme que navega direto para
   `/r/<código>` já autenticada (sem pedir nick/cor/senha de novo), mostra o
   nome da sala no cabeçalho e o card da Ana com um avatar redondo ("A" sobre
   a cor escolhida).
2. Numa aba separada (sem fechar a primeira — ver nota abaixo), abra o mesmo
   link — confirme que pede nick, cor e senha (não pede nome de sala, ela já
   existe). Digite a senha errada — "Senha incorreta.". Digite a certa com
   outro nick (ex. "Bia") e outra cor — confirme que entra e que os dois
   cards aparecem, cada um com borda e fundo na cor escolhida por aquela
   pessoa.
3. Na aba da Ana, clique "Compartilhar minha tela" e escolha uma janela —
   confirme que só a aba da Ana passa a mostrar o próprio preview (com um
   botão "Esconder preview" que some com ele sem parar a transmissão); na
   aba da Bia, o card da Ana continua mostrando o avatar, mas ganha um botão
   "Assistir compartilhamento" — o vídeo não aparece sozinho pra ninguém.
4. Na aba da Bia, clique "Assistir compartilhamento" no card da Ana —
   confirme que o vídeo da Ana aparece em poucos segundos, só nessa aba.
   Clique "Expandir" nesse card — confirme que ele ocupa a maior parte da
   grade e os outros cards encolhem numa tirinha ao redor; clique "Encolher"
   pra voltar ao normal.
5. Clique "Parar de assistir" na aba da Bia — confirme que o vídeo da Ana
   some (volta a mostrar o avatar dela) só na aba da Bia, e que o
   compartilhamento da Ana continua ativo (o botão de assistir continua
   disponível pra quem quiser).
6. Clique "Parar de compartilhar" na aba da Ana — confirme que o botão
   "Assistir compartilhamento" some do card da Ana na aba da Bia.
7. Clique "Sair da sala" na aba da Bia — confirme que ela volta pra `/` e o
   card dela some da aba da Ana; a aba da Ana continua na sala sozinha (a
   sala não morre com uma pessoa saindo, nem sendo quem a criou).
8. Feche a aba da Ana também — reabra `/r/<mesmo código>` numa aba nova e
   tente entrar — confirme que aparece "Sala não encontrada ou já foi
   encerrada." imediatamente, antes de qualquer formulário de nick/senha (a
   checagem acontece via `GET /api/rooms/:code`, antes do portão).
9. Abra um link com um código inexistente (ex. `/r/ZZZZZZZZ`) — confirme a
   mesma mensagem do passo 8.
10. Volte pra `/` — confirme que a sala do passo 1 não aparece mais em
    "Salas recentes" (foi removida por não existir mais). Crie uma sala
    nova e, sem sair dela, abra `/` numa aba adicional — confirme que a
    sala nova aparece em "Salas recentes" com o nome certo.

Nota: fechar a única aba de uma sala fecha sua conexão WebSocket, e o
servidor apaga a sala assim que ela fica sem membros — por isso o passo 2
pede pra manter a primeira aba aberta, e o passo 10 pede pra manter a sala
nova aberta numa aba enquanto confere a outra.

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
  estado de salas (incluindo o hash da senha de cada uma) vive em memória e
  é descartado quando o processo reinicia, junto com o resto do estado.
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
