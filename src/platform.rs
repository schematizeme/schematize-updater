//! Conhecimento POR SISTEMA OPERACIONAL — o que muda entre Linux/macOS/Windows.
//! O quê: detecção de SO/arch/família, diretórios de instalação, nomes de asset pré-compilado,
//! bootstrap de toolchain (rustup) e libs de build, ajuste de PATH e criação de lançador.
//! Onde: consumido por install.rs (orquestra) e main.rs (status). Tudo best-effort com mensagem
//! honesta — o updater NUNCA engole erro (o motivo do updater existir é ser confiável).

use crate::sh;
use std::path::PathBuf;

/// Org/repos do app (o updater é desacoplado, mas sabe de ONDE puxar o app).
pub const APP_REPO: &str = "schematizeme/schematize-cli";
pub const GUI_REPO: &str = "schematizeme/schematize_gui_slint";
/// Janela (Slint) do próprio gestor de atualizações — OPCIONAL (chrome); build best-effort.
/// O repo DESTE programa. O updater precisa saber se reconstruir: ele é o único
/// componente que ninguém mais atualiza — se ficar parado na última tag publicada,
/// qualquer correção nele (inclusive nas de atualizar) nunca chega em máquina nenhuma.
pub const UPDATER_REPO: &str = "schematizeme/schematize-updater";
pub const UPDATER_GUI_REPO: &str = "schematizeme/schematize-updater-gui";

/// Sistema operacional em execução. (Variantes "não construídas" no target atual são normais —
/// o `os()` só constrói a do SO compilado; as outras existem pro código cross-OS.)
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Os {
    Linux,
    Mac,
    Windows,
}

pub fn os() -> Os {
    #[cfg(target_os = "windows")]
    {
        Os::Windows
    }
    #[cfg(target_os = "macos")]
    {
        Os::Mac
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Os::Linux
    }
}

/// Família de distro Linux (pra escolher o gerenciador de pacotes). Irrelevante fora do Linux.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinuxFam {
    Debian,
    Rpm,
    Arch,
    Unknown,
}

/// Lê /etc/os-release e classifica a família. Fora do Linux devolve `Unknown`.
pub fn linux_family() -> LinuxFam {
    let txt = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let low = txt.to_lowercase();
    let has = |k: &str| low.contains(k);
    if has("debian") || has("ubuntu") || has("mint") || has("pop") || has("elementary") {
        LinuxFam::Debian
    } else if has("suse") || has("opensuse") || has("sles") || has("fedora") || has("rhel")
        || has("centos") || has("rocky") || has("alma")
    {
        LinuxFam::Rpm
    } else if has("arch") || has("manjaro") || has("endeavour") {
        LinuxFam::Arch
    } else {
        LinuxFam::Unknown
    }
}

/// Diretório home do usuário (multiplataforma).
pub fn home() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Onde os binários do app ficam (o mesmo lugar que `cargo install` usa): `~/.cargo/bin`.
pub fn install_dir() -> PathBuf {
    home().join(".cargo").join("bin")
}

/// Config/estado do updater — `~/.schematize/updater/` (pin de versão, log).
pub fn state_dir() -> PathBuf {
    home().join(".schematize").join("updater")
}

/// Dir de BUILD PERSISTENTE de um repo — `~/.schematize/updater/build/<repo>`. Mantido entre
/// updates de propósito: o checkout + o `target/` ficam cacheados, então o próximo update é
/// INCREMENTAL (só o que mudou recompila; as deps pesadas tipo Slint não recompilam). Troca
/// tempo de build (minutos→segundos) por disco — o que o usuário quer.
pub fn build_src_dir(repo: &str) -> PathBuf {
    let name = repo.rsplit('/').next().unwrap_or(repo);
    state_dir().join("build").join(name)
}

