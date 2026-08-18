# Compartilhamento de tela P2P — Design

## Contexto e objetivo

Um jeito simples de compartilhar a tela com amigos, estilo "screen share" do Discord — e somente isso: sem chat, sem áudio, sem voz, sem contas. Depois de discutir as opções, o escopo mínimo viável é **um site**, não um app instalável: tanto quem compartilha quanto quem assiste usam só o navegador (Windows e Linux cobertos automaticamente, sem build/empacotamento nativo).

## Escopo

- 1 pessoa compartilha a tela para um grupo de **2 a 5 espectadores** por sessão.
- Sem contas/login. Sem persistência entre sessões.
- Sem áudio, sem chat, sem gravação — só o vídeo da tela.
- NAT traversal: só STUN público (Google) na v1. TURN (relay) fica para uma v2, se algum amigo não conseguir conectar por rede restritiva (CGNAT/firewall corporativo).

## Arquitetura

Um único binário Rust (Leptos SSR sobre Axum) que:

1. Serve a aplicação Leptos (compilada para WASM), executada no navegador de todos os participantes.
2. Expõe um endpoint WebSocket (`/ws`) usado **só para sinalização** WebRTC (offer/answer/ICE candidates). O vídeo nunca passa por esse servidor.

O vídeo trafega P2P via WebRTC: quem compartilha abre uma `RTCPeerConnection` **por espectador** (fan-out direto do compartilhador, sem SFU) — adequado para o tamanho de grupo definido (2–5). Estado de salas vive só em memória no processo (`HashMap<RoomCode, Room>`), sem banco de dados; uma sala morre quando quem compartilha desconecta.

Deploy: um único binário Rust, atrás de um domínio com HTTPS (obrigatório para `getDisplayMedia`/WebSocket seguro em navegadores, exceto localhost).

## Componentes

**Backend (Rust, Axum + Leptos SSR):**
- `main.rs` — bootstrap do servidor Axum; registra rotas Leptos + rota `/ws`.
- `signaling.rs` — handler do WebSocket. Mantém `HashMap<RoomCode, Room>` em memória (protegido por lock), roteia mensagens `join` / `offer` / `answer` / `ice-candidate` / `leave` entre os peers da mesma sala. Não interpreta conteúdo de vídeo — só repassa JSON entre os peers certos.
- Geração de `RoomCode`: string curta aleatória (ex.: 8 caracteres alfanuméricos), criada quando quem compartilha inicia uma sessão.

**Frontend (Leptos, WASM):**
- **Página inicial (`/`)** — botão "Iniciar compartilhamento". Ao clicar: chama `getDisplayMedia` (via `web-sys`), abre WebSocket, servidor cria a sala, UI mostra o link `/r/<codigo>` para copiar e compartilhar (fora do app: WhatsApp, Discord, etc.). Aceita novos espectadores continuamente, criando uma `RTCPeerConnection` nova para cada um.
- **Página de sala (`/r/:codigo`)** — quem abre esse link entra como espectador: conecta ao WebSocket, participa da troca de sinalização, recebe o stream e renderiza num `<video>`. Não solicita câmera/microfone — só recebe vídeo.

## Fluxo de dados

1. **Iniciar** — quem compartilha abre `/`, clica "Iniciar" → navegador pede permissão de captura de tela → WebSocket abre, servidor gera `RoomCode` e cria a sala em memória → UI mostra o link.
2. **Convidar** — link é compartilhado fora do app.
3. **Entrar** — cada amigo abre o link → WebSocket conecta, envia `join <codigo>` → servidor confirma a sala e avisa quem está compartilhando que chegou um novo espectador.
4. **Conectar (WebRTC)** — quem compartilha cria uma `RTCPeerConnection` para esse espectador, gera `offer`, envia via WebSocket → servidor repassa → espectador responde com `answer` → troca de `ice-candidate`s dos dois lados até a conexão P2P direta se estabelecer.
5. **Assistir** — o vídeo passa a fluir P2P direto da tela de quem compartilha para cada `<video>` de espectador. O servidor não participa mais depois disso.
6. **Encerrar** — quem compartilha para ou fecha a aba → WebSocket fecha → servidor remove a sala e avisa os espectadores restantes ("sessão encerrada"). Se um espectador sai, sua conexão é só descartada — os demais continuam normalmente.

## Tratamento de erros

- **Sala não existe** (link errado/expirado): página de sala mostra "sessão não encontrada ou já terminou".
- **Navegador sem suporte** a `getDisplayMedia`: mensagem clara antes de tentar qualquer coisa.
- **Usuário cancela a seleção de tela**: volta ao estado inicial sem erro, permite tentar de novo.
- **Falha de ICE/conexão P2P** (ex.: rede restritiva sem TURN): após timeout, mostra "não foi possível conectar" só para o espectador afetado — os demais não são impactados.
- **WebSocket cai**: mostra "conexão perdida, recarregue a página" (sem retry automático — o `peer_id` é atribuído pelo servidor a cada conexão nova, então uma reconexão silenciosa não teria como retomar a `RtcPeerConnection` já negociada; recarregar já refaz o fluxo do zero corretamente). Como o vídeo já é P2P direto após o handshake, uma queda do WebSocket depois de conectado não interrompe quem já está assistindo.

## Testes

- **Automatizado**: testes unitários em `signaling.rs` para a lógica de sala — criar, entrar, roteamento de mensagens entre os peers corretos, remoção ao desconectar. Não depende de browser, só `HashMap` + canais.
- **Manual**: captura de tela, handshake WebRTC e exibição de vídeo são validados manualmente — abrir a página em duas abas/máquinas diferentes, compartilhar, confirmar recebimento do vídeo pelo espectador, testar "encerrar sessão" e "espectador saindo".

## Fora de escopo (v1)

- Áudio, chat, gravação, contas/login.
- TURN/relay (só STUN por agora).
- Suporte a mais de ~5 espectadores simultâneos (exigiria SFU).
- App nativo instalável — tudo roda no navegador.
