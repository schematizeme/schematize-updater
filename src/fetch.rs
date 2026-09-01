//! Rede sem dependência de crate — shell-out pro cliente HTTP nativo de cada SO.
//! O quê: `get_text` (lê uma URL como String — versão no Cargo.toml raw) e `download`
//! (grava um asset em disco). Unix usa `curl`; Windows tenta `curl.exe` (Win10+ tem) e cai
//! pra PowerShell `Invoke-WebRequest`. Onde: usado por version.rs e install.rs.

use crate::sh;
use std::path::Path;

/// Lê o corpo de `url` como texto. `None` se falhar (rede/HTTP>=400). Timeout curto.
pub fn get_text(url: &str) -> Option<String> {
    #[cfg(not(windows))]
    {
        sh::capture("curl", &["-fsSL", "-m", "20", "-H", "User-Agent: schematize-updater", url])
    }
    #[cfg(windows)]
    {
        if sh::has("curl.exe") || sh::has("curl") {
            let c = if sh::has("curl.exe") { "curl.exe" } else { "curl" };
            return sh::capture(
                c,
                &["-fsSL", "-m", "20", "-H", "User-Agent: schematize-updater", url],
            );
        }
        // Fallback: PowerShell Invoke-WebRequest.
        let ps = format!(
            "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; \
             (Invoke-WebRequest -UseBasicParsing -TimeoutSec 20 -Uri '{url}').Content"
        );
        sh::capture("powershell", &["-NoProfile", "-Command", &ps])
    }
}

/// Baixa `url` para `dest`. `true` se ok (arquivo gravado). Falha silenciosa vira `false`.
pub fn download(url: &str, dest: &Path) -> bool {
    let dest_s = match dest.to_str() {
        Some(s) => s,
        None => return false,
    };
    #[cfg(not(windows))]
    {
        sh::run_inherit("curl", &["-fSL", "-o", dest_s, url]).is_ok() && dest.is_file()
    }
    #[cfg(windows)]
    {
        if sh::has("curl.exe") || sh::has("curl") {
            let c = if sh::has("curl.exe") { "curl.exe" } else { "curl" };
            return sh::run_inherit(c, &["-fSL", "-o", dest_s, url]).is_ok() && dest.is_file();
        }
        let ps = format!(
            "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; \
             Invoke-WebRequest -UseBasicParsing -Uri '{url}' -OutFile '{dest_s}'"
        );
        sh::run_inherit("powershell", &["-NoProfile", "-Command", &ps]).is_ok() && dest.is_file()
    }
}

/// A URL responde 200 (asset existe)? Usa HEAD via curl; no Windows cai pra um GET pequeno.
/// (API disponível pra checagens; o fluxo atual do install decide pelo próprio download.)
#[allow(dead_code)]
pub fn url_ok(url: &str) -> bool {
    #[cfg(not(windows))]
    {
        sh::capture(
            "curl",
            &["-fsSL", "-I", "-o", "/dev/null", "-w", "%{http_code}", "-m", "15", url],
        )
        .map(|c| c.trim() == "200")
        .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        // Sem /dev/null portátil: tenta um HEAD via curl.exe; se não houver, assume que o
        // download vai validar (retorna true e deixa o download decidir).
        if sh::has("curl.exe") {
            return sh::capture(
                "curl.exe",
                &["-fsSL", "-I", "-o", "NUL", "-w", "%{http_code}", "-m", "15", url],
            )
            .map(|c| c.trim() == "200")
            .unwrap_or(false);
        }
        true
    }
}
