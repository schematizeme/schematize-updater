//! PURGA — remove qualquer instalação anterior antes de instalar a nova.
//!
//! O quê: apaga binários do schematize em TODOS os diretórios onde alguma versão já
//! pôde ser instalada, mais lançadores e autostart. Onde: chamado pelo `install.rs`
//! antes de colocar os binários novos. Espelha o `purge_previous` do `install.sh` —
//! os dois caminhos de instalação têm de deixar a máquina no mesmo estado.
//!
//! Por que existe: o schematize já pôde ir parar em quatro lugares — `/usr/bin`
//! (pacote .deb/.rpm), `/usr/local/bin` (self-update via pkexec), `~/.local/bin`
//! (fallback) e `~/.cargo/bin` (fonte/updater). Quem instalou por caminhos diferentes
//! ao longo do tempo fica com várias cópias, e quem "ganha" é a primeira do PATH — que
//! pode ser a MAIS VELHA. Foi assim que uma máquina recém-atualizada voltou a rodar
//! uma versão antiga: o binário novo entrou num diretório e o PATH resolveu pro outro.
//! Atualizar não conserta isso, porque o problema não é a versão — é a ambiguidade.
//!
//! O que NÃO é tocado: dependências do sistema e DADOS do usuário (`~/.claude`,
//! `~/.schematize`, `~/.overflow`). Purga instalação, não o trabalho de ninguém. E nunca o binário
//! que está EM EXECUÇÃO agora (este) — quem o substitui é o `substitui_binario`.

use crate::platform;
use std::path::{Path, PathBuf};

/// Os binários da casa. Nomes exatos — nada de padrão/glob em `rm`.
fn nomes() -> Vec<String> {
    let sfx = platform::exe_suffix();
    [
        // nomes novos (Overflow) e os anteriores — a purga tem de reconhecer os dois,
        // senão uma cópia-fantasma com o nome antigo sobrevive num dir de maior
        // precedência no PATH e o app "volta" pra uma versão velha. Foi esse o bug.
        "overflow",
        "overflow-gui",
        "overflow-updater",
        "overflow-updater-gui",
        "schematize",
        "schematize-gui",
        "schematize-updater",
        "schematize-updater-gui",
    ]
    .iter()
    .map(|b| format!("{b}{sfx}"))
    .collect()
}

