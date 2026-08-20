# Sala estilo Discord — assistir sob demanda, identidade visual e salas recentes (v3)

## Contexto e objetivo

O v2 (já implementado, nesta mesma branch) deu à sala um código com senha, multi-
usuário e persistência enquanto houver pelo menos um membro dentro. Mas duas coisas
ainda são rígidas: (1) assim que alguém compartilha a tela, o vídeo é automaticamente
mandado pra todo mundo na sala — não dá pra escolher quem assistir; e (2) não existe
identidade visual nem memória de salas visitadas — cada visita começa do zero, sem
saber se o link ainda é válido antes de tentar entrar.

Este v3 resolve isso: adiciona um nome obrigatório e compartilhado pra cada sala,
lembra localmente (no navegador de cada pessoa) das salas recentes e avisa na hora se
um link não é mais válido, e substitui o "todo mundo vê tudo automaticamente" por um
modelo de assistir sob demanda — igual chamada de vídeo do Discord: você vê quem está
na sala e quem está compartilhando, mas só vê a tela de alguém se clicar pra assistir,
e pode parar de assistir a qualquer momento sem afetar quem mais está assistindo.
Também adiciona escolha de nick + cor com um avatar simples (inicial do nick sobre um
círculo colorido), sobe o limite de sala de 8 para 10 pessoas, e adiciona um botão
explícito de sair da sala.

Este documento estende — não substitui — o design do v2
(`docs/superpowers/specs/2026-08-18-sala-persistente-multi-usuario-design.md`), que
continua valendo para tudo que não é mencionado aqui (sala com senha, sem hierarquia
entre membros, sala morre só quando o último membro sai, sem áudio, sem TURN/SFU).

## Escopo

**Dentro do v3:**
- **Nome da sala**, obrigatório, definido por quem cria, compartilhado com todo mundo
  que entra (salvo no servidor junto com a sala — mesma vida útil: existe enquanto a
  sala existir). O código continua sendo o identificador único da URL; o nome é só um
  rótulo de exibição, sem exigir unicidade entre salas diferentes.
- **Verificação imediata de sala existente**: antes de mostrar o formulário de
  nick/senha, a página da sala confere via HTTP se o código ainda é válido. Se não
  for, mostra o aviso na hora — sem abrir WebSocket, sem digitar nada.
- **Salas recentes**: toda sala criada ou entrada fica salva no `localStorage` do
  navegador (código, nome, nick usado da última vez) e aparece numa lista na home,
  mais recente primeiro, com um teto de 10 entradas. A senha **não** é salva — entrar
  numa sala recente ainda pede a senha, exatamente como entrar por um link qualquer.
  Ao abrir a home, cada sala da lista é checada (reaproveitando a mesma verificação
  acima) e as que não existem mais somem da lista.
- **Identidade visual**: ao criar ou entrar numa sala, além do nick, a pessoa escolhe
  uma cor de uma paleta fixa (~10 opções). Nick + cor ficam salvos no `localStorage`
  (substituindo o que hoje só guarda o nick) e pré-preenchidos da próxima vez. Cada
  membro tem um avatar circular com a primeira letra do nick em maiúsculo sobre um
  fundo da cor escolhida — no estilo do avatar padrão que o Discord gera pra contas
  novas. Esse avatar não é customizável além de mudar o nick (a letra e a cor seguem
  automaticamente).
- **Assistir sob demanda**: compartilhar a tela deixa de enviar vídeo pra todo mundo
  automaticamente. Em vez disso, só acende um botão "Assistir compartilhamento" no
  card de quem está compartilhando, pra cada outro membro. Clicar nesse botão é o que
  de fato abre a conexão de vídeo com aquela pessoa (só com ela); "Parar de assistir"
  fecha só essa conexão específica, sem afetar mais ninguém que esteja assistindo a
  mesma pessoa.
- **Cards de membro**: cada pessoa tem um card num slot da grade da sala. Borda e
  fundo usam a cor escolhida (borda na cor cheia, fundo na mesma cor escurecida e com
  baixa opacidade); o nick sempre em texto claro/neutro, não na cor da pessoa, pra
  garantir legibilidade em qualquer cor da paleta. O card alterna entre mostrar o
  avatar (parado, ou compartilhando mas não sendo assistido por você) e mostrar o
  vídeo de fato (quando você está assistindo aquela pessoa, ou é o seu próprio card e
  você está compartilhando).
