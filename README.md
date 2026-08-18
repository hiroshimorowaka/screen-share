# Compartilhamento de tela P2P

Site para compartilhar a tela com até 5 amigos ao mesmo tempo, direto do navegador
(Windows e Linux) — sem instalar nada, sem contas, sem áudio/chat. O vídeo trafega
P2P via WebRTC; o servidor só faz a sinalização inicial (troca de offer/answer/ICE).

## Rodando localmente

Pré-requisitos:

- Rust + `rustup target add wasm32-unknown-unknown`
- `cargo install cargo-leptos`

```bash
cargo leptos watch
```

Abra `http://127.0.0.1:3000/`.

## Testes automatizados

```bash
cargo test --features ssr
```

Cobre a lógica de sinalização (protocolo, registro de salas, endpoint WebSocket).
A captura de tela e o handshake WebRTC só existem dentro de um navegador real —
são validados manualmente (checklist abaixo).

## Checklist de teste manual (fluxo completo)

1. Abra `/` numa aba, clique "Iniciar compartilhamento", escolha uma janela/tela.
2. Confirme que aparece um link `/r/<código>`.
3. Abra esse link em outra aba (ou outra máquina) — confirme que o vídeo aparece
   em poucos segundos.
4. Abra o mesmo link numa terceira aba — confirme que ambos os espectadores
   recebem o vídeo simultaneamente.
5. Feche a aba de um espectador — confirme que os demais continuam recebendo
   vídeo normalmente.
6. Pare o compartilhamento (feche a aba de quem compartilha) — confirme que os
   espectadores restantes veem "O compartilhamento foi encerrado."
7. Abra um link com um código inexistente — confirme "Sessão não encontrada ou
   já terminou."

## Deploy

Este projeto compila para um único binário Rust. Em produção:

- Sirva atrás de HTTPS (obrigatório para `getDisplayMedia` e WebSocket seguro
  fora de `localhost`) — por exemplo, um reverse proxy como Caddy com TLS
  automático, ou uma plataforma que já termina TLS (Fly.io, Render).
- Não é necessário banco de dados nem armazenamento persistente — todo o
  estado de salas vive em memória e é descartado quando o processo reinicia.
- Sem TURN configurado (só STUN público). Se algum amigo estiver numa rede
  muito restritiva (CGNAT, firewall corporativo) e não conseguir conectar,
  isso é uma limitação conhecida da v1 — um servidor TURN (`coturn`) resolveria,
  mas fica fora de escopo por agora.
- O `.cargo/config.toml` fixa `LEPTOS_OUTPUT_NAME=screen_share` — necessário
  para o `cargo-leptos` 0.3.7 e o `leptos` 0.8 concordarem no nome do arquivo
  `.wasm` gerado (sem isso, o navegador tenta buscar um arquivo que não existe
  e a página nunca hidrata). Mantenha esse arquivo se atualizar dependências.
