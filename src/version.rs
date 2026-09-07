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

/// **O quê:** última versão publicada de QUALQUER repo da casa (lê o `version` do
/// `Cargo.toml` no `main`, via raw — sem gastar a cota de 60/h da API).
///
/// **Onde:** [`latest_app_version`] e a checagem do Deployer.
///
/// **Por que parametrizado:** o Deployer é um app com **versão própria**. Reusar a versão do
/// schematize para decidir se ele precisa de update seria comparar duas coisas diferentes —
/// e é exatamente o defeito que esta função existe para não deixar acontecer de novo.
pub fn latest_version_of(repo: &str) -> Option<String> {
    let url = format!("https://raw.githubusercontent.com/{repo}/main/Cargo.toml");
    parse_cargo_version(&fetch::get_text(&url)?)
}

/// **O quê:** versão de um binário instalado (`<bin> --version` → último token que começa
/// com dígito). `None` se não instalado ou se não responde.
///
/// **Onde:** [`installed_app_version`] e a checagem do Deployer.
pub fn installed_version_of(bin: &std::path::Path) -> Option<String> {
    let out = sh::capture(bin.to_str()?, &["--version"])?;
    out.split_whitespace()
        .last()
        .map(|s| s.to_string())
        .filter(|s| s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
}

/// **O quê:** interpreta o que a pessoa digitou em `pin <x>`.
///
/// **Onde:** o comando `pin` do `main.rs`. Função PURA — decide, não escreve.
///
/// **Por que existe:** `pin latest` gravava a string `"latest"` no arquivo. O
/// [`target_version`] é `read_pin().or_else(latest_app_version)`, então o alvo passava a ser
/// literalmente `"latest"` — e o updater tentava baixar `releases/download/vlatest`, que não
/// existe. Silenciosamente, e só na próxima atualização.
///
/// E `latest` é **a palavra que a pessoa naturalmente digita** para desafixar: o `status`
/// mostra "seguindo latest", então `pin latest` parece a forma de voltar a isso. Havia um
/// `unpin`, mas ninguém adivinha um comando que não errou. §37.48: edge case que um leigo
/// atinge é bug do software, não erro de quem digitou.
pub fn interpretar_pin(entrada: &str) -> Result<Option<String>, String> {
    let v = entrada.trim();
    // As palavras que significam "volte a seguir a última" — todas desafixam.
    if v.is_empty() || matches!(v.to_lowercase().as_str(), "latest" | "none" | "nenhum" | "-") {
        return Ok(None);
    }
    // Tolera o `v` da tag: quem copia de um release cola `v0.57.0`.
    let limpo = v.strip_prefix('v').unwrap_or(v);
    if !limpo.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return Err(format!(
            "`{v}` não parece uma versão. Use algo como `0.57.0`, \
             ou `pin latest` para voltar a seguir a última publicada"
        ));
    }
    Ok(Some(limpo.to_string()))
}

#[cfg(test)]
mod tests_pin {
    use super::*;

    /// **O bug que esta função existe para não repetir:** `pin latest` gravava a string
    /// "latest" como se fosse número de versão, e o alvo virava `vlatest`.
    #[test]
    fn latest_desafixa_em_vez_de_virar_versao() {
        assert_eq!(interpretar_pin("latest").unwrap(), None);
        assert_eq!(interpretar_pin("LATEST").unwrap(), None);
        assert_eq!(interpretar_pin("  latest  ").unwrap(), None);
        // Os outros jeitos de dizer a mesma coisa.
        for x in ["", "none", "nenhum", "-"] {
            assert_eq!(interpretar_pin(x).unwrap(), None, "{x:?} devia desafixar");
        }
    }

    /// Versão de verdade passa — com ou sem o `v` que se copia de uma tag.
    #[test]
    fn versao_passa_com_ou_sem_o_v_da_tag() {
        assert_eq!(interpretar_pin("0.57.0").unwrap(), Some("0.57.0".into()));
        assert_eq!(interpretar_pin("v0.57.0").unwrap(), Some("0.57.0".into()), "o `v` da tag");
    }

    /// Qualquer outra coisa é RECUSADA com uma mensagem que ensina o formato — nunca gravada
    /// para explodir depois, na próxima atualização.
    #[test]
    fn lixo_e_recusado_com_mensagem_acionavel() {
        for x in ["abacaxi", "main", "HEAD", "--force"] {
            let e = interpretar_pin(x).unwrap_err();
            assert!(e.contains("0.57.0"), "a mensagem tem de dar o formato: {e}");
            assert!(e.contains("pin latest"), "e a saída: {e}");
        }
    }
}
