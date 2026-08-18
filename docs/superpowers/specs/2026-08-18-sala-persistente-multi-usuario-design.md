# Sala persistente multiusuário com senha — Design (v2)

## Contexto e objetivo

O v1 (já implementado) é um compartilhamento 1-para-N: uma pessoa ("host") compartilha
a tela, gera um link efêmero, e a sala morre quando ela sai. Este v2 transforma isso
numa "call" de compartilhamento de tela pura, sem áudio nem vídeo de câmera: uma sala
com ID fixo e senha, onde **qualquer** participante pode compartilhar sua tela a
qualquer momento, todos veem simultaneamente as transmissões ativas dos outros numa
grade, e a sala continua existindo enquanto tiver pelo menos uma pessoa dentro —
independente de quem foi a pessoa que a criou.

Este documento substitui, para as áreas que ele cobre, as decisões de escopo do v1
registradas em `docs/superpowers/specs/2026-08-18-screen-share-design.md` e no
`CLAUDE.md` atual (host único, sem contas, sala efêmera atrelada ao host, sem áudio
por natureza da v1). O `CLAUDE.md` será atualizado como parte do plano de
implementação para refletir essas mudanças.

## Escopo

**Dentro do v2:**
- Sala identificada por um código gerado pelo sistema (8 caracteres, mesmo formato do
  v1) e protegida por senha, definida por quem cria.
- Todo participante — inclusive quem cria a sala — informa **nick + senha** antes de
  entrar.
- Nick é salvo em `localStorage` do navegador para não precisar redigitar; a senha
  nunca é persistida no navegador, é digitada a cada entrada.
- A sala é removida da memória do servidor só quando o último membro sai. Sair de
  quem criou não afeta a sala nem os demais.
- Qualquer membro pode iniciar/parar seu próprio compartilhamento de tela a qualquer
  momento, quantas vezes quiser, enquanto estiver na sala.
- Todo membro vê, automaticamente e ao mesmo tempo, uma grade com o vídeo de todas as
  transmissões ativas no momento (de qualquer outro membro que esteja compartilhando).
- Quem está compartilhando vê um preview local (mudo, renderizado só no próprio
  navegador, sem envolver o servidor) da própria transmissão.
- Sem hierarquia entre participantes: quem sabe a senha e entra tem exatamente os
  mesmos poderes que qualquer outro membro (compartilhar, assistir, sair). Não existe
  "dono da sala" com poderes especiais.
- Limite de **8 membros simultâneos por sala**; tentativas de entrar acima disso são
  recusadas com uma mensagem clara.
- Sem contas de usuário — só nick (texto livre, sem unicidade obrigatória) e a senha
  compartilhada da sala.

**Fora de escopo do v2 (motivo):**
- **Áudio do compartilhamento** (e por consequência, a exclusão do app Discord do
  áudio e o controle de volume por espectador, que dependem de o compartilhamento ter
  áudio). Não existe API de navegador que capture "todo o áudio do sistema exceto um
  processo específico" — só é possível capturar o sistema inteiro (majoritariamente
  Windows; suporte inconsistente no Linux) ou o áudio de um único app/janela
  escolhido, nunca uma exclusão seletiva. Isso só é tecnicamente viável com APIs
  nativas do sistema operacional (ex.: captura de loopback por processo no Windows),
  que exigem um app nativo — fora do alcance de uma aplicação rodando só no navegador.
  Fica anotado para uma v3, possivelmente já junto da portabilidade para app desktop.
- **App desktop / bandeja do sistema (Tauri)**. A arquitetura deste v2 mantém toda
  lógica específica de navegador isolada atrás do gate `#[cfg(feature = "hydrate")]`
  (como já é hoje), para que um shell Tauri futuro possa reaproveitar o protocolo de
  sinalização e a maior parte da lógica de estado, trocando só a captura de tela por
  uma API nativa. Nenhuma ação concreta é tomada nesta fase além de preservar esse
  isolamento.
- **TURN/relay** e **SFU** — mantidos fora, como no v1. O grupo pequeno (até 8
  membros) e a malha P2P direta via STUN público são suficientes; um SFU resolveria
  escalabilidade para grupos maiores, mas é um salto de complexidade desproporcional
  ao pedido atual.
- **Rate limiting de tentativas de senha** — ver seção de Riscos.

## Arquitetura

Evolução direta da arquitetura do v1: mesmo binário Rust único (Leptos SSR sobre
Axum), mesmo papel do WebSocket `/ws` como canal só de sinalização (nunca carrega
vídeo), mesma malha P2P direta via `RTCPeerConnection` sem SFU. A mudança central é
que o conceito de "host" deixa de existir — qualquer membro da sala pode assumir o
papel de sharer para qualquer outro membro, a qualquer momento.

