//! Execução de comandos externos — o updater orquestra ferramentas do sistema
//! (curl/powershell, rustup, cargo, git, gerenciador de pacotes) em vez de embutir libs.
//! O quê: `run_inherit` (mostra saída ao vivo — build/rustup/sudo pedem no terminal) e
//! `capture` (lê stdout, silencioso — checagens de versão/rede). Onde: usado por todos os módulos.

use std::path::Path;
use std::process::{Command, Stdio};

/// Roda `cmd args` herdando stdio (o usuário VÊ o progresso e sudo/rustup podem pedir senha).
/// `Ok(())` se sair 0; `Err(msg)` com o código caso contrário ou se nem lançou.
pub fn run_inherit(cmd: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(cmd)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("não consegui executar `{cmd}`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{cmd}` falhou ({status})"))
    }
}

/// Roda `cmd args` herdando stdio, com variáveis de ambiente EXTRAS.
/// Usado pra passar `CARGO_TARGET_DIR` aos builds sem depender do ambiente do
/// processo (o updater roda num terminal que pode ter qualquer coisa exportada).
pub fn run_inherit_env(cmd: &str, args: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
    let mut c = Command::new(cmd);
    c.args(args).stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    for (k, v) in env {
        c.env(k, v);
    }
    let status = c.status().map_err(|e| format!("não consegui executar `{cmd}`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{cmd}` falhou ({status})"))
    }
}

/// Roda `cmd args` num diretório específico, herdando stdio (ex.: `cargo install` no clone).
#[allow(dead_code)]
pub fn run_inherit_in(dir: &Path, cmd: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("não consegui executar `{cmd}` em {}: {e}", dir.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{cmd}` falhou ({status}) em {}", dir.display()))
    }
}

/// Captura o stdout de `cmd args` (silencioso). `None` se falhar/sair !=0. Trim aplicado.
pub fn capture(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// `cmd` existe no PATH? (multiplataforma: `command -v` no Unix, `where` no Windows.)
pub fn has(cmd: &str) -> bool {
    #[cfg(windows)]
    {
        capture("where", &[cmd]).map(|s| !s.is_empty()).unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        capture("sh", &["-c", &format!("command -v {cmd}")]).map(|s| !s.is_empty()).unwrap_or(false)
    }
}
