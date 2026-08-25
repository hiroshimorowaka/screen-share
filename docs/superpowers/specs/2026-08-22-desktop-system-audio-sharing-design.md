# Compartilhar áudio do sistema pelo app desktop (sem exclusão) — design

Data: 2026-08-22
Status: aprovado, aguardando plano de implementação

## Contexto

Segundo de três sub-projetos de áudio decompostos a partir de um pedido
maior (compartilhar tela + áudio do sistema, com a possibilidade futura
de excluir um processo específico do áudio compartilhado, e controle de
volume por espectador). O primeiro sub-projeto — controle de volume por
espectador — já está implementado (`src/ui/pages/room/member_card.rs`,
`media_controls.rs`). Este cobre "compartilhar o áudio do sistema
inteiro junto com a tela, só pelo app desktop, de forma opcional". A
exclusão de um processo específico fica para um terceiro sub-projeto,
construído em cima da mesma base deste.

Investigação prévia (ver histórico de commits do sub-projeto do app
desktop) já descartou a API oficial do Electron para isso
(`setDisplayMediaRequestHandler` com `audio: 'loopback'`) por ter bugs
documentados e específicos do Linux. Em vez disso, uma investigação ao
vivo nesta máquina confirmou que o próprio PipeWire já resolve o
problema com uma ferramenta de linha de comando padrão do sistema
(`pw-loopback`), sem precisar de nenhum crate Rust novo para esta
etapa.

## Objetivo

Um usuário do app desktop, ao compartilhar sua tela, pode marcar um
checkbox opcional "Compartilhar áudio" que faz o áudio de todo o
sistema (tudo que estiver tocando no computador) ir junto com o vídeo
para quem estiver assistindo — seja essa pessoa também no app desktop
ou num navegador comum. O site em si, fora do app desktop, nunca
mostra essa opção nem consegue compartilhar áudio.

## Arquitetura

Nenhum crate Rust novo entra nesta etapa. O processo principal do
Electron (`desktop/src/main.ts`) sobe e derruba um subprocesso
`pw-loopback` — já instalado como parte do PipeWire neste sistema — que
cria um dispositivo de áudio virtual (`Audio/Source` no PipeWire,
aparece como um "microfone" comum para qualquer aplicação) espelhando
o monitor do sink de áudio padrão do sistema. A primeira versão testada
deste comando colocava `media.class=Audio/Source` do lado errado (a
captura, não o playback) — isso cria um dispositivo com nome certo e
selecionável, mas que carrega só silêncio; confirmado gravando ele
direto com `pw-record`, sem passar pelo app, enquanto um som tocava. A
versão corrigida, testada ao vivo com um tom de teste + gravação
isolada até confirmar sinal real chegando:

```
pw-loopback -C @DEFAULT_SINK@ \
  --capture-props="stream.capture.sink=true node.passive=true" \
  --playback-props="media.class=Audio/Source node.name=screen_share_audio node.description='Screen Share Audio'"
```

O site (`src/ui/`) continua sendo exatamente o mesmo código Rust/Leptos
em qualquer contexto — a única diferença de comportamento é detectada
em runtime pela presença (ou não) de uma ponte JS exposta pelo
processo principal do Electron, nunca por sniffing de user-agent.

## Componentes

- **`desktop/src/main.ts`**: dois novos handlers IPC —
  `start-audio-loopback` (spawna o `pw-loopback` acima via
  `child_process.spawn`, resolve a Promise quando o dispositivo já
  existe — ver "Sincronização" abaixo) e `stop-audio-loopback` (mata o
  processo do loopback, se houver um rodando). O processo filho fica
  guardado numa variável de módulo, no mesmo padrão que `mainWindow`/
  `tray` já usam. `app.on('before-quit', ...)` (já existe, adiciona
  mais uma linha) garante que o loopback morre junto se o app inteiro
  for fechado com um compartilhamento de áudio ativo.
- **`desktop/src/preload.ts`**: a janela **principal** passa a ter um
  preload próprio (hoje só a janela do seletor de tela tem um). Expõe
  `window.desktopAudio = { start(): Promise<void>, stop(): Promise<void> }`
  via `contextBridge`, encaminhando para os dois handlers IPC acima. A
  simples presença de `window.desktopAudio` no `window` do navegador é
  como o código Rust do site sabe "estou rodando dentro do app
  desktop" — sem essa chave, o checkbox de áudio nunca aparece e
  `share_audio` nunca é `true`.
- **`src/ui/client/webrtc.rs`**: `capture_display()` ganha um novo
  parâmetro `share_audio: bool`. Quando `false` (comportamento atual,
  inalterado): só pede vídeo, como hoje. Quando `true`: primeiro chama
  `window.desktopAudio.start()` (via `web_sys`/`js_sys`, checando que a
  chave existe antes) e espera a Promise resolver; em seguida pega o
  vídeo (`getDisplayMedia`, como já faz) e o áudio
  (`getUserMedia({audio: {deviceId: exact}})`, com o `deviceId` do
  dispositivo cujo `label` é `"Screen Share Audio"` — encontrado
  enumerando dispositivos via `MediaDevices::enumerate_devices()`);
  depois junta as duas trilhas numa única `MediaStream` nova
  (`MediaStream::new_with_tracks` ou equivalente) antes de devolver.