**Modelo de conexões:** para cada par ordenado (A compartilhando → B assistindo)
existe uma `RTCPeerConnection` própria, sempre com A como quem oferta (`offer`) e B
como quem responde (`answer`). Se A e B estiverem compartilhando um para o outro ao
mesmo tempo, são duas conexões distintas — uma por direção — o que evita duas ofertas
disputando a mesma conexão ("glare"). Cada cliente mantém dois mapas de conexões: um
para os pares em que ele é o sharer (uma entrada por espectador atual) e outro para os
pares em que ele é espectador (uma entrada por sharer ativo).

**Registro da sala (servidor, em memória, sem banco de dados):**

```
Room {
    id: String,
    password_hash: String,        // argon2, nunca texto puro
    members: HashMap<PeerId, Member { nick, sender }>,
    sharers: HashSet<PeerId>,     // quem está compartilhando agora
}
```

`leave` remove o membro do `HashMap`; se ele estava em `sharers`, os demais são
avisados para encerrar aquele tile. A sala inteira só é removida do registro global
quando `members` fica vazio — essa é a definição operacional de "sala fixa" pedida:
sobrevive à saída de qualquer membro individual, inclusive de quem a criou, e só
desaparece quando ninguém mais está dentro. Não sobrevive a um reinício do processo
do servidor (sem persistência em disco/banco nesta fase).

## Protocolo de sinalização (extensão do existente)

**Cliente → servidor:**
- `CreateRoom { nick, password }` — cria a sala, gera o ID, o criador já entra como
  primeiro membro.
- `JoinRoom { room, nick, password }` — entra numa sala existente.
- `StartShare` / `StopShare` — anuncia início/fim do próprio compartilhamento.
- `Offer { to, sdp }`, `Answer { to, sdp }` — mesmo papel do v1, agora usados por
  qualquer par sharer→viewer, não só pelo host.
- `IceCandidate { to, stream_owner, candidate, sdp_mid, sdp_m_line_index }` — o campo
  novo `stream_owner` (o `peer_id` de quem está compartilhando naquela conexão
  específica) desambigua entre as duas conexões possíveis quando A e B compartilham
  um para o outro simultaneamente.

**Servidor → cliente:**
- `Joined { peer_id, members: [{peer_id, nick}], active_sharers: [peer_id] }` —
  snapshot do estado atual da sala, enviado a quem acabou de entrar (via `CreateRoom`
  ou `JoinRoom` bem-sucedido), para o cliente saber quem já está lá e quem já está
  compartilhando.
- `AuthFailed` — senha incorreta.
- `RoomNotFound` — sala inexistente (nunca criada, ou já esvaziada e removida).
- `RoomFull` — sala já tem 8 membros.
- `PeerJoined { peer_id, nick }`, `PeerLeft { peer_id }` — broadcast para os demais
  membros.
- `PeerStartedSharing { peer_id }`, `PeerStoppedSharing { peer_id }` — broadcast para
  os demais membros.
- `Offer { from, sdp }`, `Answer { from, sdp }`, `IceCandidate { from, stream_owner,
  candidate, ... }` — roteados pelo `to` da mensagem correspondente do cliente.

**Fluxo de oferta generalizado:** quando alguém dá `StartShare`, seu cliente cria uma
`RTCPeerConnection` e envia `Offer` para cada membro atual da sala. Quando um novo
membro entra (`PeerJoined` do lado dos demais), todo cliente que estiver com uma
transmissão ativa no momento reage criando uma nova conexão e ofertando para esse
novo membro — a mesma lógica que hoje só o host executa ao receber `PeerJoined`,
generalizada para rodar em qualquer cliente que esteja com `sharers` incluindo a si
mesmo.

## Fluxo de dados

1. **Criar sala** — na página inicial (`/`), formulário pede nick + senha. Ao
   confirmar: WebSocket conecta, envia `CreateRoom{nick, password}`; servidor gera o
   ID, cria a sala com a senha em hash, responde `Joined{...}`; o cliente navega para
   `/r/<id>` já autenticado nesta aba (sem precisar digitar de novo na mesma sessão).
2. **Convidar** — o ID (ou o link `/r/<id>`) é compartilhado fora do app; a senha é
   combinada por fora também (ex.: mensagem separada).