- **Expandir/encolher**: qualquer card com vídeo (o seu, ou o de alguém que você está
  assistindo) pode ser expandido — ele ocupa a tela em foco e todo o resto (cards
  parados, compartilhando-não-assistido, ou sendo assistidos) encolhe numa tirinha.
  É um estado local, só seu — não afeta o que os outros veem. Encolher volta pra
  grade normal.
- **Esconder o próprio preview**: quando você está compartilhando, seu próprio card
  tem um botão extra pra esconder a exibição local do seu vídeo (sem parar de
  compartilhar de verdade) — libera espaço na tela sem afetar quem está te
  assistindo.
- **Capacidade**: sobe de 8 para 10 membros simultâneos por sala.
- **Sair da sala**: botão explícito no cabeçalho da sala que fecha a conexão e volta
  pra home.

**Sem banco de dados.** Nada neste escopo precisa de persistência além da vida do
processo do servidor (nome da sala é só mais um campo no registro em memória que já
existe, com a mesma vida útil da sala) ou do navegador de cada pessoa (salas recentes
e identidade visual são só `localStorage`, não sincronizam entre dispositivos — não
foi pedido). O modelo "tudo em memória, descartado no restart" documentado no
`CLAUDE.md` continua valendo sem alterações.

**Fora de escopo do v3 (motivo):**
- **Sincronizar salas recentes entre dispositivos.** Exigiria conta de usuário e
  algum armazenamento no servidor — não foi pedido, e vai contra o "sem contas" que
  já é uma decisão deliberada do v2.
- **Notificação em tempo real de que uma sala morreu** para quem não está nela. Sem
  um canal aberto com aquele navegador não tem como avisar; a lista de salas recentes
  só se atualiza quando a pessoa volta pra home (ver "Riscos e lacunas conhecidas").
- **Impedir duas pessoas de escolherem a mesma cor** na mesma sala. Não foi pedido;
  colisão de cor é só uma coincidência visual, não quebra nada funcionalmente.
- **Qualidade adaptativa / limitar quantas pessoas podem assistir a mesma
  transmissão.** Cada conexão de "assistir" é independente; não há um teto proposto
  além do limite geral de 10 membros na sala.

## Arquitetura

### Servidor: endpoint HTTP de verificação (fora do WebSocket)

Uma rota nova, `GET /api/rooms/:code`, registrada ao lado do `ws_handler` existente em
`src/main.rs`, consultando o mesmo `Registry` em memória (sem autenticação — só
existência é informação pública o suficiente pra decidir se vale a pena tentar
entrar). Resposta JSON, sempre `200 OK`:

```json
{ "exists": true, "name": "Sala dos lindos", "member_count": 3, "max_members": 10 }
```

Quando `exists` é `false`, os demais campos são omitidos. Essa rota é usada em dois
lugares do cliente: (1) a página da sala, ao montar, antes de mostrar o formulário de
entrada; (2) a home, ao montar, uma chamada por sala da lista de recentes, pra podar
as que sumiram.

### Registro de salas (`src/signaling/registry.rs`)

- `Room` ganha um campo `name: String`, definido na criação e imutável depois (sem
  recurso de renomear).
- `MAX_MEMBERS` sobe de `8` para `10`.
- `Registry::create_room` ganha um parâmetro `room_name: String`.
- Nenhuma mudança na forma como salas são removidas (continua: sala some do mapa só
  quando o último membro sai).

### Protocolo (`src/signaling/protocol.rs`)

- `MemberInfo` ganha `color: String` (um identificador curto da paleta, ex. `"coral"`
  ou o hex direto — decidido na implementação; não precisa ser hex necessariamente,
  já que a paleta é fixa).
- `ClientMessage::CreateRoom` ganha `room_name: String` e `color: String`.
- `ClientMessage::JoinRoom` ganha `color: String`.
- `ServerMessage::Joined` ganha `room_name: String`.
- Dois pares de mensagens novos, roteados pelo servidor exatamente como
  `Offer`/`Answer`/`IceCandidate` já são hoje (servidor não interpreta, só relaia
  peer-a-peer):
  - `ClientMessage::WatchShare { sharer_id: String }` →
    `ServerMessage::WatchRequested { from: String }` (entregue só ao `sharer_id`).
  - `ClientMessage::StopWatching { sharer_id: String }` →
    `ServerMessage::WatchStopped { from: String }` (entregue só ao `sharer_id`).

`StartShare`/`StopShare` continuam existindo com o mesmo papel de hoje (anunciar
"estou compartilhando" pra acender/apagar o botão de assistir no card de todo mundo);
o que muda é que eles **não** disparam mais criação de conexão nenhuma — só atualizam
o registro e notificam via `PeerStartedSharing`/`PeerStoppedSharing`, como já fazem.

