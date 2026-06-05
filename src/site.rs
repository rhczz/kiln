use crate::cache::BuildCache;
use crate::config::{self as site_config, CollectionConfig, SiteConfig};
use crate::content::{self, ContentItem};
use crate::engine::Engine;
use crate::timing::BuildTimer;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn build(config: &SiteConfig, output_dir: &Path, include_drafts: bool) -> anyhow::Result<()> {
    let artifacts = BuildArtifacts::load(config)?;
    build_with_artifacts(
        config,
        output_dir,
        include_drafts,
        BuildMode::Full,
        None,
        &artifacts,
    )
}

pub fn build_public_incremental(
    config: &SiteConfig,
    output_dir: &Path,
    include_drafts: bool,
    cache: &mut BuildCache,
    artifacts: &BuildArtifacts,
) -> anyhow::Result<()> {
    build_with_artifacts(
        config,
        output_dir,
        include_drafts,
        BuildMode::Public,
        Some(cache),
        artifacts,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildMode {
    Full,
    Content,
    Public,
}

pub struct BuildArtifacts {
    pub engine: Engine,
    style_asset: StyleAsset,
}

impl BuildArtifacts {
    pub fn load(config: &SiteConfig) -> anyhow::Result<Self> {
        Ok(Self {
            engine: Engine::init(Path::new(&config.paths.templates))?,
            style_asset: StyleAsset::from_file(Path::new(&config.paths.styles))?,
        })
    }
}

pub fn build_with_artifacts(
    config: &SiteConfig,
    output_dir: &Path,
    include_drafts: bool,
    mode: BuildMode,
    mut cache: Option<&mut BuildCache>,
    artifacts: &BuildArtifacts,
) -> anyhow::Result<()> {
    let collections = effective_collections(config);
    let mut timer = BuildTimer::new();

    timer.phase("prepare_output");
    if matches!(mode, BuildMode::Full) && output_dir.exists() {
        std::fs::remove_dir_all(output_dir)?;
    }
    std::fs::create_dir_all(output_dir)?;

    timer.phase("copy_public");
    match (mode, cache.as_mut()) {
        (BuildMode::Content, _) => {}
        (BuildMode::Public, Some(cache)) => {
            copy_public_incremental(Path::new(&config.paths.public), output_dir, cache)?
        }
        (BuildMode::Public, None) => copy_dir(Path::new(&config.paths.public), output_dir)?,
        (BuildMode::Full, Some(cache)) => {
            copy_dir_recording(Path::new(&config.paths.public), output_dir, cache)?
        }
        (BuildMode::Full, None) => copy_dir(Path::new(&config.paths.public), output_dir)?,
    }

    if matches!(mode, BuildMode::Public) {
        rewrite_404_stylesheet(output_dir, &artifacts.style_asset)?;
        timer.finish();
        eprintln!("Public build done in {}ms", timer.total_ms());
        return Ok(());
    }

    timer.phase("load_content");
    let mut all_items: Vec<ContentItem> = Vec::new();
    for collection in &collections {
        let items = if let Some(cache) = cache.as_deref_mut() {
            content::load_collection_cached(
                &config.paths.content,
                collection,
                include_drafts,
                cache,
            )?
        } else {
            content::load_collection(&config.paths.content, collection, include_drafts)?
        };
        all_items.extend(items);
    }

    timer.phase("render_pages");
    let mut current_page_outputs: HashSet<PathBuf> = HashSet::new();
    let render_env = RenderEnv {
        engine: &artifacts.engine,
        config,
        style_asset: &artifacts.style_asset,
        collections: &collections,
        output_dir,
    };
    render_items(
        &render_env,
        &all_items,
        &mut cache,
        &mut current_page_outputs,
    )?;

    timer.phase("render_home");
    render_home(&render_env, output_dir, &all_items)?;

    if matches!(mode, BuildMode::Full) {
        timer.phase("write_assets");
        artifacts.style_asset.write(output_dir)?;
        write_asset_headers(output_dir)?;
    }

    if matches!(mode, BuildMode::Public | BuildMode::Full) {
        rewrite_404_stylesheet(output_dir, &artifacts.style_asset)?;
    }

    let output_count = current_page_outputs.len();
    if let Some(cache) = cache.as_mut() {
        let previous_outputs = cache.page_outputs().clone();
        prune_removed_outputs(output_dir, &previous_outputs, &current_page_outputs)?;
        cache.replace_page_outputs(current_page_outputs);
    }

    timer.phase("generate_feeds");
    let feed_items = collect_feed_items(&collections, &all_items);
    let rss = crate::rss::generate(config, &feed_items);
    std::fs::write(output_dir.join("rss.xml"), rss)?;

    let sitemap = crate::sitemap::generate(config, &all_items);
    std::fs::write(output_dir.join("sitemap.xml"), sitemap)?;

    std::fs::write(
        output_dir.join("robots.txt"),
        format!(
            "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
            config.site.base_url
        ),
    )?;

    timer.finish();
    let date_ordered_count = all_items.iter().filter(|i| i.year.is_some()).count();
    if date_ordered_count == 0 {
        eprintln!(
            "Warning: no date-ordered items found in {:?}",
            config.paths.content
        );
    }
    timer.print_report(all_items.len(), date_ordered_count, output_count);

    Ok(())
}

fn effective_collections(config: &SiteConfig) -> Vec<CollectionConfig> {
    if config.collections.is_empty() {
        site_config::default_collections()
    } else {
        config.collections.clone()
    }
}

struct RenderEnv<'a> {
    engine: &'a Engine,
    config: &'a SiteConfig,
    style_asset: &'a StyleAsset,
    collections: &'a [CollectionConfig],
    output_dir: &'a Path,
}

fn render_items(
    env: &RenderEnv<'_>,
    all_items: &[ContentItem],
    cache: &mut Option<&mut BuildCache>,
    current_page_outputs: &mut HashSet<PathBuf>,
) -> anyhow::Result<()> {
    for item in all_items {
        let collection = env
            .collections
            .iter()
            .find(|c| c.name == item.collection)
            .ok_or_else(|| anyhow::anyhow!("unknown collection '{}'", item.collection))?;

        let output_path = page_output_path(env.output_dir, &item.url);
        current_page_outputs.insert(output_path.clone());

        let html = if let Some(cache) = cache.as_mut().map(|cache| &mut **cache) {
            let render_hash = format!("{}:{}", item.content_hash, collection.template);
            if let Some(cached) = cache.cached_render(&item.source_path, &render_hash) {
                cached.to_string()
            } else {
                let rendered = render_item(env, collection, item)?;
                cache.store_render(&item.source_path, render_hash, rendered.clone());
                rendered
            }
        } else {
            render_item(env, collection, item)?
        };

        write_page_if_changed(&output_path, &html)?;
    }

    Ok(())
}

fn render_item(
    env: &RenderEnv<'_>,
    collection: &CollectionConfig,
    item: &ContentItem,
) -> anyhow::Result<String> {
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset);
    ctx.insert("page", &make_item_context(item));

    let body = env.engine.render(&collection.template, &ctx)?;
    let dir_path = item.url.trim_start_matches('/').trim_end_matches('/');
    wrap_with_layout(
        env,
        &item.title,
        &item.description,
        &body,
        dir_path,
        collection.date_ordered,
    )
}

