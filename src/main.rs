//! schematize-updater — bootstrapper/gestor de versão do ecossistema schematize.
//! O quê: instala e mantém o app (CLI `schematize` + GUI `schematize-gui`) atualizado na máquina
//! do usuário, CROSS-OS (Linux/macOS/Windows), de forma HÍBRIDA (binário pronto se houver, senão
//! compila do fonte). É DESACOPLADO do app — um app quebrado não trava o updater, e o updater é o
//! único artefato que publicamos pré-compilado por SO (pequeno, estável, recompilado só quando ELE
//! muda). Onde: ponto de entrada; despacha os subcomandos.

mod fetch;
mod install;
mod platform;
mod purga;
mod sh;
mod version;

/// Versão do PRÓPRIO updater (independente da versão do app).
const UPDATER_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    let rest = &args[args.len().min(1)..];

    let result: Result<(), String> = match cmd {
        "" | "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("schematize-updater {UPDATER_VERSION}");
            Ok(())
        }
        // Instala (1ª vez) — igual ao update, mas o nome deixa a intenção clara.
        "install" => install::install_or_update(rest.iter().any(|a| a == "--force")),
        // Atualiza pra versão-alvo (pin ou latest).
        "update" | "upgrade" => install::install_or_update(rest.iter().any(|a| a == "--force")),
        // Estado: versões (updater, app instalado, alvo), plataforma, pin.
        "status" => {
            print_status();
            Ok(())
        }
        // Lança a GUI instalada (pelo caminho absoluto — não depende do PATH do desktop).
        "run" | "launch" => platform::launch_app(),
        // Fixa uma versão do app (pin). `unpin` volta a seguir latest.
        "pin" => match rest.first() {
            Some(v) => version::write_pin(Some(v)).map(|_| println!("versão fixada em {v}.")),
            None => Err("uso: schematize-updater pin <versão>".into()),
        },
        "unpin" => version::write_pin(None).map(|_| println!("pin removido — seguindo latest.")),
        other => Err(format!("subcomando desconhecido: `{other}` (veja `schematize-updater help`).")),
    };

    if let Err(e) = result {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "schematize-updater {UPDATER_VERSION} — instala e atualiza o app schematize (cross-OS, híbrido)\n\
         \n\
         USO: schematize-updater <comando>\n\
         \n\
         COMANDOS:\n\
         \x20 install [--force]   Instala o app (CLI + GUI). Binário pronto se houver, senão compila do fonte.\n\
         \x20 update  [--force]   Atualiza pra versão-alvo (pin, ou a última publicada).\n\
         \x20 status              Mostra versões (updater, app instalado, alvo), plataforma e pin.\n\
         \x20 run                 Lança a GUI instalada.\n\
         \x20 pin <versão>        Fixa uma versão do app. `unpin` volta a seguir latest.\n\
         \x20 version             Versão do próprio updater.\n\
         \n\
         O updater é DESACOPLADO do app: um app quebrado não trava o update, e o update compila\n\
         na sua máquina quando não há binário pronto pra sua plataforma."
    );
}

fn print_status() {
    let (os, arch) = (platform::os(), std::env::consts::ARCH);
    let os_s = match os {
        platform::Os::Linux => format!("Linux ({:?})", platform::linux_family()),
        platform::Os::Mac => "macOS".to_string(),
        platform::Os::Windows => "Windows".to_string(),
    };
    println!("schematize-updater : v{UPDATER_VERSION}");
    println!("plataforma         : {os_s} / {arch}");
    println!(
        "binário pronto?    : {}",
        if platform::asset_names().is_some() { "sim (caminho rápido disponível)" } else { "não (compila do fonte)" }
    );
    println!(
        "app instalado      : {}",
        version::installed_app_version().map(|v| format!("v{v}")).unwrap_or_else(|| "nenhum".into())
    );
    println!(
        "última publicada   : {}",
        version::latest_app_version().map(|v| format!("v{v}")).unwrap_or_else(|| "? (rede)".into())
    );
    if let Some(p) = version::read_pin() {
        println!("versão fixada (pin): {p}");
    }
    println!("dir de instalação  : {}", platform::install_dir().display());
}