### Cliente: de onde vem cada conexão agora

Hoje (`src/pages/room.rs`, Task 8), `share_toggle_handler` cria uma `RTCPeerConnection`
de saída para **cada outro membro da sala**, assim que o compartilhamento começa. Isso
muda para: `share_toggle_handler` só captura a tela, guarda o stream local e manda
`StartShare` — nenhuma conexão é criada ainda. A criação de conexão de saída passa a
acontecer em reação a `ServerMessage::WatchRequested` (chegando via
`build_message_handler`, que já roteia `Offer`/`Answer`/`IceCandidate` da mesma
forma): cria uma `RTCPeerConnection` só para aquele `from`, anexa as tracks do stream
local já guardado, manda a oferta. Um clique em "Assistir" no lado do espectador
manda `WatchShare`; um clique em "Parar de assistir" manda `StopWatching` e fecha a
conexão de entrada correspondente no próprio lado (o mesmo `conn.incoming` que já
existe).

A limpeza ao parar de compartilhar (`stop_sharing`, que hoje já fecha tudo em
`conn.outgoing.borrow_mut().drain()`) não muda — continua fechando todas as conexões
de saída que existirem no momento, não importa se foram abertas via fan-out (v2) ou
via pedido individual (v3).

### Cliente: `localStorage`

- O que hoje é só `load_nick`/`save_nick` (`src/client/storage.rs`) vira um "perfil"
  com nick + cor (`load_profile`/`save_profile`), usado tanto na home quanto na sala.
- Uma lista de salas recentes, capada em 10 entradas, mais recente primeiro:
  `{ code: String, name: String, nick: String }` por entrada (sem senha). Atualizada
  toda vez que `Joined` é recebido (criação ou entrada bem-sucedida) e podada na home
  a cada carregamento, via o endpoint HTTP acima.

### Cliente: estrutura visual da sala (`src/pages/room.rs`)

Cada membro (inclusive você mesmo) tem um card num slot fixo de uma grade — mesmo
container que já existe, mas o conteúdo de cada `<For>` item passa a ser um
componente de card com estado derivado de três coisas: é você mesmo, está
compartilhando (`RoomMember.sharing`, já existe), e você está assistindo essa pessoa
(estado novo, só local: um `HashSet<String>` de peer_ids que você optou por assistir).
Não sendo você e não estando sendo assistida, mostra avatar (círculo com a cor +
inicial do nick) parado, com o botão de assistir se estiver compartilhando. Sendo
assistida (ou sendo o seu próprio card enquanto compartilha), mostra o `<video>` no
lugar do avatar.

"Expandido" é outro estado local — um `Option<String>` com o peer_id em foco (ou
`None`); quando `Some`, a view troca a grade normal por: um card grande com aquele
peer_id + uma tirinha horizontal com miniaturas de todos os cards (incluindo os
parados). Isso é puramente de apresentação — não manda nem recebe nenhuma mensagem de
protocolo, só reorganiza o que já está renderizado.

Assim como o portão de autenticação e o botão de compartilhar (v2), qualquer parte
clicável que capture o `RoomConnection` (não `Send + Sync`) precisa ficar fora de
`<Show>`/`<For>`-com-fallback-dinâmico e alternar por `class:hidden` — o mesmo padrão
já estabelecido nas Tasks 7 e 8, que continua valendo aqui pros botões de
assistir/parar/expandir de cada card.

## Fluxo de dados

**Criar sala:** home pede nick, cor, nome da sala e senha → `CreateRoom{nick, color,
room_name, password}` → servidor cria a sala e devolve `Joined{..., room_name}` →
sala é adicionada às salas recentes locais → navega pra `/r/<código>` já autenticado
(reaproveitando o handoff de sessão do v2 via `client::session`).

**Abrir um link de sala:** `RoomPage` monta → chama `GET /api/rooms/:code` → se
`exists: false`, mostra o aviso de sala inexistente imediatamente, sem formulário →
se `exists: true`, mostra nome da sala + formulário de nick/cor/senha (ou pula direto
se vier de uma sessão pendente da home, como já acontece).

**Compartilhar e assistir:** A compartilha (`StartShare`, sem fan-out) → todo mundo
recebe `PeerStartedSharing`, o card da A acende o botão de assistir pros outros → B
clica assistir → `WatchShare{sharer_id: A}` → A recebe `WatchRequested{from: B}` →
A cria conexão de saída só pra B, manda oferta → B recebe a oferta, cria conexão de
entrada, responde → vídeo flui só entre A e B. C não vê nada da A até clicar assistir
também, independentemente do que B está fazendo.

