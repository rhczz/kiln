mod cache;
mod cli;
mod config;
mod content;
mod engine;
mod render;
mod rss;
mod serve;
mod site;
mod sitemap;

fn main() -> anyhow::Result<()> {
    cli::run()
}
