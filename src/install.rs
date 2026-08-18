//! O núcleo HÍBRIDO: instala/atualiza o app tentando primeiro o binário pré-compilado
//! (rápido, sem toolchain) e, se faltar OU não rodar aqui (glibc/arch incompatível), COMPILA
//! do fonte na máquina do usuário (rustup + deps + cargo install do git). Cross-OS.
//! O quê: `install_or_update`. Onde: chamado por main.rs (install/update).

use crate::{fetch, platform, sh, version};
use std::path::Path;

/// Instala (1ª vez) ou atualiza o app. `force` reinstala mesmo já estando na versão-alvo.
pub fn install_or_update(force: bool) -> Result<(), String> {
    let target = version::target_version()
        .ok_or("não consegui resolver a versão-alvo (rede/GitHub indisponível?).")?;
    let installed = version::installed_app_version();

    if !force {
        if let Some(cur) = &installed {
            if cur == &target {
                println!("Já está na versão-alvo (v{target}). Nada a fazer.");
                return Ok(());
            }
        }
    }
    println!(
        "schematize-updater: alvo v{target} (instalado: {}).",
        installed.as_deref().unwrap_or("nenhum")
    );

    // 1) CAMINHO RÁPIDO — binário pré-compilado, se houver asset pra esta plataforma.
    if let Some((cli_asset, gui_asset)) = platform::asset_names() {
        match try_binary(&target, cli_asset, gui_asset) {
            Ok(true) => {
                finish(&target);
                return Ok(());
            }
            Ok(false) => println!("→ sem binário compatível pra v{target} — compilando do fonte…"),
            Err(e) => println!("→ via binário falhou ({e}) — compilando do fonte…"),
        }
    } else {
        println!("→ sem binário pré-compilado pra esta plataforma/arch — compilando do fonte…");
    }

    // 2) CAMINHO CONFIÁVEL — compila do fonte (funciona em qualquer SO com toolchain).
    build_from_source()?;
    finish(&target);
    Ok(())
}

/// Tenta baixar+instalar os binários pré-compilados da v{target}. `Ok(true)` = instalou;
/// `Ok(false)` = asset ausente OU baixado não executa aqui (incompatível → cai pro fonte).
fn try_binary(target: &str, cli_asset: &str, gui_asset: &str) -> Result<bool, String> {
    let base = format!(
        "https://github.com/{}/releases/download/v{target}",
        platform::APP_REPO
    );
    let tmp = platform::state_dir().join("dl");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let (cli_name, gui_name) = platform::bin_names();
    let cli_tmp = tmp.join(&cli_name);
    let gui_tmp = tmp.join(&gui_name);

    // CLI é obrigatório; ausente (404) → sem caminho binário.
    if !fetch::download(&format!("{base}/{cli_asset}"), &cli_tmp) {
        return Ok(false);
    }
    make_executable(&cli_tmp);
    // O binário EXECUTA aqui? (protege contra glibc/arch incompatível que brickaria a troca.)
    if sh::capture(cli_tmp.to_str().unwrap_or_default(), &["--version"]).is_none() {
        return Ok(false);
    }
    // GUI é opcional (se o asset não existir, instala só o CLI).
    let has_gui = fetch::download(&format!("{base}/{gui_asset}"), &gui_tmp);
    if has_gui {
        make_executable(&gui_tmp);
    }

    let dir = platform::install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    place(&cli_tmp, &dir.join(&cli_name))?;
    if has_gui {
        let _ = place(&gui_tmp, &dir.join(&gui_name));
    }
    let _ = std::fs::remove_dir_all(&tmp);
    println!("→ instalado do binário pré-compilado v{target}.");
    Ok(true)
}

/// Compila e instala o app do fonte (CLI + GUI Slint) — INCREMENTAL. Usa um checkout PERSISTENTE
/// por repo + `cargo build --release` (reaproveita o `target/`: só o que mudou recompila; as deps
/// pesadas tipo Slint ficam cacheadas entre updates). NÃO usa `cargo install --force`, que jogava
/// fora o cache e recompilava a árvore inteira toda vez.
fn build_from_source() -> Result<(), String> {
    let cargo = platform::ensure_toolchain()?;
    platform::ensure_build_deps()?;
    let cargo_s = cargo.to_str().unwrap_or("cargo");
    let (cli_name, gui_name) = platform::bin_names();
    let dir = platform::install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // CLI (schematize) — repo schematize-cli, crate na raiz, feature `gui` (paridade com install.sh).
    build_one(cargo_s, platform::APP_REPO, &["--features", "gui"], &cli_name, &dir.join(&cli_name))?;
    // GUI (schematize-gui) — repo schematize_gui_slint (Slint), crate na raiz.
    build_one(cargo_s, platform::GUI_REPO, &[], &gui_name, &dir.join(&gui_name))?;
    Ok(())
}