/// `target/` COMPARTILHADO pelos repos que compilamos (CLI, GUI Slint, GUI do updater).
///
/// 226 das dependências são as MESMAS nos três. Com um `target/` por checkout elas
/// compilavam três vezes e ocupavam três vezes o disco; com um só, compilam uma. É o
/// mesmo diretório e a mesma ideia do `install.sh` da casa — os dois caminhos de
/// atualização (updater e install.sh) têm de dar no mesmo resultado, senão "atualizei
/// pelo gestor" e "atualizei pelo instalador" viram experiências diferentes.
///
/// Exige perfil de release IDÊNTICO nos repos (cargo só reaproveita artefato quando
/// o perfil bate) — está documentado no `[profile.release]` de cada um.
pub fn shared_target_dir() -> PathBuf {
    state_dir().join("build").join("target")
}

/// A libsqlite3 de desenvolvimento existe nesta máquina?
///
/// Se existir, o CLI linka a da distro em vez de compilar o SQLite embutido (~250 mil
/// linhas de C a cada build limpo). Crate de Rust não dá pra reusar da distro (Rust não
/// tem ABI estável); biblioteca C, dá — e esta é a que pesa no build.
pub fn tem_sqlite_do_sistema() -> bool {
    sh::capture("pkg-config", &["--exists", "sqlite3"]).is_some()
}

/// Sufixo de executável (".exe" no Windows).
pub fn exe_suffix() -> &'static str {
    if os() == Os::Windows {
        ".exe"
    } else {
        ""
    }
}

/// Nomes dos binários instalados: (cli, gui) — com `.exe` no Windows.
pub fn bin_names() -> (String, String) {
    let s = exe_suffix();
    (format!("schematize{s}"), format!("schematize-gui{s}"))
}

/// Nome do binário da GUI do updater (com sufixo `.exe` no Windows).
pub fn updater_gui_bin() -> String {
    format!("schematize-updater-gui{}", exe_suffix())
}

/// Nome do binário DESTE programa (com `.exe` no Windows).
pub fn updater_bin_name() -> String {
    format!("schematize-updater{}", exe_suffix())
}

/// Nomes dos ASSETS pré-compilados no release (batem com selfupdate.rs do app). `None` se a
/// arquitetura/SO não tem binário publicado (aí o híbrido cai direto pro source).
pub fn asset_names() -> Option<(&'static str, &'static str)> {
    let arch = std::env::consts::ARCH; // "x86_64", "aarch64", ...
    match (os(), arch) {
        (Os::Linux, "x86_64") => Some(("schematize-linux-x86_64", "schematize-gui-linux-x86_64")),
        (Os::Mac, "aarch64") => Some(("schematize-macos-arm64", "schematize-gui-macos-arm64")),
        (Os::Mac, "x86_64") => Some(("schematize-macos-x86_64", "schematize-gui-macos-x86_64")),
        (Os::Windows, "x86_64") => {
            Some(("schematize-windows-x86_64.exe", "schematize-gui-windows-x86_64.exe"))
        }
        _ => None,
    }
}

/// `sudo` só faz sentido no Unix e quando NÃO se é root. Devolve o prefixo ("sudo" ou "").
#[cfg(not(windows))]
fn sudo() -> &'static str {
    let is_root = sh::capture("id", &["-u"]).map(|u| u == "0").unwrap_or(false);
    if is_root || !sh::has("sudo") {
        ""
    } else {
        "sudo"
    }
}