3. **Entrar** — quem abre `/r/<id>` sem sessão ativa nessa aba vê o formulário de
   nick (pré-preenchido do `localStorage`, se existir) + senha. Ao confirmar: WS
   conecta, envia `JoinRoom{room, nick, password}`. Sucesso → `Joined{...}` com a
   lista de membros e de quem já está compartilhando. Falha de senha → `AuthFailed`,
   formulário continua visível para nova tentativa. Sala inexistente/cheia →
   `RoomNotFound`/`RoomFull`, sem formulário (não adianta tentar de novo).
4. **Compartilhar** — membro clica "Compartilhar minha tela" → `getDisplayMedia` →
   stream local vira preview (tile mudo, só local) → cliente envia `StartShare` →
   servidor faz broadcast de `PeerStartedSharing` → o cliente do sharer oferta para
   cada membro atual (fluxo de oferta generalizado acima).
5. **Assistir** — cada membro que recebe uma `Offer` cria a conexão correspondente,
   responde com `Answer`, troca `IceCandidate`s, e ao receber a track renderiza um
   tile na grade rotulado com o nick de quem está compartilhando. Vídeo trafega P2P
   direto; o servidor não participa depois da negociação.
6. **Parar de compartilhar** — clique em "Parar" ou o botão nativo do navegador
   ("Stop sharing") → cliente envia `StopShare`, fecha suas conexões de saída,
   remove o preview local → servidor faz broadcast de `PeerStoppedSharing` → os
   demais removem o tile correspondente.
7. **Sair** — fechar a aba ou desconectar o WebSocket → servidor remove o membro do
   registro, faz broadcast de `PeerLeft` (e de `PeerStoppedSharing` se ele estava
   compartilhando) para os demais. A sala só é removida do registro global quando
   isso deixa `members` vazio.

## Tratamento de erros

- **Senha incorreta**: `AuthFailed`, sem travar a sala — tentativas ilimitadas nesta
  fase (ver Riscos).
- **Sala não existe**: `RoomNotFound`, mensagem clara, sem formulário de retry.
- **Sala cheia**: `RoomFull`, mensagem clara.
- **Falha de ICE num par específico**: só o tile daquela conexão (aquele sharer, para
  aquele espectador) mostra "não foi possível conectar"; o resto da sala continua
  normal — mesmo princípio de isolamento por conexão que o v1 já tinha para o host,
  generalizado para qualquer par.
- **WebSocket cai**: mesmo padrão do v1 — sem reconexão automática, pede para
  recarregar a página (um novo `peer_id` seria atribuído, então uma reconexão
  silenciosa não teria como retomar as conexões WebRTC já negociadas). Como a sala é
  persistente e não depende de nenhum membro específico, a queda de um WebSocket não
  afeta os demais membros.
- **Navegador sem `getDisplayMedia`**: mesmo banner do v1, mas agora só desabilita o
  botão de compartilhar — a pessoa ainda pode entrar na sala e assistir às
  transmissões dos outros.

## Riscos e lacunas conhecidas

- **Sem rate limiting de tentativas de senha**: nada impede um script de tentar várias
  senhas em sequência contra uma sala via WebSocket. Para uma senha "simples" isso é
  uma exposição real. Mitigado só via orientação de UI (evitar senhas óbvias);
  rate limiting fica registrado como próximo passo de segurança, não implementado
  nesta fase.
- **Malha sem SFU no pior caso**: com os 8 membros permitidos todos compartilhando ao
  mesmo tempo, a sala chega a 56 conexões WebRTC simultâneas (cada cliente sustentando
  até 14 `RTCPeerConnection`s). Funciona para o tamanho de grupo combinado, mas é o
  teto de escala assumido — não escala além disso sem introduzir um SFU.

## Testes

- **Automatizado** (sem navegador, `cargo test --features ssr`): protocolo (round-trip
  JSON dos tipos novos/alterados), registry — criar sala com senha (hash nunca em
  texto puro nos dados armazenados), `JoinRoom` com senha certa e errada, `RoomFull`
  acima de 8 membros, `StartShare`/`StopShare` propagando só para os membros certos,
  sala removida do registro só quando o último membro sai (incluindo o caso de quem
  sai ser o criador — a sala continua existindo com os membros restantes).
- **Manual** (navegador real, como no v1 — não há harness de automação de browser
  neste repo): abrir 3+ abas; criar uma sala; entrar nas outras abas com senha certa
  e errada; iniciar compartilhamento em duas abas ao mesmo tempo e confirmar que a
  grade mostra os dois tiles nas demais; confirmar que o preview local aparece só
  para quem está compartilhando; recarregar uma aba e confirmar que o nick veio do
  `localStorage` mas a senha precisa ser digitada de novo; fechar a aba de quem criou
  a sala e confirmar que a sala continua para os demais; fechar todas as abas e
  confirmar (reabrindo com o mesmo ID) que a sala não existe mais.