fn render_home(
    env: &RenderEnv<'_>,
    output_dir: &Path,
    all_items: &[ContentItem],
) -> anyhow::Result<()> {
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset);

    let featured: Vec<&ContentItem> = all_items.iter().filter(|i| i.featured).take(6).collect();
    let featured_ctx: Vec<serde_json::Value> = featured.iter().map(|i| make_item_base(i)).collect();
    ctx.insert("featured_posts", &featured_ctx);

    let archive_items: Vec<&ContentItem> = all_items.iter().filter(|i| i.year.is_some()).collect();
    let archive = make_archive(&archive_items);
    ctx.insert("archive", &archive);

    let body = env.engine.render("home.html", &ctx)?;
    let wrapped = wrap_with_layout(env, "", &env.config.site.description, &body, "", false)?;
    write_page_if_changed(&output_dir.join("index.html"), &wrapped)?;
    Ok(())
}

fn page_output_path(output_dir: &Path, url: &str) -> PathBuf {
    output_dir
        .join(url.trim_start_matches('/').trim_end_matches('/'))
        .join("index.html")
}

fn write_page_if_changed(path: &Path, html: &str) -> anyhow::Result<()> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        if existing == html {
            return Ok(());
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, html)?;
    Ok(())
}

fn prune_removed_outputs(
    output_dir: &Path,
    previous: &HashSet<PathBuf>,
    current: &HashSet<PathBuf>,
) -> anyhow::Result<()> {
    for removed in previous.difference(current) {
        let _ = std::fs::remove_file(removed);
    }
    cleanup_empty_dirs(output_dir)?;
    Ok(())
}

