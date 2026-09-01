//! Gestão de VERSÃO do app (independente da versão do updater).
//! O quê: resolve a última versão publicada (lê o `version` do Cargo.toml no `main` via raw
//! GitHub — sem API 60/h), lê a versão instalada (`schematize --version`), e um PIN opcional
//! (fixar uma versão). Onde: consumido por install.rs (decidir se atualiza) e main.rs (status).

use crate::{fetch, platform, sh};
use std::path::PathBuf;

/// URL raw do Cargo.toml do app no branch main (fonte de verdade do "última versão").
fn cargo_toml_url() -> String {
    format!("https://raw.githubusercontent.com/{}/main/Cargo.toml", platform::APP_REPO)
}

/// Última versão publicada do app (do `version = "x.y.z"` do Cargo.toml no main). `None` se rede falhar.
pub fn latest_app_version() -> Option<String> {
    let body = fetch::get_text(&cargo_toml_url())?;
    parse_cargo_version(&body)
}

/// Extrai o 1º `version = "x"` de um Cargo.toml (o do `[package]`, que vem no topo).
pub fn parse_cargo_version(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("version") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let v = rest.trim().trim_matches('"').trim();
                if !v.is_empty() && v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Versão do app INSTALADO (`<bin> --version` → "<nome> X.Y.Z" → "X.Y.Z"). `None` se não instalado.
/// Pega o ÚLTIMO token e exige que comece com dígito — por isso sobreviveu à troca de nome.
pub fn installed_app_version() -> Option<String> {
    let bin = platform::app_bin(false);
    let out = sh::capture(bin.to_str().unwrap_or("schematize"), &["--version"])?;
    // formato "overflow 0.45.0" (ou "schematize 0.44.1" numa instalação anterior)
    out.split_whitespace()
        .last()
        .map(|s| s.to_string())
        .filter(|s| s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
}

/// Arquivo de PIN (versão fixada). Vazio/ausente = sempre "latest".
fn pin_file() -> PathBuf {
    platform::state_dir().join("pin")
}

/// Lê a versão fixada (pin), se houver.
pub fn read_pin() -> Option<String> {
    let s = std::fs::read_to_string(pin_file()).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Fixa uma versão (pin). Passe `None` pra desafixar (volta a seguir latest).
pub fn write_pin(v: Option<&str>) -> Result<(), String> {
    let _ = std::fs::create_dir_all(platform::state_dir());
    match v {
        Some(v) => std::fs::write(pin_file(), v).map_err(|e| e.to_string()),
        None => {
            let _ = std::fs::remove_file(pin_file());
            Ok(())
        }
    }
}

/// A versão-ALVO: o pin se houver, senão a última publicada.
pub fn target_version() -> Option<String> {
    read_pin().or_else(latest_app_version)
}

/// `a < b` em semver simples (x.y.z, numérico por segmento; sufixos ignorados após o número).
/// (Exposto pra comparações/futuro anti-downgrade; o install atual decide por igualdade exata.)
#[allow(dead_code)]
pub fn semver_lt(a: &str, b: &str) -> bool {
    let seg = |s: &str| -> Vec<u64> {
        s.split(['.', '-', '+'])
            .take_while(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty())
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    };
    let (va, vb) = (seg(a), seg(b));
    for i in 0..va.len().max(vb.len()) {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        if x != y {
            return x < y;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_do_cargo() {
        let toml = "[package]\nname = \"schematize\"\nversion = \"0.34.1\"\nedition = \"2021\"\n";
        assert_eq!(parse_cargo_version(toml).as_deref(), Some("0.34.1"));
    }

    #[test]
    fn semver_compara() {
        assert!(semver_lt("0.33.2", "0.34.0"));
        assert!(semver_lt("0.34.0", "0.34.1"));
        assert!(!semver_lt("0.34.1", "0.34.1"));
        assert!(!semver_lt("1.0.0", "0.34.1"));
    }
}
