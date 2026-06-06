use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "kiln", about = "A lean static site compiler", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Build the static site
    Build {
        /// Path to site.config.toml
        #[arg(long, default_value = "site/site.config.toml")]
        config: PathBuf,
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: PathBuf,
        /// Include draft posts
        #[arg(long)]
        drafts: bool,
        /// Emit detailed build profile with cache/render metrics
        #[arg(long)]
        profile: bool,
    },
    /// Validate site config and content without building
    Check {
        /// Path to site.config.toml
        #[arg(long, default_value = "site/site.config.toml")]
        config: PathBuf,
    },
    /// Start dev server with auto-rebuild
    Serve {
        /// Path to site.config.toml
        #[arg(long, default_value = "site/site.config.toml")]
        config: PathBuf,
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: PathBuf,
        /// Port to listen on
        #[arg(long, default_value = "4173")]
        port: u16,
    },
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Build {
            config,
            output,
            drafts,
            profile,
        } => {
            let (site_config, _base_dir) = crate::config::SiteConfig::load(&config)?;
            crate::site::build(&site_config, &output, drafts, profile)?;
        }
        Command::Check { config } => {
            let (site_config, _base_dir) = crate::config::SiteConfig::load(&config)?;
            let artifacts = crate::site::BuildArtifacts::load(&site_config)?;
            let temp_output = temp_check_dir()?;
            let _guard = TempDirGuard::new(temp_output.clone());

            crate::site::build_with_artifacts(
                &site_config,
                &temp_output,
                None,
                &artifacts,
                crate::site::BuildOptions {
                    include_drafts: false,
                    mode: crate::site::BuildMode::Full,
                    emit_report: false,
                    profile: false,
                },
            )?;
            eprintln!("Check passed.");
        }
        Command::Serve {
            config,
            output,
            port,
        } => {
            crate::serve::start(&config, &output, port)?;
        }
    }

    Ok(())
}

fn temp_check_dir() -> anyhow::Result<std::path::PathBuf> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("system clock before UNIX_EPOCH: {}", e))?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("kiln-check-{}-{}", std::process::id(), now));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

struct TempDirGuard(std::path::PathBuf);

impl TempDirGuard {
    fn new(path: std::path::PathBuf) -> Self {
        Self(path)
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
