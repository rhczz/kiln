use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfig {
    #[serde(default)]
    pub paths: PathsConfig,
    pub site: SiteMeta,
    #[serde(default)]
    pub author: Option<AuthorConfig>,
    #[serde(default)]
    pub feed: FeedConfig,
    #[serde(default)]
    pub collections: Vec<CollectionConfig>,
    #[serde(default)]
    pub taxonomies: Vec<TaxonomyConfig>,
    #[serde(default)]
    pub paginate_by: usize,
    #[serde(default = "default_paginate_path")]
    pub paginate_path: String,
    #[serde(default, rename = "menu")]
    pub menus: std::collections::HashMap<String, Vec<MenuItemConfig>>,

    /// 所有未识别的配置表自动透传给模板，编译器不解析其结构
    #[serde(flatten)]
    pub extra: toml::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionConfig {
    pub name: String,
    pub directory: String,
    pub route: String,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub date_ordered: bool,
    #[serde(default)]
    pub feed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_content")]
    pub content: String,
    #[serde(default = "default_templates")]
    pub templates: String,
    #[serde(default = "default_public")]
    pub public: String,
    #[serde(default = "default_styles")]
    pub styles: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            content: default_content(),
            templates: default_templates(),
            public: default_public(),
            styles: default_styles(),
        }
    }
}

fn default_content() -> String {
    "content".into()
}
fn default_templates() -> String {
    "templates".into()
}
fn default_public() -> String {
    "public".into()
}
fn default_styles() -> String {
    "styles.css".into()
}

fn default_language() -> String {
    "en".into()
}

fn default_paginate_path() -> String {
    "page".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SiteMeta {
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_language")]
    pub language: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthorConfig {
    pub name: String,
    #[serde(default)]
    pub email: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeedConfig {
    #[serde(default = "default_feed_count")]
    pub item_count: usize,
}

impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            item_count: default_feed_count(),
        }
    }
}

