mod images;
mod model;
mod powershell;
mod providers;
mod seed;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use model::*;

#[derive(Parser)]
#[command(name = "visor-core", version, about = "visor hypervisor core — JSON CLI over Hyper-V and WSL2")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProviderKind {
    Hyperv,
    Wsl,
}

#[derive(Subcommand)]
enum Cmd {
    /// Report what this host can do (hyperv present, wsl present, elevated)
    Capabilities,
    /// Create a workspace
    Create {
        /// Provider to use
        #[arg(long, value_enum, default_value_t = ProviderKind::Wsl)]
        provider: ProviderKind,
        /// Workspace name (unique)
        #[arg(long)]
        name: String,
        /// Guest OS image (ubuntu-24.04, windows-11)
        #[arg(long, default_value = "ubuntu-24.04")]
        os: String,
        #[arg(long, default_value_t = 2)]
        cpus: u32,
        /// RAM in MiB
        #[arg(long, default_value_t = 4096)]
        memory: u32,
        /// Disk in GiB
        #[arg(long, default_value_t = 20)]
        disk: u32,
        /// Path to an SSH public key to inject (hyperv/linux guests)
        #[arg(long)]
        ssh_key: Option<String>,
        /// Extra apt packages to install on first boot (linux)
        #[arg(long, value_delimiter = ',')]
        packages: Vec<String>,
    },
    Start { name: String },
    Stop {
        name: String,
        #[arg(long)]
        force: bool,
    },
    Delete { name: String },
    List,
    Status { name: String },
    /// Print the guest IP (hyperv)
    Ip { name: String },
    /// Run a command inside a workspace (wsl)
    Exec {
        name: String,
        #[arg(trailing_var_arg = true, required = true)]
        cmd: Vec<String>,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match run(cli) {
        Ok(out) => {
            if let Some(out) = out {
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            }
        }
        Err(e) => {
            let err = serde_json::json!({ "error": format!("{e:#}") });
            eprintln!("{}", serde_json::to_string(&err).unwrap());
            std::process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<Option<serde_json::Value>> {
    match cli.cmd {
        Cmd::Capabilities => {
            let caps = providers::capabilities();
            Ok(Some(serde_json::to_value(caps)?))
        }
        Cmd::Create {
            provider,
            name,
            os,
            cpus,
            memory,
            disk,
            ssh_key,
            packages,
        } => {
            let spec = CreateSpec {
                name: name.clone(),
                os,
                cpus,
                memory_mib: memory,
                disk_gib: disk,
                ssh_public_key: ssh_key.map(std::fs::read_to_string).transpose()?,
                packages,
            };
            let ws = pick(provider).create(&spec)?;
            model::save_meta(&ws)?;
            Ok(Some(serde_json::to_value(ws)?))
        }
        Cmd::Start { name } => Ok(Some(serde_json::to_value(find(&name)?.start(&name)?)?)),
        Cmd::Stop { name, force } => {
            Ok(Some(serde_json::to_value(find(&name)?.stop(&name, force)?)?))
        }
        Cmd::Delete { name } => {
            find(&name)?.delete(&name)?;
            model::delete_meta(&name)?;
            Ok(None)
        }
        Cmd::List => {
            let mut all = Vec::new();
            for p in providers::all() {
                all.extend(p.list().unwrap_or_default());
            }
            Ok(Some(serde_json::to_value(all)?))
        }
        Cmd::Status { name } => Ok(Some(serde_json::to_value(find(&name)?.status(&name)?)?)),
        Cmd::Ip { name } => {
            let ws = find(&name)?.status(&name)?;
            Ok(Some(serde_json::json!({ "ip": ws.ip })))
        }
        Cmd::Exec { name, cmd } => {
            let meta = model::load_meta(&name)?;
            if meta.provider != "wsl" {
                anyhow::bail!("exec via visor-core is only for wsl workspaces; use SSH for hyperv");
            }
            let out = providers::wsl::WslProvider.exec(&name, &cmd)?;
            Ok(Some(out))
        }
    }
}

fn pick(kind: ProviderKind) -> Box<dyn providers::Provider> {
    match kind {
        ProviderKind::Hyperv => Box::new(providers::hyperv::HyperVProvider),
        ProviderKind::Wsl => Box::new(providers::wsl::WslProvider),
    }
}

fn find(name: &str) -> Result<Box<dyn providers::Provider>> {
    let meta = model::load_meta(name)?;
    Ok(match meta.provider.as_str() {
        "hyperv" => Box::new(providers::hyperv::HyperVProvider),
        "wsl" => Box::new(providers::wsl::WslProvider),
        other => anyhow::bail!("unknown provider in metadata: {other}"),
    })
}
