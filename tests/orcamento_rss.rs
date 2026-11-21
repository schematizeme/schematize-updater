//! Orçamento de memória (BUDGET.md): sobe o binário de verdade e falha se o pico de RSS
//! (`VmHWM` de `/proc/<pid>/status`) passar do teto da operação. std-only.
//! Onde: `cargo test` (gate local). Sem `/proc` (macOS/Windows) pula com aviso explícito.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Teto de RSS pico do updater, em KiB (BUDGET.md): medido ~3 MiB, teto 8 MiB.
const TETO_KIB: u64 = 8 * 1024;

/// O quê: roda o binário com `args`, polla `VmHWM` até ele sair e devolve o maior valor visto
/// (KiB). `None` se `/proc` não existe. Onde: os testes abaixo.
fn pico_kib(args: &[&str]) -> Option<u64> {
    if !std::path::Path::new("/proc/self/status").exists() {
        return None;
    }
    let mut filho = Command::new(env!("CARGO_BIN_EXE_schematize-updater"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("não subiu o binário");
    let pid = filho.id();
    let inicio = Instant::now();
    let mut pico = 0u64;
    loop {
        if let Ok(txt) = std::fs::read_to_string(format!("/proc/{pid}/status")) {
            let v = txt
                .lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse::<u64>().ok());
            pico = pico.max(v.unwrap_or(0));
        }
        match filho.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if inicio.elapsed() > Duration::from_secs(60) => {
                let _ = filho.kill();
                let _ = filho.wait();
                panic!("`{args:?}` passou de 60 s");
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("try_wait falhou: {e}"),
        }
    }
    Some(pico)
}

/// Confere uma operação contra o teto, ou avisa que pulou (nunca passa em silêncio).
fn confere(nome: &str, args: &[&str]) {
    match pico_kib(args) {
        Some(p) => {
            eprintln!("orçamento {nome}: pico {p} KiB (teto {TETO_KIB} KiB)");
            assert!(p <= TETO_KIB, "{nome}: RSS pico {p} KiB > teto {TETO_KIB} KiB (BUDGET.md)");
        }
        None => eprintln!("orçamento {nome}: PULADO — /proc indisponível nesta plataforma"),
    }
}

#[test]
fn rss_version_dentro_do_teto() {
    confere("version", &["version"]);
}

#[test]
fn rss_help_dentro_do_teto() {
    confere("help", &["help"]);
}

#[test]
fn rss_status_dentro_do_teto() {
    confere("status", &["status"]);
}

/// Self-check: o medidor enxerga um processo de verdade (pico > 0), senão o teste estaria cego.
#[test]
fn medidor_enxerga_pico_real() {
    if let Some(p) = pico_kib(&["status"]) {
        assert!(p > 0, "VmHWM não foi lido: o medidor está cego");
    }
}
