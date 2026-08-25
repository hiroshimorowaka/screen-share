# Excluir processos do áudio compartilhado — design

Data: 2026-08-22
Status: aprovado, aguardando plano de implementação

## Contexto

Terceiro e último dos três sub-projetos de áudio decompostos a partir do
pedido original (compartilhar tela + áudio do sistema, com exclusão de
processo, e controle de volume por espectador). Os outros dois já estão
prontos: controle de volume por espectador
(`docs/superpowers/specs/...` não existe como arquivo separado — foi
"design curto" aprovado em conversa) e compartilhar áudio do sistema
inteiro
(`docs/superpowers/specs/2026-08-22-desktop-system-audio-sharing-design.md`).

Este spec **substitui a colocação da UI** decidida no spec anterior: o
checkbox "Compartilhar áudio", que hoje vive na barra de controles da
sala (`src/ui/pages/room/mod.rs`), muda de lugar — vai para dentro da
janela do seletor de tela/janela do Electron
(`desktop/static/picker.html`). Também muda o mecanismo de captura: em
vez de capturar o monitor do sink padrão inteiro (tudo ou nada), passa
a existir um sink virtual próprio onde só os processos desejados são
ligados.

## Objetivo

- Ao compartilhar uma **janela/aplicativo específico** (aba
  "Aplicativos" do seletor) com "Compartilhar áudio" marcado: só o som
  daquele processo específico é compartilhado, automaticamente — sem
  precisar escolher nada a mais.
- Ao compartilhar a **tela inteira** (aba "Tela inteira") com
  "Compartilhar áudio" marcado: todo o áudio do sistema é
  compartilhado, exceto os processos que o usuário marcar para excluir
  numa lista que aparece ao lado do checkbox.
- Um processo excluído (ou, no caso de janela específica, qualquer
  processo que não seja o compartilhado) nunca aparece no áudio
  transmitido, mesmo que comece a tocar som depois que o
  compartilhamento já começou.

## Arquitetura

Investigação ao vivo (fora deste app, com dois tons de teste diferentes
simulando dois processos) confirmou o mecanismo central: criar um
**sink virtual próprio** ("Screen Share Mix", `media.class=Audio/Sink`
via `pw-loopback`, mesma técnica já usada no spec anterior mas
`Audio/Sink` em vez de `Audio/Source`) e ligar manualmente
(`pw-link`) só as portas de saída dos processos desejados nele — os
outros processos continuam tocando normalmente nas caixas de som do
usuário, só não entram no mix. Confirmado com análise de frequência
(Goertzel) que só o processo explicitamente ligado aparece na gravação
do sink virtual.

Para achar **qual processo é dono de uma janela** (caso "Aplicativos"):
o `id` que o `desktopCapturer` do Electron devolve para uma janela
(formato `window:<numero>:0`) já É o ID da janela X11 em decimal —
confirmado comparando com `_NET_CLIENT_LIST`. Rodando
`xprop -id <id> _NET_WM_PID` nesse ID (formato de saída:
`_NET_WM_PID(CARDINAL) = <pid>`) dá o PID dono, que casa direto com a
propriedade `application.process.id` que o PipeWire já expõe para cada
stream de áudio (confirmado inspecionando `pw-dump` com Spotify/Discord/
Brave rodando).

Para a **lista de exclusão** (caso "Tela inteira"): os processos de
áudio tocando no momento vêm de `pw-dump`, agrupados pela propriedade
`application.process.binary` (ex: `discord`, `spotify`) — não por
`application.name`, que para alguns apps (Discord, confirmado) mostra
algo não reconhecível tipo "WEBRTC VoiceEngine" em vez do nome do app.

Um processo novo que começa a tocar som **depois** que o
compartilhamento já está ativo também precisa ser pego (incluído se
não excluído; ignorado se for o caso de janela específica e não for o
processo certo) — um laço de verificação a cada ~1s (via `pw-dump`) no
processo principal do Electron cobre isso, ligando automaticamente
qualquer stream novo que passe no critério de inclusão da sessão atual.

## Componentes

- **`desktop/static/picker.html` / `picker.js`**: o checkbox
  "Compartilhar áudio" (que estava na sala) passa a viver aqui, perto
  das abas Aplicativos/Tela inteira, sempre visível independente da
  aba ativa. Quando marcado **e a aba ativa é "Tela inteira"**: busca
  (via IPC) a lista de processos tocando som agora e mostra como
  checklist (marcado = excluir). Quando marcado **e a aba é
  "Aplicativos"**: nenhuma lista aparece — a inclusão é automática,
  resolvida no processo principal a partir da janela escolhida.
  Escolher uma fonte (clicar num card) envia de volta pro processo
  principal um pacote único: fonte de vídeo + `shareAudio` + lista de
  binários excluídos (vazia fora do modo "Tela inteira").

