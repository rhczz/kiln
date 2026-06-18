mod asset;
mod cache;
mod cli;
mod config;
mod content;
mod diagnostic;
mod engine;
mod manifest;
mod model;
mod output_safety;
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

pub use asset::AssetManifest;
pub use cache::BuildCache;
pub use config::{
    AuthorConfig, CollectionConfig, FeedConfig, MenuItemConfig, PathsConfig, SiteConfig, SiteMeta,
    TaxonomyConfig,
};
pub use diagnostic::{
    print_build_summary, Diagnostic, DiagnosticCollector, DiagnosticLevel, TemplateFrame,
};
pub use manifest::{BuildManifest, ManifestEntry};
pub use model::{PageKind, SiteModel};
pub use render::{Heading, RenderOutput};
pub use site::{build, build_with_artifacts, BuildArtifacts, BuildMode, BuildOptions};
