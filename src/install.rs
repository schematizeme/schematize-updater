//! O núcleo HÍBRIDO: instala/atualiza o app tentando primeiro o binário pré-compilado
//! (rápido, sem toolchain) e, se faltar OU não rodar aqui (glibc/arch incompatível), COMPILA
//! do fonte na máquina do usuário (rustup + deps + cargo install do git). Cross-OS.
//! O quê: `install_or_update`. Onde: chamado por main.rs (install/update).

use crate::{fetch, platform, purga, sh, version};
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
    let base = format!("https://github.com/{}/releases/download/v{target}", platform::APP_REPO);
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
    let (i_cli, i_gui) = platform::bin_names_interregno();
    limpa_interregno(&dir, &i_cli);
    limpa_interregno(&dir, &i_gui);
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
    let (i_cli, i_gui) = platform::bin_names_interregno();
    let dir = platform::install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // PURGA das cópias-FANTASMA antes de instalar. Não é zelo: é o conserto do
    // "atualizei e voltou pra uma versão antiga". O binário podia estar em quatro
    // lugares e o PATH resolvia pro errado; instalar por cima de UM deles não desfaz
    // isso. O diretório de destino não é varrido — lá a troca é por rename, então um
    // build que falhe no meio não deixa a máquina sem app. Ver `purga.rs`.
    let manter: Vec<std::path::PathBuf> = std::env::current_exe().ok().into_iter().collect();
    let (removidos, resistiram) = purga::remove_copias_fantasma(&dir, &manter);
    for p in &removidos {
        println!("→ removida instalação anterior: {}", p.display());
    }
    for p in &resistiram {
        println!(
            "aviso: não consegui remover {} (permissão?) — se o `schematize` continuar",
            p.display()
        );
        println!("       abrindo uma versão velha, apague este arquivo à mão.");
    }

    // SQLite: se a distro tem a lib de desenvolvimento, LINKA a dela em vez de compilar
    // ~250 mil linhas de C a cada build limpo. Mesma escolha que o install.sh faz.
    let sqlite_do_sistema = platform::tem_sqlite_do_sistema();
    let feats: &[&str] = if sqlite_do_sistema {
        println!("→ usando a libsqlite3 da distro (não compila o SQLite embutido).");
        &["--no-default-features", "--features", "sqlite-do-sistema"]
    } else {
        &[]
    };

    // CLI (schematize) — repo schematize-cli, crate na raiz. A GUI egui não existe mais
    // neste crate: a única janela é a Slint (repo próprio, build abaixo).
    build_one(cargo_s, platform::APP_REPO, feats, None, &cli_name, &dir.join(&cli_name))?;
    limpa_interregno(&dir, &i_cli);
    // GUI (schematize-gui) — repo schematize_gui_slint (Slint), crate na raiz. A GUI depende do crate
    // `schematize` como git-dep (branch=main); o `Cargo.lock` commitado FIXA um commit, e o build só
    // recompila — sem avançar o dep. Resultado: a GUI embutia uma versão VELHA (`app_version()` =
    // CARGO_PKG_VERSION do schematize no commit pinado) mesmo com o CLI já novo. `cargo update -p
    // schematize` avança o git-dep pro HEAD do main ANTES de compilar → a versão embutida bate.
    build_one(
        cargo_s,
        platform::GUI_REPO,
        feats,
        Some("schematize"),
        &gui_name,
        &dir.join(&gui_name),
    )?;
    limpa_interregno(&dir, &i_gui);
    // Encerra qualquer GUI ANTIGA ainda aberta: só fechar a janela não bastava (o processo velho
    // seguia vivo e o relaunch reusava a versão anterior). Mata pra o próximo open pegar a nova.
    // Mata a GUI antiga com qualquer um dos nomes — o processo em execução pode ter
    // subido pelo lançador do interregno, e um relaunch reusaria a versão velha.
    kill_stale_gui(&gui_name);
    kill_stale_gui(&i_gui);

    // GUI do updater (janela amigável do próprio gestor) — OPCIONAL: se o build falhar, o update NÃO
    // falha (o updater headless e o app já estão instalados; é só chrome). Não depende do crate
    // `schematize`, então sem `refresh_dep`.
    let ugui = platform::updater_gui_bin();
    if let Err(e) =
        build_one(cargo_s, platform::UPDATER_GUI_REPO, &[], None, &ugui, &dir.join(&ugui))
    {
        println!("aviso: build da GUI do updater falhou (opcional, seguindo): {e}");
    }

    // O PRÓPRIO updater, POR ÚLTIMO.
    //
    // Ele era o único componente que ninguém atualizava: reconstruía o CLI, a GUI e a
    // GUI do updater, e ficava parado na última tag publicada. Resultado prático: uma
    // correção NELE (por exemplo, a de trocar binário em execução) não chegava em
    // máquina nenhuma pelo caminho normal — o gestor de atualizações era o único que
    // não recebia atualização.
    //
    // Trocar o binário de um processo EM EXECUÇÃO — este aqui — funciona porque
    // `substitui_binario` renomeia por cima em vez de escrever no arquivo: este
    // processo segue no inode antigo até terminar, e a próxima execução já é a nova.
    //
    // Por último de propósito: se falhar, o app já está atualizado (que é o que o
    // usuário pediu); e por isso também é `aviso`, não erro.
    let eu = platform::updater_bin_name();
    if let Err(e) = build_one(cargo_s, platform::UPDATER_REPO, &[], None, &eu, &dir.join(&eu)) {
        println!("aviso: não consegui me atualizar ({e}) — o app está atualizado; tento de novo no próximo update.");
    }

    // Só agora, com os binários no lugar: joga fora os `target/` por-repo da versão
    // anterior (ver `limpa_targets_antigos`).
    limpa_targets_antigos();
    Ok(())
}

