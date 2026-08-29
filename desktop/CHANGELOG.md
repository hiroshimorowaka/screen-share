# Notas de versão — Screen Share Desktop

Cada versão nova do app desktop tem uma seção aqui, escrita para quem usa
o app (sem detalhes técnicos). O CI publica a versão do topo desta lista
como texto da release no GitHub.

Regras:

- Toda alteração em `desktop/` sobe o número da versão em
  `desktop/package.json` e ganha uma seção correspondente aqui.
- A seção mais recente fica no topo, com o formato `## X.Y.Z`.
- Suba o último número para correções, o número do meio para novidades.

## 0.3.0

- O app agora se atualiza sozinho no Windows: quando sai uma versão nova,
  ele baixa em segundo plano, avisa com uma notificação e instala na
  próxima vez que você fechar o app. (A versão portátil e o Linux
  continuam sendo atualizados baixando a release nova na mão.)
- Quando um programa volta a tocar som depois de um tempo em silêncio, o
  áudio dele entra na transmissão mais rápido — quase sem perder o
  comecinho do som.
- O ícone na bandeja agora mostra o estado: bolinha verde parada, bolinha
  vermelha enquanto você está transmitindo.

## 0.2.0

- O compartilhamento rápido pela bandeja agora avisa, com uma notificação
  do sistema, assim que a transmissão entra no ar e o link da sala é
  copiado para a área de transferência.

## 0.1.0

- Primeira versão: sala persistente com senha, compartilhamento de tela
  pela bandeja, áudio do sistema no Linux e no Windows.
