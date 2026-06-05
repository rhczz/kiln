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
mod timing;

pub fn run() -> anyhow::Result<()> {
    cli::run()
}

pub use cache::BuildCache;
pub use config::{AuthorConfig, CollectionConfig, FeedConfig, PathsConfig, SiteConfig, SiteMeta};
pub use site::{build, build_with_artifacts, BuildArtifacts, BuildMode};