fn default_feed_count() -> usize {
    20
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaxonomyConfig {
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub template: String,
}

impl TaxonomyConfig {
    pub fn effective_slug(&self) -> &str {
        if self.slug.is_empty() {
            &self.name
        } else {
            &self.slug
        }
    }
}

pub fn effective_taxonomies(config: &SiteConfig) -> Vec<TaxonomyConfig> {
    if config.taxonomies.is_empty() {
        vec![TaxonomyConfig {
            name: "tags".into(),
            slug: "tags".into(),
            template: "term.html".into(),
        }]
    } else {
        config.taxonomies.clone()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MenuItemConfig {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub weight: i32,
}

pub fn default_collections() -> Vec<CollectionConfig> {
    vec![
        CollectionConfig {
            name: "posts".into(),
            directory: "posts".into(),
            route: "/posts/{slug}/".into(),
            template: "post.html".into(),
            date_ordered: true,
            feed: true,
        },
        CollectionConfig {
            name: "pages".into(),
            directory: "pages".into(),
            route: "/{slug}/".into(),
            template: "page.html".into(),
            date_ordered: false,
            feed: false,
        },
    ]
}

pub fn effective_collections(config: &SiteConfig) -> Vec<CollectionConfig> {
    if config.collections.is_empty() {
        default_collections()
    } else {
        config.collections.clone()
    }
}

impl SiteConfig {
    pub fn load(config_path: &Path) -> anyhow::Result<(Self, PathBuf)> {
        let content = std::fs::read_to_string(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to read config {:?}: {}", config_path, e))?;
        let mut config: Self = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config {:?}: {}", config_path, e))?;

        if config.collections.is_empty() {
            config.collections = default_collections();
        }
        for col in &mut config.collections {
            if col.template.is_empty() {
                col.template = format!("{}.html", col.name);
            }
        }

        let base_dir = config_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        config.validate_raw(config_path)?;

        fn resolve(base: &Path, p: &str) -> String {
            let joined = base.join(p);
            std::fs::canonicalize(&joined)
                .unwrap_or(joined)
                .to_string_lossy()
                .to_string()
        }

        config.paths.content = resolve(&base_dir, &config.paths.content);
        config.paths.templates = resolve(&base_dir, &config.paths.templates);
        config.paths.public = resolve(&base_dir, &config.paths.public);
        config.paths.styles = resolve(&base_dir, &config.paths.styles);
        config.site.base_url = config.site.base_url.trim_end_matches('/').to_string();
        config.validate_resolved(config_path)?;

        Ok((config, base_dir))
    }

    pub(crate) fn validate_structure(&self) -> anyhow::Result<()> {
        self.validate_collection_taxonomy_rules(None)
    }

    fn validate_raw(&self, config_path: &Path) -> anyhow::Result<()> {
        if self.site.title.trim().is_empty() {
            anyhow::bail!(
                "Invalid config {:?}: site.title is required and cannot be empty",
                config_path
            );
        }

        let base_url = self.site.base_url.trim();
        if !(base_url.starts_with("https://") || base_url.starts_with("http://")) {
            anyhow::bail!(
                "Invalid config {:?}: site.base_url must start with http:// or https://, got {:?}",
                config_path,
                self.site.base_url
            );
        }

        validate_site_relative_path(Some(config_path), "paths.styles", &self.paths.styles)?;
        self.validate_collection_taxonomy_rules(Some(config_path))?;

        Ok(())
    }

    fn validate_collection_taxonomy_rules(&self, config_path: Option<&Path>) -> anyhow::Result<()> {
        // Validate collections
        let mut names = std::collections::HashSet::new();
        let mut dirs = std::collections::HashSet::new();
        let collections = effective_collections(self);
        for col in &collections {
            validate_config_segment(config_path, "collection.name", &col.name)?;
            validate_site_relative_path(config_path, "collection.directory", &col.directory)?;

            if !names.insert(&col.name) {
                return invalid_config(
                    config_path,
                    format!("duplicate collection name {:?}", col.name),
                );
            }
            if !dirs.insert(&col.directory) {
                return invalid_config(
                    config_path,
                    format!("duplicate collection directory {:?}", col.directory),
                );
            }
            if !col.route.starts_with('/') || !col.route.ends_with('/') {
                return invalid_config(
                    config_path,
                    format!(
                        "collection {:?} route must start and end with '/', got {:?}",
                        col.name, col.route
                    ),
                );
            }
            if !col.route.contains("{slug}") {
                return invalid_config(
                    config_path,
                    format!(
                        "collection {:?} route must contain '{{slug}}', got {:?}",
                        col.name, col.route
                    ),
                );
            }
            if col.route.contains("..") {
                return invalid_config(
                    config_path,
                    format!("collection {:?} route cannot contain '..'", col.name),
                );
            }
            if col.route.contains('\\') || col.route.contains("//") {
                return invalid_config(
                    config_path,
                    format!(
                        "collection {:?} route must be a normalized URL path, got {:?}",
                        col.name, col.route
                    ),
                );
            }
        }

        let mut taxonomy_names = std::collections::HashSet::new();
        let mut taxonomy_slugs = std::collections::HashSet::new();
        for taxonomy in effective_taxonomies(self) {
            validate_config_segment(config_path, "taxonomy.name", &taxonomy.name)?;
            let slug = taxonomy.effective_slug();
            validate_config_segment(config_path, "taxonomy.slug", slug)?;

            if !taxonomy_names.insert(taxonomy.name.clone()) {
                return invalid_config(
                    config_path,
                    format!("duplicate taxonomy name {:?}", taxonomy.name),
                );
            }
            if !taxonomy_slugs.insert(slug.to_string()) {
                return invalid_config(config_path, format!("duplicate taxonomy slug {:?}", slug));
            }

            for col in &collections {
                if collection_route_namespace(&col.route).as_deref() == Some(slug) {
                    return invalid_config(
                        config_path,
                        format!(
                            "taxonomy slug {:?} conflicts with collection {:?} route {:?}",
                            slug, col.name, col.route
                        ),
                    );
                }
            }
        }

        Ok(())
    }

    fn validate_resolved(&self, config_path: &Path) -> anyhow::Result<()> {
        let content = Path::new(&self.paths.content);
        if !content.is_dir() {
            anyhow::bail!(
                "Invalid config {:?}: paths.content must point to an existing directory, got {:?}",
                config_path,
                self.paths.content
            );
        }

        let styles = Path::new(&self.paths.styles);
        if !styles.is_file() {
            anyhow::bail!(
                "Invalid config {:?}: paths.styles must point to an existing CSS file, got {:?}",
                config_path,
                self.paths.styles
            );
        }

        Ok(())
    }
}

fn invalid_config(config_path: Option<&Path>, message: String) -> anyhow::Result<()> {
    if let Some(path) = config_path {
        anyhow::bail!("Invalid config {:?}: {}", path, message);
    }
    anyhow::bail!("Invalid config: {}", message);
}

fn validate_site_relative_path(
    config_path: Option<&Path>,
    field: &str,
    value: &str,
) -> anyhow::Result<()> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() {
        return invalid_config(
            config_path,
            format!(
                "{} must be a non-empty site-relative path, got {:?}",
                field, value
            ),
        );
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return invalid_config(
            config_path,
            format!("{} cannot contain '..', got {:?}", field, value),
        );
    }
    Ok(())
}

fn validate_config_segment(
    config_path: Option<&Path>,
    field: &str,
    value: &str,
) -> anyhow::Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed != value {
        return invalid_config(
            config_path,
            format!(
                "{} must be a non-empty normalized slug, got {:?}",
                field, value
            ),
        );
    }
    if trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.starts_with('.')
    {
        return invalid_config(
            config_path,
            format!(
                "{} must be a single URL-safe path segment, got {:?}",
                field, value
            ),
        );
    }
    Ok(())
}

fn collection_route_namespace(route: &str) -> Option<String> {
    let without_leading = route.strip_prefix('/')?;
    let first = without_leading.split('/').next().unwrap_or_default();
    if first.is_empty() || first.contains("{slug}") {
        None
    } else {
        Some(first.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::SiteConfig;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_invalid_base_url() {
        let root = temp_dir("kiln-config-invalid-url");
        write_minimal_site(&root, "ftp://example.com", "styles.css");
        let err = SiteConfig::load(&root.join("site.config.toml")).unwrap_err();
        assert!(err.to_string().contains("site.base_url"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_unsafe_styles_path() {
        let root = temp_dir("kiln-config-unsafe-styles");
        write_minimal_site(&root, "https://example.com", "../styles.css");
        let err = SiteConfig::load(&root.join("site.config.toml")).unwrap_err();
        assert!(err.to_string().contains("paths.styles"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn normalizes_base_url_and_resolves_paths() {
        let root = temp_dir("kiln-config-valid");
        write_minimal_site(&root, "https://example.com/", "styles.css");
        let (config, _) = SiteConfig::load(&root.join("site.config.toml")).unwrap();
        assert_eq!(config.site.base_url, "https://example.com");
        assert!(config.paths.styles.ends_with("styles.css"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loads_config_without_optional_tables() {
        let root = temp_dir("kiln-config-minimal");
        std::fs::create_dir_all(root.join("content/posts")).unwrap();
        std::fs::write(root.join("styles.css"), "body{}\n").unwrap();
        std::fs::write(
            root.join("site.config.toml"),
            r#"[site]
title = "Test"
base_url = "https://example.com"
"#,
        )
        .unwrap();

        let (config, _) = SiteConfig::load(&root.join("site.config.toml")).unwrap();
        assert!(config.paths.styles.ends_with("styles.css"));
        assert!(config.paths.content.ends_with("content"));
        assert_eq!(config.feed.item_count, 20);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_taxonomy_slug_that_conflicts_with_collection_namespace() {
        let root = temp_dir("kiln-config-taxonomy-route-conflict");
        write_minimal_site(&root, "https://example.com", "styles.css");
        std::fs::write(
            root.join("site.config.toml"),
            r#"[paths]
content = "content"
styles = "styles.css"

[site]
title = "Test"
base_url = "https://example.com"

[[collections]]
name = "articles"
directory = "posts"
route = "/tags/{slug}/"
"#,
        )
        .unwrap();

        let err = SiteConfig::load(&root.join("site.config.toml")).unwrap_err();
        assert!(err.to_string().contains("taxonomy slug"));
        assert!(err.to_string().contains("conflicts with collection"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_invalid_taxonomy_slug() {
        let root = temp_dir("kiln-config-invalid-taxonomy-slug");
        write_minimal_site(&root, "https://example.com", "styles.css");
        std::fs::write(
            root.join("site.config.toml"),
            r#"[paths]
content = "content"
styles = "styles.css"

[site]
title = "Test"
base_url = "https://example.com"

[[taxonomies]]
name = "topics"
slug = "../topics"
"#,
        )
        .unwrap();

        let err = SiteConfig::load(&root.join("site.config.toml")).unwrap_err();
        assert!(err.to_string().contains("taxonomy.slug"));
        assert!(err.to_string().contains("single URL-safe path segment"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_duplicate_taxonomy_slug() {
        let root = temp_dir("kiln-config-duplicate-taxonomy-slug");
        write_minimal_site(&root, "https://example.com", "styles.css");
        std::fs::write(
            root.join("site.config.toml"),
            r#"[paths]
content = "content"
styles = "styles.css"

[site]
title = "Test"
base_url = "https://example.com"

[[taxonomies]]
name = "tags"
slug = "topics"

[[taxonomies]]
name = "categories"
slug = "topics"
"#,
        )
        .unwrap();

        let err = SiteConfig::load(&root.join("site.config.toml")).unwrap_err();
        assert!(err.to_string().contains("duplicate taxonomy slug"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_invalid_collection_names() {
        for (suffix, name) in [
            ("slash", "bad/name"),
            ("backslash", "bad\\name"),
            ("whitespace", " posts"),
            ("dot", ".hidden"),
        ] {
            let root = temp_dir(&format!("kiln-config-invalid-collection-name-{suffix}"));
            write_collection_config(&root, name, "posts", "/posts/{slug}/");

            let err = SiteConfig::load(&root.join("site.config.toml")).unwrap_err();
            assert!(err.to_string().contains("collection.name"));
            assert!(
                err.to_string().contains("normalized slug")
                    || err.to_string().contains("single URL-safe path segment")
            );
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn rejects_invalid_collection_directories() {
        for (suffix, directory) in [
            ("parent", "../posts"),
            ("absolute", "/tmp/posts"),
            ("empty", ""),
        ] {
            let root = temp_dir(&format!(
                "kiln-config-invalid-collection-directory-{suffix}"
            ));
            write_collection_config(&root, "posts", directory, "/posts/{slug}/");

            let err = SiteConfig::load(&root.join("site.config.toml")).unwrap_err();
            assert!(err.to_string().contains("collection.directory"));
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn rejects_malformed_collection_routes() {
        for (suffix, route, expected) in [
            ("relative", "posts/{slug}/", "start and end"),
            ("no-trailing-slash", "/posts/{slug}", "start and end"),
            ("missing-slug", "/posts/", "must contain"),
            ("dotdot", "/../{slug}/", "cannot contain '..'"),
            ("double-slash", "/posts//{slug}/", "normalized URL path"),
            ("backslash", "/posts\\{slug}/", "normalized URL path"),
        ] {
            let root = temp_dir(&format!("kiln-config-invalid-collection-route-{suffix}"));
            write_collection_config(&root, "posts", "posts", route);

            let err = SiteConfig::load(&root.join("site.config.toml")).unwrap_err();
            assert!(err.to_string().contains("collection \"posts\" route"));
            assert!(err.to_string().contains(expected));
            let _ = std::fs::remove_dir_all(root);
        }
    }

    fn write_minimal_site(root: &std::path::Path, base_url: &str, styles_path: &str) {
        std::fs::create_dir_all(root.join("content/posts")).unwrap();
        std::fs::write(root.join("styles.css"), "body{}\n").unwrap();
        std::fs::write(
            root.join("site.config.toml"),
            format!(
                r#"[paths]
content = "content"
styles = "{styles_path}"

[site]
title = "Test"
base_url = "{base_url}"
"#
            ),
        )
        .unwrap();
    }

    fn write_collection_config(root: &std::path::Path, name: &str, directory: &str, route: &str) {
        std::fs::create_dir_all(root.join("content/posts")).unwrap();
        std::fs::write(root.join("styles.css"), "body{}\n").unwrap();
        std::fs::write(
            root.join("site.config.toml"),
            format!(
                r#"[paths]
content = "content"
styles = "styles.css"

[site]
title = "Test"
base_url = "https://example.com"

[[collections]]
name = '{name}'
directory = '{directory}'
route = '{route}'
"#
            ),
        )
        .unwrap();
    }

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), now))
    }
}
