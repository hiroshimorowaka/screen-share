# Notas de versão — Screen Share Desktop

Cada versão nova do app desktop tem uma seção aqui, escrita para quem usa
o app (sem detalhes técnicos). O CI publica a versão do topo desta lista
como texto da release no GitHub.

Regras:

- Toda alteração em `desktop/` sobe o número da versão em
  `desktop/package.json` e ganha uma seção correspondente aqui.
- A seção mais recente fica no topo, com o formato `## X.Y.Z`.
- Suba o último número para correções, o número do meio para novidades.

## 0.3.8

- Correção: em alguns computadores Windows (máquinas virtuais, servidores,
  drivers de vídeo básicos) a janela "O que você quer compartilhar?" nunca
  aparecia e o botão de compartilhar tela parecia não fazer nada. A janela
  do seletor deixou de depender de um recurso de transparência que esses
  ambientes não têm, e a inicialização do app foi reforçada para que uma
  falha em outra parte (atualizador automático, ícone da bandeja) não
  impeça mais o seletor de funcionar.

## 0.3.7

- Correção: no Windows, clicar em "compartilhar tela" não abria o seletor
  quando o áudio de sistema não estava disponível no computador. Agora o
  compartilhamento de tela funciona de forma independente e, se o áudio
  não puder ser capturado, a transmissão segue só com vídeo.

## 0.3.6

- Correção interna: reorganização do código do seletor de tela e checagem
  automática de qualidade de código. Sem mudança visível no uso normal.

## 0.3.5

- Correção interna: limpeza de comentários no código. Sem mudança visível
  no uso normal.

## 0.3.4

- O botão "Convidar" volta a funcionar no app: ele não estava copiando o
  link da sala para a área de transferência.
- Compartilhar o áudio do sistema no Linux volta a funcionar — o
  indicador de áudio ficava preso em "Áudio desligado" mesmo com a opção
  marcada no seletor de tela.

## 0.3.3

- Correção interna: fecha uma brecha rara em que dispensar o seletor de
  tela no instante em que ele abria podia derrubar o processo principal
  do app. Sem mudança visível no uso normal.

## 0.3.2

- Corrige o seletor de tela, que parou de abrir na 0.3.1: o reforço de
  segurança daquela versão recusava permissões do navegador cedo demais e
  acabava barrando também a captura de tela. Agora só a captura de tela
  passa; câmera, microfone, localização e afins continuam recusados.

## 0.3.1

- Reforço de segurança do app. Nada muda no uso do dia a dia:
  - a janela do app fica presa ao site oficial — se algo tentar levá-la
    para outro endereço, é bloqueado;
  - só a própria tela do app pode acionar as funções nativas (áudio do
    sistema, cópia do link, notificações); qualquer outra origem é
    recusada;
  - o app nega qualquer pedido de permissão do navegador (câmera,
    microfone, localização e afins), que ele não usa;
  - a captura de áudio do sistema para sozinha quando a página recarrega,
    fecha ou trava, sem deixar processos ou ligações de som soltos;
  - no Windows, uma atualização automática só é aplicada se estiver
    assinada pela mesma origem do app.

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