fn cleanup_empty_dirs(root: &Path) -> anyhow::Result<()> {
    let mut dirs = Vec::new();
    collect_dirs(root, &mut dirs)?;
    dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for dir in dirs {
        if dir == root {
            continue;
        }
        if std::fs::read_dir(&dir)?.next().is_none() {
            let _ = std::fs::remove_dir(&dir);
        }
    }
    Ok(())
}

fn collect_dirs(dir: &Path, dirs: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    dirs.push(dir.to_path_buf());
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_dirs(&path, dirs)?;
        }
    }
    Ok(())
}

fn collect_feed_items<'a>(
    collections: &[CollectionConfig],
    all_items: &'a [ContentItem],
) -> Vec<&'a ContentItem> {
    let mut feed_items: Vec<&ContentItem> = all_items
        .iter()
        .filter(|item| {
            collections
                .iter()
                .any(|c| c.name == item.collection && c.feed)
        })
        .collect();
    feed_items.sort_by(|a, b| b.raw_date.cmp(&a.raw_date).then_with(|| a.url.cmp(&b.url)));
    feed_items
}

fn wrap_with_layout(
    env: &RenderEnv<'_>,
    title: &str,
    description: &str,
    body: &str,
    path: &str,
    is_article: bool,
) -> anyhow::Result<String> {
    let og_type = if is_article { "article" } else { "website" };
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset);
    ctx.insert("title", title);
    ctx.insert("description", description);
    ctx.insert("body", body);
    ctx.insert("path", path);
    ctx.insert("og_type", og_type);
    env.engine.render("layout.html", &ctx)
}

fn insert_template_context(ctx: &mut tera::Context, config: &SiteConfig, style_asset: &StyleAsset) {
    let site = serde_json::json!({
        "title": config.site.title,
        "subtitle": config.site.subtitle,
        "description": config.site.description,
        "language": config.site.language,
        "base_url": config.site.base_url,
        "stylesheet_href": style_asset.href,
        "author": {
            "name": config.author.as_ref().map(|a| a.name.as_str()).unwrap_or(""),
            "email": config.author.as_ref().map(|a| a.email.as_str()).unwrap_or(""),
        }
    });

    let theme = serde_json::to_value(&config.extra)
        .expect("toml::Value -> serde_json::Value is infallible");

    // Legacy `config` merges site fields into theme for backward compatibility
    let mut legacy = match &theme {
        serde_json::Value::Object(map) => map.clone(),
        _ => Default::default(),
    };
    if let Some(site_obj) = site.as_object() {
        for (key, value) in site_obj {
            legacy.insert(key.clone(), value.clone());
        }
    }

    ctx.insert("site", &site);
    ctx.insert("theme", &theme);
    ctx.insert("config", &serde_json::Value::Object(legacy));
}

fn make_item_base(item: &ContentItem) -> serde_json::Value {
    serde_json::json!({
        "title": item.title,
        "slug": item.slug,
        "date": item.date.as_deref().unwrap_or(""),
        "iso_date": item.iso_date.as_deref().unwrap_or(""),
        "short_date": item.short_date.as_deref().unwrap_or(""),
        "long_date": item.long_date.as_deref().unwrap_or(""),
        "year": item.year.as_deref().unwrap_or(""),
        "description": item.description,
        "url": item.url,
    })
}

fn make_item_context(item: &ContentItem) -> serde_json::Value {
    let mut ctx = make_item_base(item);
    if let Some(obj) = ctx.as_object_mut() {
        obj.insert("body_html".into(), serde_json::json!(item.body_html));
        obj.insert("tags".into(), serde_json::json!(item.tags));
        obj.insert("type".into(), serde_json::json!(item.collection));
    }
    ctx
}