**Sair vs. sala morrer:** "Sair da sala" fecha a conexão e navega pra home — a sala
continua existindo pros outros membros (comportamento do v2, sem mudança). Só quando
o último membro sai o servidor remove a sala; quem tinha ela nas recentes só descobre
isso na próxima vez que abrir a home (a lista se atualiza sozinha nesse momento).

## Tratamento de erros

- **Sala não existe** (link direto ou "sala recente" morta): detectado no `GET
  /api/rooms/:code` antes de qualquer formulário — mesma mensagem de hoje ("Sala não
  encontrada ou já foi encerrada."), só que mais cedo.
- **Nome da sala em branco:** mesma validação client-side que já existe pra nick e
  senha (bloqueia o envio, mostra "Preencha todos os campos.").
- **`WatchShare` chega depois que a pessoa já parou de compartilhar** (corrida entre
  clicar assistir e a outra pessoa parar): o lado que recebe `WatchRequested` decide
  com base no seu próprio estado local (`is_sharing`) se cria a conexão ou ignora —
  não depende do servidor validar isso, evitando que o registro precise saber "quem
  pediu pra assistir quem" (ele só relaia, como já faz com `Offer`/`Answer`).
- **Sala cheia (10 membros):** mesmo caminho de erro que já existe
  (`ServerMessage::RoomFull`), só ajustando o texto da mensagem pro novo número.

## Riscos e lacunas conhecidas

- **Lista de salas recentes fica desatualizada até a próxima visita à home** — é uma
  limitação de não ter um canal aberto com navegadores que não estão conectados no
  momento. Aceitável: o pior caso é ver por alguns instantes uma sala que já morreu
  na lista, e ela some assim que a pessoa reabre a home.
- **Duas pessoas podem escolher a mesma cor** na mesma sala — sem checagem de
  unicidade (decisão consciente, ver Escopo). Visualmente pode confundir num grupo
  grande, mas o nick abaixo do avatar sempre desambigua.
- **Inicial do avatar com nicks incomuns** (nick vazio pós-trim, emoji, ou só
  caracteres não-alfabéticos): usar o primeiro caractere Unicode do nick tratado
  (`chars().next()`), maiúsculo quando aplicável; se o nick ficar vazio a validação
  de formulário já impede o envio, então esse caso não deveria surgir em uso normal.
- **Pior caso de conexões simultâneas** sobe de 8 pra 10 membros (até 90 conexões
  P2P se todo mundo compartilhar e todo mundo assistir todo mundo), mas o modelo sob
  demanda desta v3 na prática **reduz** o número típico de conexões abertas em
  comparação com o fan-out automático do v2, já que agora só existem conexões para
  pares que ativamente se escolheram.

## Testes

**Automatizado (sem navegador), junto com o que já existe:**
- Registro: `create_room` aceita e armazena `name`; capacidade efetiva de 10;
  `JoinedSnapshot`/mensagens incluem o nome da sala.
- Protocolo: (de)serialização JSON de `WatchShare`/`StopWatching`/
  `WatchRequested`/`WatchStopped`, e dos campos novos em `CreateRoom`/`JoinRoom`/
  `MemberInfo`/`Joined`.
- Endpoint HTTP `GET /api/rooms/:code`: existente retorna `exists: true` com os
  campos certos; inexistente retorna `exists: false`.
- Relay: uma mensagem `WatchShare` dirigida a um peer específico chega só nele (mesmo
  padrão de teste que já existe pra `Offer`/`IceCandidate` em `tests/signaling_ws.rs`).

**Manual, em navegador real (como já é para tudo que é WebRTC/captura de tela):**
- Fluxo completo: criar sala com nome+cor, ver ela aparecer em salas recentes, sair,
  reabrir a home e confirmar que ela ainda está lá com o nome certo.
- Abrir um link de sala morta (ou uma sala recente cuja última pessoa já saiu) e
  confirmar o aviso imediato, sem formulário.
- Duas pessoas compartilhando ao mesmo tempo, uma terceira assistindo só uma delas —
  confirmar que a que não foi escolhida nunca inicia transferência de vídeo.
- Parar de assistir uma pessoa enquanto outra ainda assiste a mesma — confirmar que
  a conexão da segunda pessoa não é afetada.
- Expandir/encolher, e esconder/mostrar o próprio preview — confirmar que são estados
  visuais locais (não mudam nada pro resto da sala).
