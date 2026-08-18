# schematize-updater

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
