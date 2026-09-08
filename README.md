# schematize-updater — APOSENTADO

> ## ⚠ Este repositório está ARQUIVADO
>
> O `schematize-updater` foi **absorvido pelo [`schematize-market`](https://github.com/schematizeme/schematize_market_rs)**
> em 2026-09-08, pelo [ADR-0013](https://github.com/schematizeme/schematize_app_archive/blob/main/decisoes/ADR-0013-market-dono-de-instalar-e-atualizar.md).
> O market e hoje o unico responsavel por **instalar e atualizar** tudo do ecossistema.
>
> ### Para instalar ou atualizar, use:
>
> ```sh
> # instalacao (primeira vez)
> curl -fsSL https://raw.githubusercontent.com/schematizeme/schematize-cli/main/install.sh | bash
>
> # dia a dia
> schematize-market update      # atualiza tudo
> schematize-market status      # o que esta instalado, e o que ha de novo
> schematize-market pin 0.62.0  # fixa uma versao; `pin latest` desafixa
> ```
>
> Os verbos sao os MESMOS que eram aqui — quem tem o comando na mao troca so o nome do
> programa. O pin gravado por este updater continua sendo lido pelo market, no mesmo
> caminho e no mesmo formato: quem fixou uma versao nao a perde na migracao.
>
> ### Ja tem o binario antigo na maquina?
>
> Ele continua funcionando, mas **nao recebe mais correcao**, e ele responde — um
> `schematize-updater update` numa maquina onde o market ja assumiu roda um gestor
> congelado e desfaz o que o novo fez. O market e o `install.sh` o removem sozinhos ao
> assumir, dizendo o que fizeram. Para tirar a mao: `rm ~/.cargo/bin/schematize-updater`.
>
> ### Por que o repositorio NAO foi deletado
>
> Porque o historico e a prova de como a casa chegou aqui. As 2.085 linhas daqui foram
> **movidas** para o market (nao reescritas — ver o D4 do ADR-0013), e o `git log` deste
> repo e o unico lugar onde se ve por que cada uma delas ficou daquele jeito. Um repo
> deletado leva junto a resposta de "por que isso e assim".

---

## O que ele era (documentacao historica)

Bootstrapper e gestor de versão do ecossistema **schematize** — instala e mantém o app
(`schematize` CLI + `schematize-gui`) atualizado na máquina do usuário, **cross-OS**
(Linux / macOS / Windows), de forma **híbrida**: usa o binário pré-compilado quando existe e
roda na plataforma; senão **compila do fonte** (instala rustup + libs de build e faz
`cargo install`).

É **desacoplado** do app de propósito: um app quebrado não trava a atualização, e o updater é o
único artefato que publicamos pré-compilado por SO — pequeno, estável, recompilado **só quando ele
mesmo muda** (via CI, `.github/workflows/release.yml`, no push de uma tag `v*`).

## Instalar (usuário final)

**Linux / macOS:**
```sh
curl -fsSL https://raw.githubusercontent.com/schematizeme/schematize-updater/main/bootstrap/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/schematizeme/schematize-updater/main/bootstrap/install.ps1 | iex
```

O bootstrap baixa o binário do updater e roda `schematize-updater install`, que instala o app.

## Comandos

```
schematize-updater install [--force]   # instala o app (CLI + GUI)
schematize-updater update  [--force]   # atualiza pra versão-alvo (pin, ou a última publicada)
schematize-updater status              # versões (updater, app, alvo), plataforma e pin
schematize-updater run                 # lança a GUI instalada
schematize-updater pin <versão>        # fixa uma versão do app (unpin volta a seguir latest)
schematize-updater version             # versão do próprio updater
```

## Como decide (híbrido)

1. Resolve a **versão-alvo** (pin, ou o `version` do `Cargo.toml` no `main` do app — sem API 60/h).
2. **Binário**: baixa o asset da plataforma do release do app; **verifica que executa** aqui
   (`--version`) — protege contra binário de glibc/arch incompatível que brickaria a troca.
3. Se não há binário compatível → **fonte INCREMENTAL**: garante rustup + libs de build do SO,
   mantém um checkout persistente por repo (`~/.schematize/updater/build/`) e roda `git fetch` +
   `cargo build --release` — reaproveita o `target/`, então só o que mudou recompila (as deps
   pesadas tipo Slint ficam cacheadas; update vira segundos, não minutos). Copia o binário pro
   `~/.cargo/bin`. NÃO usa `cargo install --force` (que jogava fora o cache toda vez).

## Build local

```sh
cargo build --release        # target/release/schematize-updater
cargo test
```

Zero dependências de crate (só `std` + shell-out) — cross-compila e vira musl-static sem dor.