fn make_archive(items: &[&ContentItem]) -> serde_json::Value {
    use std::collections::BTreeMap;
    let mut archive: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    for item in items {
        let year = item.year.as_deref().unwrap_or("unknown");
        archive
            .entry(year.to_string())
            .or_default()
            .push(make_item_base(item));
    }
    let mut result: Vec<serde_json::Value> = Vec::new();
    for (year, posts) in archive.iter().rev() {
        result.push(serde_json::json!({
            "year": year,
            "posts": posts,
        }));
    }
    serde_json::Value::Array(result)
}

fn copy_dir(src: &Path, dst: &Path) -> anyhow::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap_or_default();
        let dest = dst.join(name);
        if path.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_dir(&path, &dest)?;
        } else {
            std::fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

fn copy_dir_recording(src: &Path, dst: &Path, cache: &mut BuildCache) -> anyhow::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap_or_default();
        let dest = dst.join(name);
        if path.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_dir_recording(&path, &dest, cache)?;
        } else {
            std::fs::copy(&path, &dest)?;
            cache.store_public_hash(path.clone(), file_hash(&path)?);
            cache.add_public_output(dest);
        }
    }
    Ok(())
}

fn copy_public_incremental(src: &Path, dst: &Path, cache: &mut BuildCache) -> anyhow::Result<()> {
    if !src.exists() {
        return Ok(());
    }

    let mut current_outputs = HashSet::new();
    copy_public_recursive(src, dst, src, cache, &mut current_outputs)?;

    for removed in cache.public_outputs().difference(&current_outputs) {
        let _ = std::fs::remove_file(removed);
    }
    cache.replace_public_outputs(current_outputs);
    Ok(())
}

fn copy_public_recursive(
    root: &Path,
    dst_root: &Path,
    path: &Path,
    cache: &mut BuildCache,
    current_outputs: &mut HashSet<PathBuf>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            copy_public_recursive(root, dst_root, &path, cache, current_outputs)?;
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let dest = dst_root.join(rel);
            let hash = file_hash(&path)?;
            let should_copy = cache
                .copied_public_hash(&path)
                .map(|cached| cached != hash)
                .unwrap_or(true)
                || !dest.exists();
            if should_copy {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&path, &dest)?;
                cache.store_public_hash(path.clone(), hash);
            }
            current_outputs.insert(dest);
        }
    }
    Ok(())
}

fn file_hash(path: &Path) -> anyhow::Result<String> {
    let content = std::fs::read(path)?;
    Ok(content::fingerprint(&content))
}

struct StyleAsset {
    href: String,
    output_path: String,
    content: Vec<u8>,
}

impl StyleAsset {
    fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("Failed to read stylesheet {:?}: {}", path, e))?;
        let hash = content::fingerprint(&content);
        let output_path = format!("assets/styles.{}.css", hash);
        Ok(Self {
            href: format!("/{}", output_path),
            output_path,
            content,
        })
    }

    fn write(&self, output_dir: &Path) -> anyhow::Result<()> {
        let dest = output_dir.join(&self.output_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, &self.content)?;
        Ok(())
    }
}

fn write_asset_headers(output_dir: &Path) -> anyhow::Result<()> {
    let headers_path = output_dir.join("_headers");
    let rule = "\n/assets/*.css\n  Cache-Control: public, max-age=31536000, immutable\n";
    let mut headers = if headers_path.exists() {
        std::fs::read_to_string(&headers_path)?
    } else {
        String::new()
    };
    if !headers.contains("/assets/*.css") {
        headers.push_str(rule);
        std::fs::write(headers_path, headers)?;
    }
    Ok(())
}

