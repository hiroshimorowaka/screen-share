# Casca desktop Electron (substitui o Tauri) — design

Data: 2026-08-21
Status: aprovado, aguardando plano de implementação

## Contexto

A casca desktop original foi construída em Tauri (ver
`docs/superpowers/specs/2026-08-21-tauri-desktop-shell-design.md`).
Depois de destravar o pedido de permissão de captura de tela, ficou claro
que o compartilhamento de tela em si renderiza vídeo preto — investigado
e documentado em
`docs/superpowers/specs/2026-08-21-tauri-screen-share-black-video-investigation.md`.
A causa raiz: o WebKitGTK (motor usado pelo Tauri no Linux) não
implementa corretamente o fallback de memória compartilhada quando a
negociação de formato DMA-BUF do PipeWire falha — uma lacuna conhecida e
sem previsão de correção (o próprio Tauri lista suporte a
Chromium/CEF no Linux como item futuro sem ETA). Chromium e Firefox
implementam esse fallback corretamente, e é por isso que o
compartilhamento sempre funcionou normalmente pelo navegador comum.

Este spec substitui a casca Tauri por uma em **Electron**, que empacota
o Chromium de verdade em vez de depender do WebView do sistema —
eliminando a causa raiz na fonte, ao custo de um app mais pesado e da
casca deixar de ser Rust.

## Objetivo

Um app desktop nativo para Linux que abre o front-end web já existente
dentro de uma janela Electron, com ícone de bandeja (mesmo comportamento
de antes: fechar esconde, "Sair" encerra de verdade), e — novidade em
relação à casca Tauri — compartilhamento de tela funcionando de fato,
incluindo a escolha de qual tela/janela compartilhar.

## Arquitetura

`desktop/src-tauri/` é removido por completo (o histórico de commits do
Tauri permanece no git para quem quiser consultar). `desktop/` passa a
ser um projeto Electron + TypeScript, gerenciado com `pnpm`, com sua
própria `package.json`/`pnpm-lock.yaml` — mesma filosofia de antes: não
é workspace com o crate Rust da raiz, que continua inteiramente
intocado. A janela principal aponta para
`https://screen-share-h0rb5w.fly.dev/` como URL externa; nenhum asset
web é empacotado dentro do app.

## Componentes

- `desktop/src/main.ts` — processo principal do Electron. Responsável
  por: criar a janela principal; registrar o ícone de bandeja com menu
  "Abrir"/"Sair"; interceptar o evento de fechamento da janela (esconder
  em vez de encerrar); registrar
  `session.defaultSession.setDisplayMediaRequestHandler`.
- `desktop/src/picker.html` + `desktop/src/picker.ts` — janela de
  seleção de fonte de captura, própria do app. O Electron, diferente de
  um navegador comum, não mostra sozinho um seletor nativo de
  tela/janela — isso é responsabilidade do app embutir. Mostra uma grade
  simples com miniatura + nome de cada tela/janela disponível (via
  `desktopCapturer.getSources`); clicar numa fecha o seletor e libera o
  compartilhamento daquela fonte; fechar o seletor sem escolher cancela
  o pedido corretamente (sem travar a Promise do `getDisplayMedia()` do
  lado da página).
- `desktop/src/preload.ts` — ponte de IPC (`contextBridge`) entre
  `main.ts` e `picker.ts`, para o seletor pedir a lista de fontes e
  devolver a escolha do usuário.
- Ícone do app/bandeja: reaproveita o mesmo placeholder gerado para a
  casca Tauri (quadrado azul sólido) — ícone definitivo continua fora de
  escopo.

## Fluxo de dados

Igual à casca Tauri para tudo que não é compartilhamento de tela: a
janela carrega a mesma página de produção, sinalização WebSocket e
WebRTC seguem indo direto para o servidor, sem componente novo no meio.
Minimizar para a bandeja apenas esconde a janela — a página continua
viva e processando.

O que é novo é o caminho do `getDisplayMedia()`: a página pede captura →
o handler no processo principal recebe o pedido (`videoRequested`,
`audioRequested`) → abre a janela do seletor com a lista de fontes → o
usuário escolhe uma → o handler responde ao pedido original com a fonte
escolhida. Como o app não pede áudio no `getDisplayMedia()` hoje
(confirma o próprio `CLAUDE.md`: "não há áudio ainda"), o handler não
precisa lidar com áudio — só repassa `audio` se algum dia
`audioRequested` vier `true`.

## Plataformas / build

Apenas Linux nesta entrega, mesmo escopo de antes. `pnpm install` e
`pnpm start` (ou equivalente) para rodar em desenvolvimento — sem
empacotamento/instalador nesta entrega.

## Testes

Sem harness de automação de navegador para esta camada, mesma abordagem
de sempre — validação manual:

- Abrir o app, entrar numa sala, e confirmar que a UI carrega igual ao
  navegador.
- Clicar em compartilhar tela: deve abrir a janela de seleção própria do
  app, com miniaturas de tela(s) e janela(s) disponíveis.
- Escolher uma fonte: o compartilhamento deve funcionar de verdade desta
  vez — vídeo visível tanto no preview de quem compartilha quanto para
  quem está assistindo por outro navegador (isso é o que a casca Tauri
  não conseguia fazer).
- Fechar a janela do seletor sem escolher nada: o pedido de
  compartilhamento deve ser cancelado de forma limpa, sem travar a
  página.
- Fechar (X) a janela principal: some da barra de tarefas, processo
  continua rodando, uma chamada em andamento não cai.
- Reabrir pela bandeja: janela volta com o estado preservado.
- "Sair" pela bandeja: processo encerra de verdade.

## Fora de escopo

- Áudio (do sistema ou por processo) — specs futuros e separados, como
  já estava definido antes desta mudança de framework.
- Build e teste no Windows.
- Instaladores, assinatura de código, auto-update (`electron-builder` ou
  equivalente fica para uma entrega futura).
- Ícone e identidade visual definitivos do app desktop.
