mod cache;
mod cli;
mod config;
mod content;
mod diagnostic;
mod engine;
mod manifest;
mod model;
mod paginator;
mod render;
mod rss;
mod serve;
mod shortcode;
mod site;
mod sitemap;
mod timing;

pub fn run() -> anyhow::Result<()> {
    cli::run()
}

pub use cache::BuildCache;
pub use config::{
    AuthorConfig, CollectionConfig, FeedConfig, MenuItemConfig, PathsConfig, SiteConfig, SiteMeta,
    TaxonomyConfig,
};
pub use diagnostic::{Diagnostic, DiagnosticLevel};
pub use manifest::{BuildManifest, ManifestEntry};
pub use model::{PageKind, SiteModel};
pub use render::{Heading, RenderOutput};
pub use site::{build, build_with_artifacts, BuildArtifacts, BuildMode};
