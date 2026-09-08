#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Bootstrap do `schematize-updater` — APOSENTADO (ADR-0013).
#
# O QUE ESTE ARQUIVO FAZ HOJE: avisa que o updater deixou de existir e encaminha para o
# sucessor. Ele NAO baixa mais binario nenhum.
#
# POR QUE ELE CONTINUA AQUI, EM VEZ DE SER APAGADO
#
# Este caminho e citado em documentacao, em historico de terminal e em bookmark de gente
# que instalou a casa. Um `curl | bash` que devolve 404 nao diz nada: a pessoa conclui que
# o projeto sumiu. Um que diz "o gestor agora e outro, o comando e este" custa cinco linhas
# e resolve. Nome morto que se apaga e o que quebra (§37.48).
#
# POR QUE SAI 1 E NAO 0: quem chamou isto queria um gestor instalado, e ele NAO foi. Sair 0
# faria um script automatizado seguir em frente achando que deu certo.
# ---------------------------------------------------------------------------
set -euo pipefail

cat >&2 <<'MSG'
✗ o schematize-updater foi APOSENTADO.

  Ele foi absorvido pelo `schematize-market`, que agora e o unico responsavel por
  INSTALAR e ATUALIZAR tudo do ecossistema (ADR-0013). Nada de novo passa por aqui.

  O que rodar no lugar:

    curl -fsSL https://raw.githubusercontent.com/schematizeme/schematize-cli/main/install.sh | bash

  Esse comando instala o schematize e o gestor. Depois disso, o dia a dia e:

    schematize-market update      # atualiza tudo
    schematize-market status      # o que esta instalado, e o que ha de novo
    schematize-market list        # tudo que da pra instalar

  Ja tem o updater antigo na maquina? Ele continua funcionando, mas nao recebe mais
  correcao. O market o remove sozinho quando assume, e diz que fez isso.
MSG
exit 1
