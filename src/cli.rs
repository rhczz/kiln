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
        } => {
            let (site_config, _base_dir) = crate::config::SiteConfig::load(&config)?;
            crate::site::build(&site_config, &output, drafts)?;
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
