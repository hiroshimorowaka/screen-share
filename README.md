# Compartilhamento de tela P2P

Salas persistentes e protegidas por senha para compartilhar tela com um
grupo pequeno (até 10 pessoas), direto do navegador (Windows e Linux) — sem
instalar nada, sem contas. Qualquer pessoa na sala pode compartilhar a
própria tela a qualquer momento; assistir é uma escolha individual de cada
um, sem afetar quem mais está assistindo. O vídeo trafega direto entre os
navegadores via WebRTC — o servidor só cuida da sinalização.

Pelo navegador, o compartilhamento é só de vídeo. Quem instalar o
[app desktop](#app-desktop) (Linux ou Windows) pode compartilhar o áudio do
sistema junto com a tela — de um app específico automaticamente, ou da
tela inteira excluindo os apps que quiser — e também compartilhar direto
pela bandeja do sistema, sem precisar abrir a janela do app.

## Rodando localmente

Pré-requisitos:

- Rust + `rustup target add wasm32-unknown-unknown`
- `cargo install cargo-leptos`

```bash
cargo leptos watch
```

Abra `http://127.0.0.1:3000/`.

## App desktop

`desktop/` é um wrapper Electron do mesmo site. Além de dar acesso ao
áudio do sistema (impossível só pelo navegador), ele abre escondido na
bandeja assim que inicia — clique com o botão direito no ícone pra "Abrir"
a janela normal, ou pra "Compartilhar tela": isso cria uma sala com nome
aleatório, entra nela com o nick salvo (ou um aleatório, se você nunca
definiu um), abre o seletor de tela, e assim que você escolher o que
compartilhar, copia o link da sala pra sua área de transferência — tudo
sem a janela do app aparecer em nenhum momento. Cancelar o seletor de tela
nesse fluxo sai da sala automaticamente, em vez de deixar ela pendurada
sem ninguém olhando.

Roda por cima da mesma sala/protocolo do site, sem servidor próprio.
Disponível pra **Linux (X11)** e **Windows**.

### Instaladores prontos

Toda mudança em `desktop/` publicada em `main` gera instaladores novos
automaticamente (veja [CI/CD](#cicd) abaixo). Baixe a versão mais recente
na [release `desktop-latest`](https://github.com/hiroshimorowaka/screen-share/releases/tag/desktop-latest):
`.AppImage`/`.deb` pra Linux, instalador ou versão portátil (`.exe`) pra
Windows.

### Rodando a partir do código

Pré-requisitos (além dos da seção anterior):

- Node.js + [`pnpm`](https://pnpm.io)
- **Linux**: PipeWire com `pw-loopback`, `pw-link` e `pw-dump` no `PATH`
  (padrão em distros atuais) e `xprop` (pacote `x11-utils`/`xorg-xprop`,
  conforme a distro) — usados pro compartilhamento de áudio por app/tela.
- **Windows**: nada além de Node/pnpm pra rodar o app. Pra recompilar o
  addon nativo de áudio (WASAPI, em `desktop/native/windows-audio/`)
  depois de mexer nele, precisa de Rust com o alvo
  `x86_64-pc-windows-msvc` e das Visual Studio Build Tools — veja
  `npm install && npm run build` dentro daquela pasta.

Rodando:

```bash
cd desktop
pnpm install
pnpm start
```

Por padrão o app aponta para o site publicado
(`https://screen-share-h0rb5w.fly.dev/`) — para testar contra um
`cargo leptos watch` local, troque `PROD_URL` em
`desktop/src/main-window.ts` temporariamente para
`http://127.0.0.1:3000/`.

No picker de compartilhamento, marque "Compartilhar áudio": escolhendo um
app específico ("Aplicativos"), só o áudio dele vai junto, automaticamente;
escolhendo "Tela inteira", vai o áudio do sistema todo, exceto os processos
marcados no dropdown de exclusão (no Windows, a exclusão funciona do mesmo
jeito, só que via WASAPI em vez de PipeWire). Um app que começa a tocar som
só depois de já estar compartilhando ainda é pego (checagem contínua, não
só no início).

Pra gerar os instaladores localmente em vez de baixar os da CI:
`pnpm run dist:linux` ou `pnpm run dist:win` (dentro de `desktop/`) — saem
em `desktop/release/`.

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
   encerrada." imediatamente, antes de qualquer formulário de nick/senha.
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

O projeto já vem pronto pra isso: `Dockerfile` e `fly.toml` configurados
(porta 8080, região `gru` — São Paulo).

1. Instale o `flyctl` (se ainda não tiver):
   ```bash
   curl -L https://fly.io/install.sh | sh
   ```
2. Faça login (abre o navegador):
   ```bash
   fly auth login
   ```
3. Na raiz do projeto, suba o app (a primeira vez cria o app no Fly com o
   nome do `fly.toml`; se `screen-share-h0rb5w` já estiver em uso por outra
   conta, troque o campo `app` no `fly.toml` antes):
   ```bash
   fly deploy
   ```
4. Pronto — o Fly te dá uma URL tipo `https://screen-share-h0rb5w.fly.dev`,
   já com HTTPS. É esse link que você manda pros seus amigos.

Pra rodar de novo depois de qualquer mudança no código, é só `fly deploy`
outra vez. Deploys depois do primeiro são bem mais rápidos, graças ao cache
de build do Fly.

## CI/CD

Todo push (ou merge de PR) na `main` roda uma pipeline no GitHub Actions
(`.github/workflows/ci-cd.yml`) que só mexe no que realmente mudou:

- Mudou algo em `src/`, `Cargo.toml`/`Cargo.lock`, `Dockerfile`,
  `fly.toml`, `.cargo/`, `public/` ou `tests/`? Roda `clippy` + os testes
  nos dois alvos (`ssr` e `hydrate`) e faz `fly deploy` de verdade.
- Mudou algo em `desktop/`? Builda os instaladores de Linux e Windows em
  paralelo e publica tudo na release rolante `desktop-latest` (ela é
  atualizada a cada push, não acumula uma release por commit).

Um PR que só mexe num lado nunca aciona o outro. Precisa do secret
`FLY_API_TOKEN` configurado no repositório pra o deploy funcionar:

```bash
fly tokens create deploy -x 999999h | gh secret set FLY_API_TOKEN
```

Dá pra rodar a pipeline inteira manualmente (os dois lados, ignorando o
que mudou de fato) pela aba Actions do GitHub ou com
`gh workflow run "CI/CD"` — útil pra redeployar sem precisar de um commit
qualquer só pra isso.

## Deploy (geral)

Este projeto compila para um único binário Rust. Em produção:

- Sirva atrás de HTTPS (obrigatório para `getDisplayMedia` e WebSocket seguro
  fora de `localhost`).
- Não precisa de banco de dados — o estado das salas vive em memória e é
  descartado quando o processo reinicia.
- Sem TURN configurado (só STUN público). Redes muito restritivas (CGNAT,
  firewall corporativo) podem não conseguir conectar.

**Não** rode o binário compilado diretamente (`./target/debug/screen_share`)
para testar localmente — nesse modo a página falha ao hidratar. Use sempre
`cargo leptos watch` (dev) ou `cargo leptos serve --release` (produção local).