/// Garante o Rust/cargo (bootstrap do toolchain). Instala rustup se `cargo` faltar.
/// Devolve o caminho ABSOLUTO do cargo (pós-install o PATH do processo não tem ~/.cargo/bin).
pub fn ensure_toolchain() -> Result<PathBuf, String> {
    let cargo_abs = install_dir().join(format!("cargo{}", exe_suffix()));
    if sh::has("cargo") {
        return Ok(PathBuf::from("cargo"));
    }
    if cargo_abs.is_file() {
        return Ok(cargo_abs);
    }
    println!("→ instalando Rust (rustup, perfil mínimo)…");
    match os() {
        Os::Windows => {
            // Baixa o rustup-init.exe e roda sem interação.
            let tmp = state_dir().join("rustup-init.exe");
            let _ = std::fs::create_dir_all(state_dir());
            if !crate::fetch::download("https://win.rustup.rs/x86_64", &tmp) {
                return Err(
                    "não consegui baixar o rustup-init.exe. Instale o Rust manualmente em https://rustup.rs".into(),
                );
            }
            sh::run_inherit(tmp.to_str().unwrap_or_default(), &["-y", "--profile", "minimal"])?;
        }
        _ => {
            // Unix: curl https://sh.rustup.rs | sh -s -- -y.
            sh::run_inherit(
                "sh",
                &["-c", "curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal"],
            )?;
        }
    }
    if cargo_abs.is_file() {
        Ok(cargo_abs)
    } else if sh::has("cargo") {
        Ok(PathBuf::from("cargo"))
    } else {
        Err("rustup instalou mas `cargo` não apareceu. Reabra o terminal e rode de novo.".into())
    }
}

/// Instala as libs de BUILD necessárias pra compilar a GUI (Slint) do fonte. Best-effort por SO.
pub fn ensure_build_deps() -> Result<(), String> {
    match os() {
        Os::Linux => ensure_build_deps_linux(),
        Os::Mac => ensure_build_deps_mac(),
        Os::Windows => ensure_build_deps_windows(),
    }
}

#[cfg(not(windows))]
fn pkg_install(mgr_args: &[&str]) -> Result<(), String> {
    let s = sudo();
    if s.is_empty() {
        run_words(mgr_args)
    } else {
        let mut v = vec![s];
        v.extend_from_slice(mgr_args);
        run_words(&v)
    }
}

#[cfg(not(windows))]
fn run_words(words: &[&str]) -> Result<(), String> {
    let (cmd, args) = words.split_first().ok_or("comando vazio")?;
    sh::run_inherit(cmd, args)
}

#[cfg(windows)]
fn ensure_build_deps_linux() -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
fn ensure_build_deps_linux() -> Result<(), String> {
    // Deps do Slint (X11/Wayland/GL/xcb + fontconfig) — espelham o install.sh da casa.
    match linux_family() {
        LinuxFam::Debian => {
            let _ = pkg_install(&["apt-get", "update", "-qq"]);
            pkg_install(&[
                "apt-get", "install", "-y", "build-essential", "pkg-config", "libx11-dev",
                "libxcursor-dev", "libxrandr-dev", "libxi-dev", "libxkbcommon-dev", "libwayland-dev",
                "libgl1-mesa-dev", "libxcb1-dev", "libxcb-render0-dev", "libxcb-shape0-dev",
                "libxcb-xfixes0-dev", "libfontconfig1-dev",
            ])?;
            // Fontes de cobertura ampla (não-latinos) — best-effort.
            let _ = pkg_install(&["apt-get", "install", "-y", "fonts-noto-core", "fonts-noto-cjk", "fonts-dejavu-core"]);
        }
        LinuxFam::Rpm => {
            let mgr = if sh::has("zypper") { "zypper" } else { "dnf" };
            if mgr == "zypper" {
                pkg_install(&[
                    "zypper", "--non-interactive", "install", "-y", "gcc", "gcc-c++", "make",
                    "pkg-config", "libX11-devel", "libXcursor-devel", "libXrandr-devel", "libXi-devel",
                    "libxkbcommon-devel", "wayland-devel", "Mesa-libGL-devel", "libxcb-devel",
                    "fontconfig-devel",
                ])?;
                let _ = pkg_install(&["zypper", "--non-interactive", "install", "-y", "noto-sans-fonts", "noto-sans-cjk-fonts", "dejavu-fonts"]);
            } else {
                pkg_install(&[
                    "dnf", "install", "-y", "gcc", "gcc-c++", "make", "pkg-config", "libX11-devel",
                    "libXcursor-devel", "libXrandr-devel", "libXi-devel", "libxkbcommon-devel",
                    "wayland-devel", "Mesa-libGL-devel", "libxcb-devel", "fontconfig-devel",
                ])?;
                let _ = pkg_install(&["dnf", "install", "-y", "google-noto-sans-fonts", "google-noto-sans-cjk-fonts", "dejavu-sans-fonts"]);
            }
        }
        LinuxFam::Arch => {
            pkg_install(&[
                "pacman", "-S", "--needed", "--noconfirm", "base-devel", "pkgconf", "libx11",
                "libxcursor", "libxrandr", "libxi", "libxkbcommon", "wayland", "mesa", "libxcb",
                "fontconfig", "noto-fonts", "noto-fonts-cjk",
            ])?;
        }
        LinuxFam::Unknown => {
            return Err(
                "distro Linux não reconhecida: instale manualmente as libs de X11/Wayland/GL/fontconfig \
                 (dev) e rode de novo."
                    .into(),
            );
        }
    }
    Ok(())
}