/// Sincroniza o checkout persistente de `repo` (git fetch+reset se já existe; clone se não) e
/// compila `cargo build --release` (incremental) nele, copiando o binário `binname` pro `dst`.
fn build_one(cargo: &str, repo: &str, extra: &[&str], binname: &str, dst: &Path) -> Result<(), String> {
    let src = platform::build_src_dir(repo);
    sync_checkout(repo, &src)?;
    println!("→ compilando {binname} (incremental — só o que mudou recompila)…");

    let manifest = src.join("Cargo.toml");
    let manifest_s = manifest.to_string_lossy().to_string();
    let mut args: Vec<String> =
        vec!["build".into(), "--release".into(), "--manifest-path".into(), manifest_s];
    for e in extra {
        args.push((*e).to_string());
    }
    let argsref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    sh::run_inherit(cargo, &argsref)?;

    // Copia (NÃO move) o binário — mantém o artefato no target pra o próximo build incremental.
    let built = src.join("target").join("release").join(binname);
    if !built.is_file() {
        return Err(format!("build não produziu {}", built.display()));
    }
    copy_bin(&built, dst)
}

/// Deixa o checkout `src` de `repo` na versão do `main` (git fetch --depth 1 + reset --hard). Clona
/// se não existir; recloná se o checkout estiver corrompido. Shallow pra ser rápido/leve.
fn sync_checkout(repo: &str, src: &Path) -> Result<(), String> {
    let url = format!("https://github.com/{repo}");
    let src_s = src.to_string_lossy().to_string();
    if src.join(".git").is_dir() {
        let _ = sh::run_inherit("git", &["-C", &src_s, "fetch", "--depth", "1", "origin", "main"]);
        if sh::run_inherit("git", &["-C", &src_s, "reset", "--hard", "origin/main"]).is_ok() {
            return Ok(());
        }
        // checkout corrompido → reclona do zero.
        let _ = std::fs::remove_dir_all(src);
    }
    if let Some(parent) = src.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    sh::run_inherit("git", &["clone", "--depth", "1", &url, &src_s])
}

/// Copia `src`→`dst` (não move — preserva o artefato de build pro incremental) + chmod.
/// No Windows aposenta o `.exe` em uso como `.old` antes de sobrescrever.
fn copy_bin(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(windows)]
    if dst.exists() {
        let old = dst.with_extension("old");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(dst, &old);
    }
    std::fs::copy(src, dst).map_err(|e| format!("não consegui copiar pra {}: {e}", dst.display()))?;
    make_executable(dst);
    Ok(())
}

/// Pós-instalação: PATH + lançador + relatório honesto da versão que ficou instalada.
fn finish(target: &str) {
    platform::ensure_path_setup();
    platform::make_launcher();
    let got = version::installed_app_version().unwrap_or_else(|| target.to_string());
    println!("✓ schematize v{got} instalado em {}.", platform::install_dir().display());
    if platform::os() != platform::Os::Windows {
        println!("  Se o comando `schematize` não for achado, reabra o terminal (PATH) ou rode: source ~/.bashrc");
    } else {
        println!("  Reabra o terminal pra o PATH pegar o `schematize`.");
    }
}

#[cfg(windows)]
fn make_executable(_p: &Path) {}
#[cfg(not(windows))]
fn make_executable(p: &Path) {
    let _ = sh::run_inherit("chmod", &["+x", p.to_str().unwrap_or_default()]);
}

/// Move `src`→`dst` (rename atômico no mesmo FS; fallback copy). No Windows, se o destino estiver
/// em uso, renomeia o antigo pra `.old` antes (não dá pra sobrescrever um .exe em execução).
fn place(src: &Path, dst: &Path) -> Result<(), String> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        // Destino em uso: aposenta o antigo e tenta de novo.
        let old = dst.with_extension("old");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(dst, &old);
        if std::fs::rename(src, dst).is_ok() {
            return Ok(());
        }
    }
    std::fs::copy(src, dst).map_err(|e| format!("não consegui gravar {}: {e}", dst.display()))?;
    make_executable(dst);
    Ok(())
}
