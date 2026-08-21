# Investigação: compartilhamento de tela renderiza preto na casca Tauri

Data: 2026-08-21
Status: bloqueado por bug de sistema fora do código deste repositório — pausado

## Contexto

Depois de destravar o pedido de permissão de captura de tela dentro da
janela Tauri (ver commit `feat(desktop): allow media permission requests
for getDisplayMedia` em `desktop/src-tauri/src/main.rs`), o seletor de
tela/janela do sistema passou a aparecer normalmente e o
`getDisplayMedia()` resolve — mas o vídeo renderiza como um retângulo
preto sólido, tanto no preview local de quem compartilha quanto na tela
de quem está assistindo. Isso acontece consistentemente, testado à mão
pelo usuário na máquina de desenvolvimento.

## Causa raiz confirmada

Instrumentando o app com `GST_DEBUG` (variáveis de ambiente setadas
temporariamente em `main()`, removidas depois de diagnosticar), o log
mostrou a negociação de formato falhando na própria captura, antes de
qualquer frame chegar a ser renderizado:

```
FIXME    webkitvideocapturer  GStreamerVideoCapturer.cpp:320:reconfigure:
         Caps re-negotiation disabled on display capture source
DEBUG    pipewiresrc  caps of peer: video/x-raw(memory:DMABuf),
         format=(string)DMA_DRM, drm-format=(string)AR24
DEBUG    pipewiresrc  have common caps: video/x-raw(memory:DMABuf), ...
DEBUG    pipewiresrc  clear format
DEBUG    pipewiresrc  clear format
WARN     pipewiresrc  error: stream error: no more input formats
WARN     basesrc      error: streaming stopped, reason not-negotiated (-4)
```

Em português: o PipeWire (via mutter/GNOME, único backend deste sistema
que implementa a interface `ScreenCast` do xdg-desktop-portal) oferece os
frames da tela em formato DMA-BUF/DMA_DRM (buffer de GPU, zero-cópia).
GStreamer aceita esse formato na negociação inicial ("have common caps"),
mas em seguida o formato é limpo/rejeitado ("clear format" x2) e a
conexão do stream falha com "no more input formats" — sem nenhum formato
alternativo pra cair. O próprio código-fonte do WebKitGTK documenta essa
limitação: para fontes de captura de tela (diferente de webcam), a
renegociação de caps é deliberadamente desabilitada
(`GStreamerVideoCapturer.cpp:320`), então quando a primeira tentativa
falha, não há segunda tentativa — a captura morre e o `<video>` element
nunca recebe frame nenhum, daí o preto.

Isso é consistente com uma falha de negociação de *modifiers* DMA-BUF
entre o exportador (mutter, screencast do GNOME) e o importador
(GStreamer/Mesa) — uma classe de bug conhecida no ecossistema
PipeWire+GStreamer+WebKitGTK, mas não algo que o código deste app
controla.

## O que foi tentado e descartado

Todos os itens abaixo foram testados e revertidos por não resolverem o
problema (o commit `chore(desktop): revert unproven black-video
workarounds` remove os dois primeiros do código):

1. **`WEBKIT_DISABLE_DMABUF_RENDERER=1`** — workaround oficial do Tauri
   para telas em branco por falha do renderizador DMA-BUF (ver
   `tauri-apps/tauri#9394`). Não resolveu, e piorou a mensagem de erro ao
   testar sem as outras mudanças (mas o mesmo erro "not-negotiated"
   também acontece sem essa flag — não é ela a causa).
2. **`WEBKIT_DISABLE_COMPOSITING_MODE=1`** — desabilita composição
   acelerada por GPU no WebKitGTK; várias fontes citam como necessário
   para WebRTC funcionar no Linux. Não teve efeito aqui porque o
   problema é anterior à composição — a captura nunca produz frame.
3. **Versão mais nova do `gstreamer1.0-pipewire`** — já está na mais
   recente disponível via apt no Ubuntu 24.04 (`1.0.5-1ubuntu3.3`).
4. **Backend alternativo do portal de captura de tela** — só existe um
   backend nesta máquina que implementa `org.freedesktop.impl.portal.ScreenCast`:
   o do GNOME (mutter). `xdg-desktop-portal-gtk` está instalado mas não
   implementa essa interface, então não há para onde trocar.
5. **Configuração do WirePlumber** (o gerenciador de sessão do PipeWire
   responsável pelas políticas de negociação, incluindo modifiers) — sem
   nenhuma configuração exposta para isso na versão instalada (0.4.17).

## Ambiente onde foi reproduzido

- Ubuntu 24.04 "noble", sessão X11 (GNOME on Xorg) — `XDG_SESSION_TYPE=x11`.
- GPU: AMD Radeon RX 5700 XT (Navi 10), driver Mesa 25.2.8 (radeonsi).
- `libwebkit2gtk-4.1-0` 2.52.3-0ubuntu0.24.04.1.
- `gstreamer1.0-pipewire` 1.0.5-1ubuntu3.3, `pipewire` 1.0.5, `wireplumber` 0.4.17.
- `xdg-desktop-portal-gnome` 46.2-0ubuntu1 (único backend com `ScreenCast`).
- Tauri 2.11.5 / `wry` 0.55.1 (Cargo trava `wry` em `^0.55.0` — não é
  possível usar a API de permissões nova do `wry` 0.56.0, ver nota
  separada no histórico de commits de `desktop/src-tauri`).

## Caminhos não explorados (mais invasivos, não tentados)

- Compilar `gstreamer1.0-pipewire` e/ou `webkitgtk` de uma versão
  diferente na mão (fora dos pacotes do Ubuntu) — arriscado, pode quebrar
  outras coisas que dependem dessas libs no sistema.
- Reportar como bug upstream (WebKitGTK e/ou `gstreamer1.0-pipewire`) e
  esperar correção — não desbloqueia agora, mas é o caminho "correto" a
  longo prazo.
- Testar em uma sessão Wayland em vez de X11 (o caminho de captura via
  portal é desenhado primariamente para Wayland; não sabemos se o mesmo
  bug de negociação acontece lá — não testado).

## Estado atual

Compartilhar tela de dentro da janela Tauri está bloqueado por esse bug,
específico da combinação PipeWire+Mesa+WebKitGTK desta máquina. O resto
da casca Tauri (janela, bandeja, permissão de mídia agora concedida
corretamente) funciona normalmente — ver
`docs/superpowers/specs/2026-08-21-tauri-desktop-shell-design.md`.
Compartilhamento continua funcionando normalmente pelo navegador comum
(Chrome), que não tem esse problema.

Retomar esta investigação faz sentido se: (a) o Ubuntu atualizar
`gstreamer1.0-pipewire`/`webkitgtk` para uma versão que corrija a
negociação de modifiers, (b) surgir disposição para compilar essas libs
manualmente, ou (c) o objetivo mudar para testar em Wayland.