/// Remove os `target/` por-repo que existiam ANTES do target compartilhado.
///
/// Quem já tinha o app instalado carrega um `target/` dentro de cada checkout — dezenas
/// de GB de artefato que nenhum build volta a ler depois desta versão. Deixar isso pro
/// usuário descobrir e apagar à mão é o oposto do piso da casa: o script mudou o layout,
/// o script limpa. Nunca antes do build: se ele falhar, o cache antigo continua lá.
///
/// Só toca em caminhos que ESTE programa criou (`build_src_dir(repo)/target`) — nada de
/// varrer diretório por padrão. Falhar aqui não é erro: é só disco que sobrou.
fn limpa_targets_antigos() {
    let repos = [platform::APP_REPO, platform::GUI_REPO, platform::UPDATER_GUI_REPO];
    let mut liberado: u64 = 0;
    for repo in repos {
        let antigo = platform::build_src_dir(repo).join("target");
        if !antigo.is_dir() {
            continue;
        }
        let tamanho = tamanho_de(&antigo);
        if std::fs::remove_dir_all(&antigo).is_ok() {
            liberado += tamanho;
        }
    }
    if liberado > 0 {
        println!(
            "→ liberados {} de `target/` antigo (agora há um só, compartilhado).",
            legivel(liberado)
        );
    }
}

/// Soma o tamanho dos arquivos de uma árvore. Best-effort: o que não der pra ler conta 0
/// (é só pra imprimir "liberados X GB", não uma contabilidade).
fn tamanho_de(dir: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    for e in rd.flatten() {
        match e.metadata() {
            Ok(m) if m.is_dir() => total += tamanho_de(&e.path()),
            Ok(m) => total += m.len(),
            Err(_) => {}
        }
    }
    total
}