#[cfg(windows)]
fn ensure_build_deps_mac() -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
fn ensure_build_deps_mac() -> Result<(), String> {
    // macOS: precisa das Command Line Tools (cc/clang). `xcode-select --install` abre o instalador
    // gráfico (não bloqueante) se ainda não houver. Slint no Mac usa frameworks do sistema — sem brew.
    let clt_ok = sh::capture("xcode-select", &["-p"]).map(|p| !p.is_empty()).unwrap_or(false);
    if !clt_ok {
        println!("→ instalando as Command Line Tools do Xcode (aceite o popup)…");
        let _ = sh::run_inherit("xcode-select", &["--install"]);
        return Err(
            "conclua a instalação das Command Line Tools do Xcode (popup) e rode de novo."
                .into(),
        );
    }
    Ok(())
}

#[cfg(not(windows))]
fn ensure_build_deps_windows() -> Result<(), String> {
    Ok(())
}
#[cfg(windows)]
fn ensure_build_deps_windows() -> Result<(), String> {
    // Windows precisa do toolchain MSVC (link.exe) pra compilar. Detecta via `link` no PATH ou
    // uma instalação do VS Build Tools; se faltar, tenta winget (não interativo) e orienta.
    if sh::has("link") || sh::has("cl") {
        return Ok(());
    }
    if sh::has("winget") {
        println!("→ instalando o Visual Studio Build Tools (MSVC) via winget…");
        let _ = sh::run_inherit(
            "winget",
            &[
                "install", "--id", "Microsoft.VisualStudio.2022.BuildTools", "-e",
                "--accept-source-agreements", "--accept-package-agreements",
                "--override",
                "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools",
            ],
        );
        return Err(
            "instalei o MSVC Build Tools — REABRA o terminal (pra o PATH pegar) e rode de novo."
                .into(),
        );
    }
    Err(
        "no Windows a compilação do fonte precisa do MSVC Build Tools. Instale o \
         'Visual Studio Build Tools' (workload C++) e rode de novo, ou aguarde um binário pré-compilado."
            .into(),
    )
}

