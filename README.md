# Compartilhamento de tela P2P

[![CI/CD](https://github.com/hiroshimorowaka/screen-share/actions/workflows/ci-cd.yml/badge.svg)](https://github.com/hiroshimorowaka/screen-share/actions/workflows/ci-cd.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Leptos](https://img.shields.io/badge/built%20with-Leptos-orange.svg)](https://leptos.dev)

Salas persistentes e protegidas por senha para compartilhar tela com um
grupo pequeno (até 10 pessoas), direto do navegador (Windows e Linux) — sem
instalar nada, sem contas. Qualquer pessoa na sala pode compartilhar a
própria tela a qualquer momento; assistir é uma escolha individual de cada
um, sem afetar quem mais está assistindo. O vídeo trafega direto entre os
navegadores via WebRTC; o servidor só cuida da sinalização.

Pelo navegador o compartilhamento já pode levar áudio, dependendo do que
você escolhe compartilhar (guia, janela ou tela inteira). Quem instalar o
[app desktop](#app-desktop) (Linux ou Windows) leva o áudio do sistema de
forma mais completa — de um app específico automaticamente, ou da tela
inteira excluindo os apps que quiser — e pode compartilhar direto pela
bandeja do sistema, sem abrir a janela do app.

Compartilhar exige um navegador de desktop (o `getDisplayMedia` não existe
em navegador de celular). No celular dá pra entrar na sala e assistir
normalmente — a interface se adapta ao toque: foca uma transmissão por vez,
toque no vídeo mostra/esconde os controles, e o resto vira folha inferior.

## Funcionalidades

- Salas persistentes, com código curto e nome, públicas ou protegidas por
  senha.
- Sem host: qualquer membro compartilha ou para de compartilhar a própria
  tela quando quiser.
- Assistir é escolha de cada espectador, sem afetar quem mais está
  assistindo.
- Vídeo peer-to-peer via WebRTC — o servidor só faz a sinalização.
- Áudio pelo navegador conforme o que for compartilhado; áudio completo do
  sistema pelo app desktop, com inclusão/exclusão por app.
- Cada navegador lembra as salas que já criou ou entrou.
- Relé TURN próprio opcional, pra redes mais restritivas.
- No celular dá pra entrar e assistir (compartilhar exige desktop).

## Rodando localmente

Pré-requisitos:

- Rust + `rustup target add wasm32-unknown-unknown`
- `cargo install cargo-leptos`

```bash
cargo leptos watch
```

Abra `http://127.0.0.1:3000/`.

## Estrutura do projeto

Rust + Leptos como único framework, compilado duas vezes: renderizado no
servidor (SSR) e hidratado no navegador (WASM). O código é dividido por
responsabilidade:

```
apps/web          UI (componentes Leptos, roda em ssr e hydrate)
apps/server       host Axum/Tokio — HTTP, WebSocket, middlewares
crates/domain     lógica pura sem I/O (SDP, backoff)
crates/protocol   tipos do protocolo de sinalização
crates/signaling  relé de sinalização — registro de salas, roteamento
desktop/          wrapper Electron — áudio de sistema, bandeja
```

## App desktop

`desktop/` é um wrapper Electron do mesmo site. Ele existe por causa do
áudio do sistema, que o navegador não entrega. Ao iniciar, abre escondido
na bandeja — botão direito no ícone pra "Abrir" a janela normal, ou pra
"Compartilhar tela": isso cria uma sala com nome aleatório, entra nela com
o nick salvo (ou um aleatório, se você nunca definiu um), abre o seletor de
tela e, assim que você escolhe o que compartilhar, copia o link da sala pra
área de transferência — sem a janela do app aparecer. Cancelar o seletor
nesse fluxo sai da sala, em vez de deixá-la pendurada sem ninguém olhando.

Roda por cima da mesma sala e do mesmo protocolo do site, sem servidor
próprio. Disponível pra **Linux (X11)** e **Windows**.

### Instaladores prontos

Toda mudança em `desktop/` publicada em `main` gera instaladores novos
automaticamente (veja [CI/CD](#cicd) abaixo) e os publica na aba
**Releases** deste repositório, numa tag `desktop-v<X.Y.Z>`:
`.AppImage`/`.deb` pra Linux, instalador ou portátil (`.exe`) pra Windows.
Cada PR que mexe em `desktop/` precisa subir a versão em
`desktop/package.json` e descrever a mudança em `desktop/CHANGELOG.md` (em
português, sem tecnicidade) — esse texto vira as notas da release.

### Rodando a partir do código

Pré-requisitos (além dos da seção anterior):

- Node.js + [`pnpm`](https://pnpm.io)
- **Linux**: PipeWire com `pw-loopback`, `pw-link` e `pw-dump` no `PATH`
  (padrão em distros atuais) e `xprop` (pacote `x11-utils`/`xorg-xprop`,
  conforme a distro) — usados pro compartilhamento de áudio por app/tela.
- **Windows**: nada além de Node/pnpm pra rodar. Pra recompilar o addon
  nativo de áudio (WASAPI, em `desktop/native/windows-audio/`) depois de
  mexer nele, precisa de Rust com o alvo `x86_64-pc-windows-msvc` e das
  Visual Studio Build Tools — veja `npm install && npm run build` dentro
  daquela pasta.

Rodando:

```bash
cd desktop
pnpm install
pnpm start
```

Por padrão o app aponta pra URL de produção (`PROD_URL` em
`desktop/src/main/app-url.ts`). Pra testar contra um `cargo leptos watch`
local, rode com `SCREEN_SHARE_URL=http://127.0.0.1:3000/ pnpm start`.

No picker de compartilhamento, marque "Compartilhar áudio": escolhendo um
app específico ("Aplicativos"), só o áudio dele vai junto, automaticamente;
escolhendo "Tela inteira", vai o áudio do sistema todo, exceto os processos
marcados no dropdown de exclusão. Um app que começa a tocar som depois de o
compartilhamento já ter começado ainda é pego — a checagem é contínua.

Pra gerar os instaladores localmente: `pnpm run dist:linux` ou
`pnpm run dist:win` (dentro de `desktop/`), saída em `desktop/release/`.

## Testes automatizados

```bash
scripts/test-all.sh --no-mutants   # fmt, clippy, build, Rust, WASM, Playwright
cargo test --workspace --features ssr   # só a suíte Rust nativa
```

A suíte Rust cobre a lógica de sinalização: protocolo, registro de salas,
endpoint WebSocket. A captura de tela e o handshake WebRTC só existem
dentro de um navegador de verdade e são cobertos pelos testes Playwright em
`apps/web/end2end/` (dois membros numa sala, compartilhar, assistir, mídia
WebRTC real trafegando; um projeto `mobile-web` à parte cobre a UI de toque
num viewport de celular). O que ainda não é automatizado: o controle de
"parar de compartilhar" do próprio navegador, captura de janela/tela real,
áudio real e adaptação de bitrate.

## CI/CD

Todo push (ou merge de PR) na `main` roda uma pipeline que só mexe no que
realmente mudou: mudanças no servidor rodam `clippy` + os testes nos dois
alvos (`ssr` e `hydrate`) e fazem o deploy; mudanças no app desktop geram
instaladores novos pra Linux e Windows e os publicam na aba **Releases**.
Um PR que só mexe num lado nunca aciona o outro.

## Deploy (geral)

Este projeto compila para um único binário Rust. Em produção:

- Sirva atrás de HTTPS (obrigatório para `getDisplayMedia` e WebSocket
  seguro fora de `localhost`).
- Não precisa de banco de dados — o estado das salas vive em memória e é
  descartado quando o processo reinicia.
- Funciona só com STUN público, mas também dá pra configurar um TURN
  próprio pra redes mais restritivas (CGNAT, firewall corporativo).

**Não** rode o binário compilado diretamente
(`./target/debug/screen-share-server`) para testar localmente — nesse modo
a página falha ao hidratar. Use sempre `cargo leptos watch` (dev) ou
`cargo leptos serve --release` (produção local).

## Licença

[MIT](LICENSE).
