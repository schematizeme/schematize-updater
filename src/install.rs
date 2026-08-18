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

/// Compila e instala o app do fonte (CLI + GUI Slint) via cargo install do git do main.
fn build_from_source() -> Result<(), String> {
    let cargo = platform::ensure_toolchain()?;
    platform::ensure_build_deps()?;
    let cargo_s = cargo.to_str().unwrap_or("cargo");

    // CLI (schematize) — repo schematize-cli, crate na raiz, com a feature `gui` (paridade com install.sh).
    println!("→ compilando o CLI (schematize) do fonte…");
    sh::run_inherit(
        cargo_s,
        &[
            "install", "--git", &format!("https://github.com/{}", platform::APP_REPO),
            "--branch", "main", "--features", "gui", "--force",
        ],
    )?;

    // GUI (schematize-gui) — repo schematize_gui_slint (Slint), crate na raiz.
    println!("→ compilando a GUI (schematize-gui) do fonte…");
    sh::run_inherit(
        cargo_s,
        &[
            "install", "--git", &format!("https://github.com/{}", platform::GUI_REPO),
            "--branch", "main", "--force",
        ],
    )?;
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