/// Garante que `install_dir()` (~/.cargo/bin) esteja no PATH pra o app rodar do terminal.
/// Unix: acrescenta export nos rc (idempotente). Windows: `setx PATH` (perene) do usuário.
pub fn ensure_path_setup() {
    #[cfg(not(windows))]
    {
        let line = "\n# schematize-updater: ~/.cargo/bin no PATH\nexport PATH=\"$HOME/.cargo/bin:$PATH\"\n";
        for rc in [".bashrc", ".profile", ".zshrc"] {
            let p = home().join(rc);
            let cur = std::fs::read_to_string(&p).unwrap_or_default();
            if !cur.contains(".cargo/bin") {
                let mut new = cur;
                new.push_str(&line);
                let _ = std::fs::write(&p, new);
            }
        }
    }
    #[cfg(windows)]
    {
        let dir_s = install_dir().to_string_lossy().to_string();
        // Só adiciona se ainda não estiver no PATH do usuário (evita duplicar).
        let cur = sh::capture("powershell", &["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariable('Path','User')"]).unwrap_or_default();
        if !cur.to_lowercase().contains(&dir_s.to_lowercase()) {
            let ps = format!(
                "[Environment]::SetEnvironmentVariable('Path', ([Environment]::GetEnvironmentVariable('Path','User') + ';{dir_s}'), 'User')"
            );
            let _ = sh::run_inherit("powershell", &["-NoProfile", "-Command", &ps]);
        }
    }
}

/// Cria o lançador da GUI (best-effort). Linux: .desktop com Exec ABSOLUTO (o bug do lançador
/// que abria o binário errado do PATH do DE). Mac/Windows: por ora só garante o binário no PATH.
pub fn make_launcher() {
    if os() != Os::Linux {
        return;
    }
    let (_, gui) = bin_names();
    let guibin = install_dir().join(&gui);
    if !guibin.is_file() {
        return;
    }
    let apps = home().join(".local/share/applications");
    let _ = std::fs::create_dir_all(&apps);

    // Gera o ícone em TODOS os tamanhos A PARTIR DO CÓDIGO (via o CLI `schematize icon`), pra o
    // Icon= abaixo resolver. RESILIENTE: o updater roda make_launcher em TODO update; antes ele
    // regravava o .desktop SEM Icon= e o dock (Wayland) perdia o ícone. Best-effort.
    let icons_dir = home().join(".local/share/icons/hicolor");
    let szbin = install_dir().join("schematize");
    let icon_png = icons_dir.join("256x256").join("apps").join("schematize.png");
    let _ = sh::capture(
        szbin.to_str().unwrap_or("schematize"),
        &["icon", "--hicolor", icons_dir.to_str().unwrap_or_default()],
    );
    // Icon= com caminho ABSOLUTO do 256px (à prova de cache/tema); se o png não saiu, cai pro nome.
    let icon = if icon_png.is_file() {
        icon_png.display().to_string()
    } else {
        "schematize".to_string()
    };
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=schematize\nGenericName=Ecossistema schematize\n\
         Comment=Skills, overdev e mais — schematize\nExec={}\nIcon={icon}\nTerminal=false\n\
         Categories=Development;Utility;\nKeywords=schematize;skills;overdev;claude;\n\
         StartupWMClass=schematize-gui\n",
        guibin.display()
    );
    let _ = std::fs::write(apps.join("schematize-gui.desktop"), desktop);
    let _ = sh::run_inherit("update-desktop-database", &[apps.to_str().unwrap_or_default()]);
    let _ = sh::capture("gtk-update-icon-cache", &["-f", "-t", icons_dir.to_str().unwrap_or_default()]);
}

// (helpers cross-OS abaixo)

/// Executa o app (GUI) instalado pelo caminho absoluto (não depende do PATH do DE).
pub fn launch_app() -> Result<(), String> {
    let (_, gui) = bin_names();
    let guibin = install_dir().join(&gui);
    let target = if guibin.is_file() { guibin } else { PathBuf::from(gui) };
    std::process::Command::new(&target)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("não consegui lançar {}: {e}", target.display()))
}

/// Caminho absoluto de um binário do app instalado (ou o nome nu se não achar).
pub fn app_bin(gui: bool) -> PathBuf {
    let (cli_n, gui_n) = bin_names();
    let name = if gui { gui_n } else { cli_n };
    let abs = install_dir().join(&name);
    if abs.is_file() {
        abs
    } else {
        PathBuf::from(name)
    }
}
