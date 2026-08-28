# Notas de versão — Screen Share Desktop

Cada versão nova do app desktop tem uma seção aqui, escrita para quem usa
o app (sem detalhes técnicos). O CI publica a versão do topo desta lista
como texto da release no GitHub.

Regras:

- Toda alteração em `desktop/` sobe o número da versão em
  `desktop/package.json` e ganha uma seção correspondente aqui.
- A seção mais recente fica no topo, com o formato `## X.Y.Z`.
- Suba o último número para correções, o número do meio para novidades.

## 0.2.0

- O compartilhamento rápido pela bandeja agora avisa, com uma notificação
  do sistema, assim que a transmissão entra no ar e o link da sala é
  copiado para a área de transferência.

## 0.1.0

- Primeira versão: sala persistente com senha, compartilhamento de tela
  pela bandeja, áudio do sistema no Linux e no Windows.