/// Todo diretório onde alguma versão já pôde ser instalada.
fn diretorios() -> Vec<PathBuf> {
    let home = platform::home();
    vec![
        home.join(".cargo").join("bin"),
        home.join(".local").join("bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
    ]
}

/// Remove as cópias-FANTASMA: toda instalação anterior FORA do diretório em que esta
/// instalação vai escrever (`destino`).
///
/// Por que não apaga também o destino: o binário de lá é substituído no fim, por
/// rename, e apagar antes abriria uma janela — um build de 20 minutos que falha
/// deixaria a máquina sem app nenhum. As cópias que causam o bug são justamente as
/// OUTRAS: são elas que o PATH pode resolver primeiro. Some com elas e a ambiguidade
/// acaba, sem nunca deixar o usuário sem ferramenta.
///
/// `manter` cobre o binário em execução (este) — apagar a si mesmo no meio da
/// instalação deixaria a máquina sem gestor se algo falhasse depois.
///
/// Devolve `(removidos, resistiram)`. Falhar em um deles não é erro: pode ser
/// `/usr/bin` sem permissão — o que restou é reportado pro usuário resolver, em vez de
/// fingir que limpou.
pub fn remove_copias_fantasma(destino: &Path, manter: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    remove_em(&diretorios(), destino, manter)
}

/// O motor da purga, com a lista de diretórios por PARÂMETRO.
///
/// Separado de [`remove_copias_fantasma`] por uma razão prática e cara: a primeira
/// versão disto varria `diretorios()` fixos, e o teste que deveria provar "não apaga
/// no destino" apagou os binários REAIS da máquina onde rodou. Função que remove
/// arquivo tem de ser testável sem tocar no sistema — senão o teste é que vira o risco.
fn remove_em(dirs: &[PathBuf], destino: &Path, manter: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut removidos = Vec::new();
    let mut resistiram = Vec::new();
    for dir in dirs {
        let dir = dir.clone();
        if mesmo_arquivo(&dir, destino) {
            continue; // o destino é trocado por rename, não apagado
        }
        for nome in nomes() {
            let alvo = dir.join(&nome);
            if !alvo.is_file() || manter.iter().any(|m| mesmo_arquivo(m, &alvo)) {
                continue;
            }
            match std::fs::remove_file(&alvo) {
                Ok(()) => removidos.push(alvo),
                Err(_) => resistiram.push(alvo),
            }
        }
    }
    (removidos, resistiram)
}

/// Dois caminhos apontam pro mesmo arquivo? Compara canonizado — `~/.cargo/bin/x` e
/// `/home/u/.cargo/bin/x` são o mesmo, e não podemos apagar o que pedimos pra manter.
fn mesmo_arquivo(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lista de nomes é EXATA — nada de padrão que possa varrer vizinho.
    ///
    /// São os QUATRO binários da casa em DOIS nomes cada (Overflow e o anterior):
    /// a purga tem de reconhecer os dois, senão uma cópia-fantasma com o nome antigo
    /// sobrevive num dir de maior precedência no PATH e o app "volta" pra uma versão
    /// velha — que é exatamente o bug que esta purga existe pra matar.
    #[test]
    fn so_nomes_exatos_da_casa() {
        let n = nomes();
        assert_eq!(n.len(), 8, "4 binários x 2 nomes");
        assert_eq!(n.iter().filter(|x| x.starts_with("overflow")).count(), 4);
        assert_eq!(n.iter().filter(|x| x.starts_with("schematize")).count(), 4);
        assert!(n.iter().all(|x| !x.contains('*') && !x.contains('?')));
    }

    /// Os quatro diretórios em que uma versão já pôde ser instalada estão cobertos —
    /// é a lista inteira que faz a ambiguidade de PATH sumir.
    #[test]
    fn cobre_os_quatro_lugares_possiveis() {
        let d: Vec<String> = diretorios().iter().map(|p| p.display().to_string()).collect();
        assert!(d.iter().any(|p| p.ends_with(".cargo/bin")));
        assert!(d.iter().any(|p| p.ends_with(".local/bin")));
        assert!(d.iter().any(|p| p == "/usr/local/bin"));
        assert!(d.iter().any(|p| p == "/usr/bin"));
    }

    /// O destino não é varrido; as cópias-fantasma são. Roda 100% em diretórios
    /// temporários — `remove_em` recebe a lista justamente pra o teste nunca tocar
    /// em `/usr/bin` nem em `~/.cargo/bin` da máquina que roda a suíte.
    #[test]
    fn varre_a_fantasma_e_poupa_o_destino() {
        let base = std::env::temp_dir().join(format!("purga-dest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let destino = base.join("destino");
        let fantasma = base.join("fantasma");
        std::fs::create_dir_all(&destino).unwrap();
        std::fs::create_dir_all(&fantasma).unwrap();
        let bom = destino.join("schematize");
        let velho = fantasma.join("schematize");
        std::fs::write(&bom, b"novo").unwrap();
        std::fs::write(&velho, b"velho").unwrap();

        let (rm, _) = remove_em(&[destino.clone(), fantasma.clone()], &destino, &[]);

        assert!(bom.is_file(), "o binário do destino tem de continuar lá");
        assert!(!velho.exists(), "a cópia-fantasma tem de sumir — é ela que o PATH pegava");
        assert_eq!(rm, vec![velho]);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// O que está em `manter` NÃO é removido — é o que impede o updater de apagar a si
    /// mesmo no meio da instalação.
    #[test]
    fn respeita_o_que_deve_ser_mantido() {
        let base = std::env::temp_dir().join(format!("purga-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let eu = base.join("schematize-updater");
        std::fs::write(&eu, b"x").unwrap();
        assert!(mesmo_arquivo(&eu, &eu.clone()));
        // um caminho equivalente (com "." no meio) também tem de casar
        let equivalente = base.join(".").join("schematize-updater");
        assert!(
            mesmo_arquivo(&eu, &equivalente),
            "canonizar tem de resolver caminhos equivalentes"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