fn rewrite_404_stylesheet(output_dir: &Path, style_asset: &StyleAsset) -> anyhow::Result<()> {
    let page = output_dir.join("404.html");
    if !page.exists() {
        return Ok(());
    }

    let html = std::fs::read_to_string(&page)?;
    let rewritten = html.replace(
        r#"href="/styles.css""#,
        &format!(r#"href="{}""#, style_asset.href),
    );
    if rewritten != html {
        std::fs::write(page, rewritten)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build;
    use crate::config::{AuthorConfig, FeedConfig, PathsConfig, SiteConfig, SiteMeta};
    use crate::content::ContentItem;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn builds_fixture_with_hashed_styles_and_default_templates() {
        let root = temp_dir("kiln-site-test");
        let content = root.join("content");
        let posts = content.join("posts");
        let styles = root.join("styles.css");
        let output = root.join("dist");

        std::fs::create_dir_all(&posts).unwrap();
        std::fs::write(
            posts.join("2026-06-02-example.md"),
            r#"---
title: "Example"
date: "2026-06-02"
description: "Example description"
featured: true
tags: ["Test"]
---

## Heading

| A | B |
|---|---|
| 1 | 2 |
"#,
        )
        .unwrap();
        std::fs::write(&styles, "body { color: red; }\n").unwrap();

        let config = SiteConfig {
            paths: PathsConfig {
                content: content.to_string_lossy().to_string(),
                templates: root.join("missing-templates").to_string_lossy().to_string(),
                public: root.join("missing-public").to_string_lossy().to_string(),
                styles: styles.to_string_lossy().to_string(),
            },
            site: SiteMeta {
                title: "Fixture".into(),
                subtitle: String::new(),
                description: "Fixture site".into(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: Some(AuthorConfig {
                name: "Tester".into(),
                email: String::new(),
            }),
            feed: FeedConfig { item_count: 20 },
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
        };

        build(&config, &output, false).unwrap();

        let index = read(&output.join("index.html"));
        assert!(index.contains(r#"<link rel="stylesheet" href="/assets/styles."#));
        assert!(!output.join("styles.css").exists());
        assert!(has_hashed_css(&output));

        let post = read(&output.join("posts/example/index.html"));
        assert!(post.contains(r#"class="heading-anchor""#));
        assert!(post.contains(r#"class="table-scroll""#));

        assert!(output.join("rss.xml").is_file());
        assert!(output.join("sitemap.xml").is_file());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rewrites_copied_404_stylesheet_to_hashed_asset() {
        let root = temp_dir("kiln-404-test");
        let output = root.join("dist");
        let style_asset = super::StyleAsset {
            href: "/assets/styles.abcdef123456.css".into(),
            output_path: "assets/styles.abcdef123456.css".into(),
            content: Vec::new(),
        };

        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(
            output.join("404.html"),
            r#"<link rel="stylesheet" href="/styles.css">"#,
        )
        .unwrap();

        super::rewrite_404_stylesheet(&output, &style_asset).unwrap();
        assert!(read(&output.join("404.html"))
            .contains(r#"<link rel="stylesheet" href="/assets/styles.abcdef123456.css">"#));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn public_assets_do_not_overwrite_generated_pages() {
        let root = temp_dir("kiln-public-override-test");
        let content = root.join("content/posts");
        let public = root.join("public");
        let styles = root.join("styles.css");
        let output = root.join("dist");

        std::fs::create_dir_all(&content).unwrap();
        std::fs::create_dir_all(&public).unwrap();
        std::fs::write(
            content.join("2026-01-01-hello.md"),
            r#"---
title: "Hello"
date: "2026-01-01"
---
Content here."#,
        )
        .unwrap();
        // Static index.html in public that should NOT override the generated homepage
        std::fs::write(public.join("index.html"), "<html>static</html>").unwrap();
        std::fs::write(&styles, "body{}\n").unwrap();

        let config = SiteConfig {
            paths: PathsConfig {
                content: content.parent().unwrap().to_string_lossy().to_string(),
                templates: root.join("nope").to_string_lossy().to_string(),
                public: public.to_string_lossy().to_string(),
                styles: styles.to_string_lossy().to_string(),
            },
            site: SiteMeta {
                title: "Test".into(),
                subtitle: String::new(),
                description: "Desc".into(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: None,
            feed: FeedConfig { item_count: 20 },
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
        };

        build(&config, &output, false).unwrap();

        let index = read(&output.join("index.html"));
        assert!(
            !index.contains("static"),
            "public/index.html should not override generated homepage"
        );
        assert!(
            index.contains("Test"),
            "generated homepage should contain site title"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn exposes_explicit_site_and_theme_contexts() {
        let config = SiteConfig {
            paths: PathsConfig {
                content: ".".into(),
                templates: ".".into(),
                public: ".".into(),
                styles: "styles.css".into(),
            },
            site: SiteMeta {
                title: "Test".into(),
                subtitle: "Sub".into(),
                description: "Desc".into(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: Some(AuthorConfig {
                name: "Tester".into(),
                email: "test@example.com".into(),
            }),
            feed: FeedConfig { item_count: 20 },
            collections: vec![],
            extra: toml::Value::Table(
                vec![
                    ("intro".into(), toml::Value::String("Hello".into())),
                    ("email".into(), toml::Value::String("hi@example.com".into())),
                ]
                .into_iter()
                .collect(),
            ),
        };
        let style_asset = super::StyleAsset {
            href: "/assets/styles.abc.css".into(),
            output_path: "assets/styles.abc.css".into(),
            content: Vec::new(),
        };

        let mut ctx = tera::Context::new();
        super::insert_template_context(&mut ctx, &config, &style_asset);

        let site = ctx.get("site").unwrap();
        let theme = ctx.get("theme").unwrap();
        let config_legacy = ctx.get("config").unwrap();

        assert_eq!(site["title"], "Test");
        assert_eq!(site["stylesheet_href"], "/assets/styles.abc.css");
        assert_eq!(theme["intro"], "Hello");
        assert_eq!(config_legacy["title"], "Test");
        assert_eq!(config_legacy["intro"], "Hello");
    }

    #[test]
    fn collects_feed_items_in_descending_date_order() {
        let collections = vec![
            crate::config::CollectionConfig {
                name: "posts".into(),
                directory: "posts".into(),
                route: "/posts/{slug}/".into(),
                template: "post.html".into(),
                date_ordered: true,
                feed: true,
            },
            crate::config::CollectionConfig {
                name: "notes".into(),
                directory: "notes".into(),
                route: "/notes/{slug}/".into(),
                template: "page.html".into(),
                date_ordered: true,
                feed: true,
            },
        ];

        let items = vec![
            ContentItem {
                source_path: PathBuf::from("content/posts/2026-05-01-older.md"),
                content_hash: "older".into(),
                title: "Older".into(),
                slug: "older".into(),
                description: String::new(),
                body_html: String::new(),
                collection: "posts".into(),
                url: "/posts/older/".into(),
                date: Some("2026-05-01".into()),
                iso_date: Some("2026-05-01".into()),
                short_date: Some("2026.05.01".into()),
                long_date: Some("May 1, 2026".into()),
                year: Some("2026".into()),
                featured: false,
                draft: false,
                tags: vec![],
                raw_date: chrono::NaiveDate::from_ymd_opt(2026, 5, 1),
            },
            ContentItem {
                source_path: PathBuf::from("content/notes/2026-06-01-newer.md"),
                content_hash: "newer".into(),
                title: "Newer".into(),
                slug: "newer".into(),
                description: String::new(),
                body_html: String::new(),
                collection: "notes".into(),
                url: "/notes/newer/".into(),
                date: Some("2026-06-01".into()),
                iso_date: Some("2026-06-01".into()),
                short_date: Some("2026.06.01".into()),
                long_date: Some("June 1, 2026".into()),
                year: Some("2026".into()),
                featured: false,
                draft: false,
                tags: vec![],
                raw_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 1),
            },
            ContentItem {
                source_path: PathBuf::from("content/pages/ignored.md"),
                content_hash: "ignored".into(),
                title: "Ignored".into(),
                slug: "ignored".into(),
                description: String::new(),
                body_html: String::new(),
                collection: "pages".into(),
                url: "/ignored/".into(),
                date: None,
                iso_date: None,
                short_date: None,
                long_date: None,
                year: None,
                featured: false,
                draft: false,
                tags: vec![],
                raw_date: None,
            },
        ];

        let feed_items = super::collect_feed_items(&collections, &items);
        assert_eq!(feed_items.len(), 2);
        assert_eq!(feed_items[0].title, "Newer");
        assert_eq!(feed_items[1].title, "Older");
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), now))
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {:?}: {}", path, e))
    }

    fn has_hashed_css(output: &Path) -> bool {
        let assets = output.join("assets");
        std::fs::read_dir(assets)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                name.starts_with("styles.") && name.ends_with(".css")
            })
    }
}
