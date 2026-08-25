# Casca desktop Tauri — design

Data: 2026-08-21
Status: aprovado, aguardando plano de implementação

## Contexto

Este é o primeiro de três sub-projetos que, juntos, viabilizam um app
desktop nativo capaz de compartilhar áudio do sistema excluindo processos
específicos (ex: não deixar o áudio do Discord vazar pra transmissão). Os
três sub-projetos, cada um com seu próprio spec:

1. **Casca desktop Tauri** (este documento) — janela nativa reaproveitando
   o front-end web existente, sem nenhuma mudança de captura de áudio.
2. Captura/exclusão de áudio por processo no Linux, via `pipewire-rs`.
3. Captura/exclusão de áudio por processo no Windows, via `wasapi`.

Os sub-projetos 2 e 3 dependem deste, mas este não depende deles — a
casca Tauri deve funcionar e ser útil (compartilhar tela sem áudio
filtrado) mesmo antes de qualquer trabalho de áudio nativo começar.

## Objetivo

Um app desktop nativo, para Linux, que abre o front-end web já existente
(Leptos, servido em produção pelo Fly.io em `screen-share-h0rb5w.fly.dev`)
dentro de uma janela nativa, com
um ícone de bandeja que permite manter o app rodando em segundo plano sem
uma janela ocupando espaço na tela — sem duplicar nenhum código de UI e
sem alterar o crate web existente.

## Arquitetura

Novo diretório `desktop/` na raiz do repositório, contendo um projeto
Tauri padrão com seu próprio `Cargo.toml` e `Cargo.lock`. Este projeto
**não** é membro de um Cargo workspace com o crate raiz — é um projeto
Cargo independente, coexistindo no mesmo repositório git. O crate raiz
existente (`screen-share`) permanece inteiramente intocado: mesmos
comandos (`cargo leptos watch`/`build`), mesmo `Dockerfile`, mesmo deploy
no Fly.io, mesmo acoplamento entre `cargo-leptos` e `.cargo/config.toml`
documentado no `CLAUDE.md`.

Motivo da escolha (crate irmão independente, em vez de workspace
unificado): o `desktop/` não precisa de nenhum código Rust do crate raiz
— ele só precisa saber a URL do site já hospedado. Evitar o workspace
evita mexer numa estrutura de build já delicada e documentada como frágil
(ver seção "Commands" do `CLAUDE.md`), sem nenhum ganho prático em troca.

## Componentes

- `desktop/src-tauri/` — projeto Tauri padrão: `Cargo.toml`,
  `tauri.conf.json`, `src/main.rs`, `icons/`.
- `main.rs` é responsável por três coisas:
  1. Criar a janela principal apontando para
     `https://screen-share-h0rb5w.fly.dev/` (URL de produção já
     hospedada — nenhum asset web é empacotado dentro do app).
  2. Registrar um ícone de bandeja do sistema com um menu de duas opções:
     "Abrir" e "Sair".
  3. Interceptar o evento de fechamento da janela
     (`WindowEvent::CloseRequested`): em vez de permitir o encerramento
     padrão do processo, chama `.hide()` na janela e cancela o evento.
- Clicar no ícone da bandeja, ou em "Abrir" no menu, chama `.show()` +
  `.set_focus()` na janela. "Sair" no menu encerra o processo de verdade
  (`app.exit(0)`).
- Ícone do app/bandeja: um ícone placeholder genérico. Substituir por um
  ícone definitivo é trabalho futuro, fora do escopo deste spec.

## Fluxo de dados

Nenhum fluxo novo é introduzido. A webview carrega exatamente a mesma
página que já existe hoje no navegador — o mesmo bundle WASM/JS/CSS
gerado pelo crate raiz, servido pelo mesmo backend Axum. Sinalização
WebSocket e conexões WebRTC continuam indo diretamente da webview para o
servidor de produção, sem nenhum componente novo no meio. Minimizar para
a bandeja apenas esconde a janela (`hide`); a webview permanece viva e
processando normalmente enquanto escondida, então uma tela sendo
compartilhada não é interrompida por esconder a janela.

## Plataformas / build

Apenas Linux (WebKitGTK) nesta entrega. Comandos `cargo tauri dev` e
`cargo tauri build`, executados de dentro de `desktop/src-tauri/`.
Windows (WebView2) fica para quando o sub-projeto de áudio WASAPI
começar — nesse momento também reavaliamos o comportamento de
`getDisplayMedia` nessa plataforma.

## Testes

Não há harness de automação de navegador neste repositório para esta
camada (consistente com a abordagem de testes já documentada no
`CLAUDE.md` para `client/` e páginas). Validação é manual:

- Abrir o app, entrar numa sala existente, compartilhar a tela de dentro
  da janela Tauri (WebKitGTK) e confirmar que outro membro, usando um
  navegador comum, consegue assistir normalmente.
- Fechar a janela pelo X: o app deve sumir da barra de tarefas mas
  continuar rodando na bandeja, e uma chamada em andamento não deve cair.
- Reabrir pela bandeja (clique no ícone ou "Abrir" no menu): a janela
  deve reaparecer com o estado da página preservado.
- "Sair" no menu da bandeja: o processo deve encerrar de verdade.

## Fora de escopo

- Captura ou exclusão de áudio por processo (specs separados, um para
  Linux/PipeWire e um para Windows/WASAPI).
- Build e teste no Windows.
- Instaladores, assinatura de código, ou qualquer mecanismo de
  auto-update.
- Ícone e identidade visual definitivos do app desktop.