- **`desktop/src/main.ts`**:
  - `showSourcePicker()` passa a resolver com
    `{ source, shareAudio, excludedBinaries }` em vez de só a fonte de
    vídeo.
  - Handler IPC novo para listar processos de áudio ativos
    (`list-audio-apps`), consultado pelo picker quando o checkbox é
    marcado na aba "Tela inteira".
  - `start-audio-loopback` deixa de não receber argumento: passa a
    receber ou `{ mode: "window", pid }` ou
    `{ mode: "screen", excludedBinaries }`. Cria o sink "Screen Share
    Mix" (se ainda não existir nesta sessão de compartilhamento), faz
    uma varredura inicial linkando os streams que já passam no
    critério, e liga um intervalo (~1s) que repete a varredura e liga
    automaticamente qualquer stream novo elegível.
  - `stop-audio-loopback` para o intervalo de verificação e mata o
    processo do sink virtual, sem argumento — chamado
    incondicionalmente sempre que uma sessão de compartilhamento
    termina (ver abaixo, isso simplifica o lado Rust: não precisa mais
    saber se havia áudio ativo, já que parar algo que não existe não
    faz nada).
  - A decisão de subir o sink de áudio acontece **antes** de responder
    ao `setDisplayMediaRequestHandler` — ou seja, no momento em que o
    usuário confirma a escolha no seletor, não depois.

- **`src/ui/client/webrtc.rs`**: `capture_display()` volta a não
  receber nenhum parâmetro (a decisão já foi tomada no seletor,
  inteiramente do lado Electron). Depois de pegar o vídeo, **sempre**
  tenta pegar a trilha de áudio do dispositivo "Screen Share Mix" — se
  o dispositivo não existir (usuário não marcou o checkbox no
  seletor), isso deixa de ser tratado como erro/fica silencioso: o
  resultado é só a stream de vídeo, igual ao comportamento de sempre.
  `start_desktop_audio_loopback`/`stop_desktop_audio_loopback` deixam
  de existir do lado Rust — quem inicia o áudio agora é inteiramente o
  processo principal do Electron, no momento da escolha do seletor.

- **`src/ui/pages/room/mod.rs` / `share.rs`**: o checkbox
  "Compartilhar áudio", o `RwSignal` `share_audio`, e o `RwSignal`
  `sharing_with_audio` são removidos — não existem mais no lado Rust.
  `stop_sharing` volta a ter a assinatura de antes do spec anterior,
  mas chama `stop_desktop_audio_loopback` incondicionalmente (função
  agora sem argumentos) sempre que para de compartilhar, seguro mesmo
  se não havia áudio ativo. `desktop_audio_supported()`/
  `is_desktop_app()` são removidos por não terem mais nenhum uso.

## Fluxo de dados

1. Usuário clica em compartilhar tela → `getDisplayMedia` → Electron
   abre o seletor.
2. Usuário escolhe a aba, opcionalmente marca "Compartilhar áudio" e
   (só na aba "Tela inteira") marca exclusões, clica numa fonte.
3. Processo principal recebe o pacote completo. Se `shareAudio`: monta
   o critério de inclusão (PID exato pra janela específica, ou
   "binário não está na lista de excluídos" pra tela inteira), cria o
   sink virtual, faz a varredura inicial e liga o intervalo de
   verificação — **tudo isso antes** de responder ao pedido de vídeo.
4. `callback({ video: fonte_escolhida })` resolve o `getDisplayMedia`
   do lado Rust.
5. Rust pega o vídeo, tenta pegar áudio do dispositivo "Screen Share
   Mix" (existe se `shareAudio` era `true`; não existe se era `false`
   — os dois casos são só "consegui" ou "não consegui", sem
   coordenação explícita adicional), junta numa `MediaStream` só
   (ou fica só com vídeo, se não tinha áudio), segue o fluxo normal de
   sinalização/WebRTC sem nenhuma mudança de protocolo — igual ao spec
   anterior.
6. Parar de compartilhar sempre chama `stop-audio-loopback`
   incondicionalmente — para o intervalo, mata o sink virtual, sem
   custo se nada estava rodando.

## Testes

Sem harness de automação de navegador para esta camada — validação
manual:

- Compartilhar uma janela específica (ex: Spotify) com áudio marcado:
  só o som do Spotify chega em quem assiste, mesmo com outros apps
  tocando som ao mesmo tempo — sem nenhuma lista de exclusão aparecer.
- Compartilhar a tela inteira com áudio marcado, excluindo um processo
  (ex: Discord): o som de todo o resto do sistema chega normalmente,
  o do processo excluído não chega, mesmo que ele comece a tocar som
  só depois que o compartilhamento já estava rolando.
- Compartilhar a tela inteira com áudio marcado, sem excluir nada:
  todo o áudio do sistema chega — comportamento equivalente ao spec
  anterior.
- Compartilhar sem marcar o checkbox (qualquer aba): comportamento
  idêntico a antes de qualquer um dos três sub-projetos de áudio — sem
  nenhuma trilha de áudio.
- Parar de compartilhar limpa o sink virtual (`wpctl status` não lista
  mais "Screen Share Mix" depois) em todos os casos: botão da UI,
  botão nativo do navegador, sair pela bandeja.

## Fora de escopo

- Trocar exclusões com a transmissão já em andamento (decisão tomada
  uma vez só, ao escolher a fonte no seletor).
- Ícones dos processos na lista de exclusão (só o nome do binário por
  enquanto).
- Windows.