- **`src/ui/pages/room/share.rs`**: `share_toggle_handler` passa a
  receber também um `share_audio: ReadSignal<bool>` e repassa esse
  valor para `capture_display`. `stop_sharing` passa a chamar
  `window.desktopAudio.stop()` (se existir e se essa sessão de
  compartilhamento tinha áudio ativo) — cobre parar manualmente e o
  botão nativo "Stop sharing" do navegador, já que os dois já passam
  por essa mesma função hoje.
- **`src/ui/pages/room/mod.rs`**: novo `RwSignal<bool>` `share_audio`,
  e um checkbox "Compartilhar áudio" na barra de controles, ao lado do
  botão de compartilhar tela. `class:hidden` quando `window.desktopAudio`
  não existe. Desmarcado por padrão. Uma nova função auxiliar
  `is_desktop_app() -> bool` (par hydrate/não-hydrate, mesmo padrão de
  `share_supported()` em `share.rs`) checa a presença de
  `window.desktopAudio` via `js_sys::Reflect::has`.

## Fluxo de dados

1. Usuário marca "Compartilhar áudio" e clica em "Compartilhar tela".
2. `capture_display(true)` chama `window.desktopAudio.start()`.
3. O preload manda `start-audio-loopback` pro processo principal, que
   spawna o `pw-loopback` e só resolve a Promise quando o dispositivo
   já está de fato criado (ver "Sincronização").
4. Com o dispositivo já existindo, o código Rust pega o vídeo via
   `getDisplayMedia` (como hoje) e o áudio via `getUserMedia` mirando
   esse dispositivo virtual pelo nome, e junta as duas trilhas numa
   `MediaStream` só.
5. Essa `MediaStream` combinada segue o caminho que já existe hoje —
   vira a stream local, é anexada a cada `RTCPeerConnection` que se
   conecta depois. Como as duas trilhas já estão na stream *antes* da
   primeira oferta WebRTC ser criada, a oferta já inclui áudio e vídeo
   desde o início — **nenhuma mudança de protocolo de sinalização é
   necessária**, nem para quem já estava assistindo antes vs depois.
6. Quem assiste — no app desktop ou num navegador comum, sem
   diferença de código nenhuma dos dois lados — recebe as duas
   trilhas normalmente pelo `<video>` que já existe. O controle de
   volume por espectador (sub-projeto 1) já funciona em cima disso sem
   nenhuma alteração, porque ele opera sobre qualquer trilha de áudio
   que aparecer no elemento, não sabe nem precisa saber como ela
   chegou lá.
7. Parar de compartilhar (manual ou via botão nativo do navegador)
   chama `window.desktopAudio.stop()` quando havia áudio ativo,
   encerrando o `pw-loopback` e removendo o dispositivo virtual.

## Sincronização entre o Electron e o subprocesso

`child_process.spawn` retorna antes do `pw-loopback` terminar de criar
o nó no grafo do PipeWire — chamar `getUserMedia` cedo demais faria o
`enumerateDevices()` não encontrar o dispositivo ainda. O handler
`start-audio-loopback` só resolve a Promise depois de confirmar que o
dispositivo existe de verdade, consultando `pw-dump` filtrando pelo
`node.name` (`screen_share_audio`) em um loop de verificação a cada
100ms, com timeout total de 3 segundos (rejeita a Promise se estourar)
— não um `setTimeout` fixo arbitrário, que seria frágil.

## Testes

Sem harness de automação de navegador para esta camada, mesmo padrão
do resto do projeto — validação manual:

- No navegador comum (fora do app desktop): o checkbox "Compartilhar
  áudio" nunca aparece.
- No app desktop, sem marcar o checkbox: compartilhar continua
  exatamente como hoje, sem nenhuma trilha de áudio — comportamento
  inalterado.
- No app desktop, marcando o checkbox: compartilhar tela faz o áudio
  do sistema (ex: tocar uma música) chegar em quem está assistindo,
  tanto no app desktop quanto num navegador comum assistindo a mesma
  sala.
- Parar de compartilhar remove o dispositivo virtual —
  `wpctl status` não deve mais listar "Screen Share Audio" depois de
  parar.
- Fechar o app inteiro pela bandeja ("Sair") enquanto um
  compartilhamento com áudio está ativo também limpa o dispositivo
  virtual (não deixa `pw-loopback` órfão rodando).
- O botão nativo "Stop sharing" do navegador (ou do próprio Chromium
  do Electron), usado em vez do botão da interface, também limpa o
  dispositivo — já que passa pela mesma `stop_sharing`.

## Fora de escopo

- Excluir um processo específico do áudio compartilhado (terceiro
  sub-projeto, construído em cima desta mesma base de
  loopback/dispositivo virtual).
- Windows (esta entrega é Linux/PipeWire apenas, mesmo escopo de
  plataforma que o resto do app desktop até aqui).
- Persistir a escolha do checkbox entre sessões — sempre desmarcado ao
  entrar numa sala nova, mesmo padrão do controle de volume.
- Qualquer UI de "escolher qual áudio" (ex: só um app específico) —
  isso é exatamente o que o terceiro sub-projeto resolve.