/// Bytes em unidade legível (uma casa decimal a partir de MB).
fn legivel(bytes: u64) -> String {
    const GB: u64 = 1024 * 1024 * 1024;
    const MB: u64 = 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Encerra processos da GUI já rodando (por NOME exato do binário) após trocar o executável, pra que
/// a próxima abertura use a versão recém-instalada — o usuário reclamava de "atualizei mas abre a
/// versão antiga" porque fechar a janela não matava o processo. Best-effort e cross-OS; nunca falha
/// o update. NÃO casa o próprio updater (`schematize-updater`) nem o CLI — só o binário exato da GUI.
fn kill_stale_gui(gui_name: &str) {
    #[cfg(windows)]
    {
        let _ = sh::capture("taskkill", &["/F", "/IM", &format!("{gui_name}.exe")]);
    }
    #[cfg(not(windows))]
    {
        // -x = nome exato do processo (não casa paths/cmdline tipo `schematize_gui_slint`).
        let _ = sh::capture("pkill", &["-x", gui_name]);
    }
}

/// Sincroniza o checkout persistente de `repo` (git fetch+reset se já existe; clone se não) e
/// compila `cargo build --release` (incremental) nele, copiando o binário `binname` pro `dst`.
fn build_one(
    cargo: &str,
    repo: &str,
    extra: &[&str],
    refresh_dep: Option<&str>,
    binname: &str,
    dst: &Path,
) -> Result<(), String> {
    let src = platform::build_src_dir(repo);
    sync_checkout(repo, &src)?;

    let manifest = src.join("Cargo.toml");
    let manifest_s = manifest.to_string_lossy().to_string();

    // Avança um git-dep pro HEAD do branch ANTES de compilar (o `Cargo.lock` commitado o pina num
    // commit antigo, e `git reset --hard` restaura esse lock a cada update). Best-effort: se a rede
    // cair, segue com o lock existente (offline ainda compila). Sem isto, a versão embutida trava.
    if let Some(dep) = refresh_dep {
        println!("→ atualizando dep `{dep}` pro HEAD do main (evita versão embutida velha)…");
        let _ = sh::run_inherit(cargo, &["update", "--manifest-path", &manifest_s, "-p", dep]);
    }

    println!("→ compilando {binname} (incremental — só o que mudou recompila)…");
    let mut args: Vec<String> =
        vec!["build".into(), "--release".into(), "--manifest-path".into(), manifest_s];
    for e in extra {
        args.push((*e).to_string());
    }
    let argsref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    // `target/` COMPARTILHADO: as ~226 dependências comuns aos três repos compilam UMA
    // vez, não três (ver `platform::shared_target_dir`).
    let tgt = platform::shared_target_dir();
    std::fs::create_dir_all(&tgt).map_err(|e| e.to_string())?;
    let tgt_s = tgt.to_string_lossy().to_string();
    sh::run_inherit_env(cargo, &argsref, &[("CARGO_TARGET_DIR", &tgt_s)])?;

    // Copia (NÃO move) o binário — mantém o artefato no target pra o próximo build incremental.
    let built = tgt.join("release").join(binname);
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

/// Apaga um binário do INTERREGNO (nome Overflow) do diretório de instalação.
///
/// Aquele nome saiu de circulação: não há instalação nova pra escrever por cima, e
/// deixá-lo no PATH cria um binário órfão que ninguém atualiza — o fantasma clássico
/// que faz o app "voltar" pra uma versão velha quando o PATH resolve pra ele primeiro.
///
/// Só o diretório de instalação: as outras cópias são da `purga`, que já os conhece.
fn limpa_interregno(dir: &Path, nome: &str) {
    let p = dir.join(nome);
    if p.exists() && std::fs::remove_file(&p).is_ok() {
        println!("→ removido binário do interregno: {}", p.display());
    }
}

/// Copia `src`→`dst` (não move — preserva o artefato de build pro incremental) + chmod.
/// No Windows aposenta o `.exe` em uso como `.old` antes de sobrescrever.
fn copy_bin(src: &Path, dst: &Path) -> Result<(), String> {
    substitui_binario(src, dst)
}

/// Troca um binário que pode estar EM EXECUÇÃO, sem matar ninguém.
///
/// O erro que isto conserta: `Text file busy` (ETXTBSY). No Linux não dá pra abrir
/// pra escrita um arquivo que está sendo executado — e o `schematize` está: o agente
/// do autostart roda o tempo todo. Então `fs::copy` direto no destino falhava no meio
/// do update, depois de já ter compilado tudo.
///
/// A saída é a do Unix: gravar um arquivo NOVO ao lado (mesmo diretório, pra o rename
/// ser atômico e no mesmo sistema de arquivos) e `rename(2)` por cima. Renomear sobre
/// um executável em uso é permitido: quem já está rodando continua no inode antigo, e
/// a próxima execução pega o novo. Nada de pedir pro usuário fechar o app.
///
/// No Windows não existe esse truque (o arquivo fica travado), então lá seguimos
/// aposentando o antigo como `.old` antes de gravar.
fn substitui_binario(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(windows)]
    if dst.exists() {
        let old = dst.with_extension("old");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(dst, &old);
    }
    // Temporário NO MESMO diretório do destino: rename entre sistemas de arquivos
    // diferentes falha (EXDEV), e é justamente o rename que precisa funcionar.
    let tmp = dst.with_file_name(format!(
        "{}.novo",
        dst.file_name().and_then(|s| s.to_str()).unwrap_or("schematize")
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::copy(src, &tmp).map_err(|e| format!("não consegui gravar {}: {e}", tmp.display()))?;
    make_executable(&tmp);
    if let Err(e) = std::fs::rename(&tmp, dst) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("não consegui substituir {}: {e}", dst.display()));
    }
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
    // Caminho feliz: o download já está no mesmo sistema de arquivos → um rename resolve.
    if std::fs::rename(src, dst).is_ok() {
        make_executable(dst);
        return Ok(());
    }
    // Senão (EXDEV, ou destino travado no Windows), cai no mesmo mecanismo do build:
    // grava ao lado do destino e renomeia por cima. NUNCA `fs::copy` direto no destino —
    // é o que dava `Text file busy` com o agente rodando.
    substitui_binario(src, dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REGRESSÃO: trocar um binário que está EM EXECUÇÃO.
    ///
    /// Foi o bug que escapou duas vezes (Linux Mint e openSUSE): o update compilava
    /// tudo e morria no último passo com `Text file busy` (ETXTBSY), porque o
    /// `schematize` está sempre rodando — o agente do autostart. Escrever no destino
    /// falha; renomear por cima, não. Este teste monta exatamente esse cenário.
    #[cfg(unix)]
    #[test]
    fn substitui_binario_em_execucao() {
        use std::process::{Command, Stdio};

        let base = std::env::temp_dir().join(format!("upd-busy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let alvo = base.join("emuso");

        // Um executável de verdade no lugar do destino, e um processo RODANDO ele.
        std::fs::copy("/bin/sleep", &alvo).unwrap();
        make_executable(&alvo);
        let mut filho = Command::new(&alvo)
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("não consegui rodar o binário de teste");

        // O jeito ANTIGO (escrever no destino) tem de falhar — é o bug.
        let erro = std::fs::copy("/bin/true", &alvo).unwrap_err();
        assert_eq!(
            erro.raw_os_error(),
            Some(26),
            "esperava ETXTBSY (26) ao escrever num binário em execução, veio {erro:?}"
        );

        // O jeito NOVO (gravar ao lado + renomear) tem de passar, com o processo vivo.
        substitui_binario(Path::new("/bin/true"), &alvo).expect("substituição devia funcionar");
        assert!(filho.try_wait().unwrap().is_none(), "o processo antigo tem de seguir vivo");

        // E o destino agora é o binário NOVO (o `true` sai com 0 na hora; o `sleep` não).
        let st = Command::new(&alvo).status().unwrap();
        assert!(st.success());

        let _ = filho.kill();
        let _ = filho.wait();
        let _ = std::fs::remove_dir_all(&base);
    }

    /// O temporário fica NO MESMO diretório do destino: rename entre sistemas de
    /// arquivos diferentes falha com EXDEV, e é o rename que precisa funcionar.
    #[cfg(unix)]
    #[test]
    fn temporario_fica_no_diretorio_do_destino() {
        let base = std::env::temp_dir().join(format!("upd-tmpdir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let alvo = base.join("bin");
        substitui_binario(Path::new("/bin/true"), &alvo).unwrap();
        assert!(alvo.is_file());
        // não deixou lixo pra trás
        assert!(!base.join("bin.novo").exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
