use anyhow::Context;

use crate::cache::BuildCache;
use crate::config::{self as site_config, CollectionConfig, SiteConfig};
use crate::content::{self, ContentItem};
use crate::engine::Engine;
use crate::model;
use crate::timing::BuildTimer;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn build(
    config: &SiteConfig,
    output_dir: &Path,
    include_drafts: bool,
    profile: bool,
) -> anyhow::Result<()> {
    let artifacts = BuildArtifacts::load(config)?;
    let opts = BuildOptions {
        include_drafts,
        mode: BuildMode::Full,
        emit_report: true,
        profile,
    };

    if profile {
        let mut cache = BuildCache::new();
        build_with_artifacts(config, output_dir, Some(&mut cache), &artifacts, opts)
    } else {
        build_with_artifacts(config, output_dir, None, &artifacts, opts)
    }
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
        Some(cache),
        artifacts,
        BuildOptions {
            include_drafts,
            mode: BuildMode::Public,
            emit_report: true,
            profile: false,
        },
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildMode {
    Full,
    Content,
    Public,
}

pub struct BuildOptions {
    pub include_drafts: bool,
    pub mode: BuildMode,
    pub emit_report: bool,
    pub profile: bool,
}

pub struct BuildArtifacts {
    pub engine: Engine,
    pub style_asset: StyleAsset,
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
    mut cache: Option<&mut BuildCache>,
    artifacts: &BuildArtifacts,
    opts: BuildOptions,
) -> anyhow::Result<()> {
    let collections = effective_collections(config);
    let mut timer = if opts.profile {
        BuildTimer::with_profile()
    } else {
        BuildTimer::new()
    };
    let mut diagnostics = crate::DiagnosticCollector::new();
    if let Some(cache) = cache.as_mut() {
        cache.reset_stats();
    }

    timer.phase("prepare_output");
    if matches!(opts.mode, BuildMode::Full) && output_dir.exists() {
        std::fs::remove_dir_all(output_dir)?;
    }
    std::fs::create_dir_all(output_dir)?;

    timer.phase("copy_public");
    let asset_manifest = if !matches!(opts.mode, BuildMode::Content) {
        let prev_manifest = crate::AssetManifest::load(output_dir).unwrap_or_default();
        let manifest =
            crate::asset::fingerprint_public(Path::new(&config.paths.public), output_dir)?;
        manifest.save(output_dir)?;
        crate::asset::prune_stale(&manifest, output_dir)?;
        // Also remove stale non-fingerprinted files from previous manifest
        for orig in prev_manifest.mappings.keys() {
            if !manifest.mappings.contains_key(orig) {
                let path = output_dir.join(orig);
                if path.is_file() {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        if let Some(cache) = cache.as_mut() {
            for (orig, hashed) in &manifest.mappings {
                cache.store_public_hash(
                    PathBuf::from(orig),
                    if orig == hashed {
                        orig.clone()
                    } else {
                        hashed.clone()
                    },
                );
                cache.add_public_output(output_dir.join(hashed));
            }
        }
        manifest
    } else {
        crate::AssetManifest::load(output_dir).unwrap_or_default()
    };

    timer.phase("load_content");
    let all_items: Vec<ContentItem> = {
        let mut items: Vec<ContentItem> = Vec::new();
        for collection in &collections {
            let loaded = if let Some(cache) = cache.as_deref_mut() {
                content::load_collection_cached(
                    &config.paths.content,
                    collection,
                    opts.include_drafts,
                    cache,
                )?
            } else {
                content::load_collection(&config.paths.content, collection, opts.include_drafts)?
            };
            items.extend(loaded);
        }
        items
    };

    let config_hash = build_config_hash(config);
    let asset_hash = asset_manifest.content_hash();
    // Update the engine's asset_url function with the current manifest
    artifacts
        .engine
        .update_asset_mappings(asset_manifest.mappings.clone());

    timer.phase("render_pages");
    let site_model = model::build_site_model(all_items, &collections, config);
    let mut current_page_outputs: HashSet<PathBuf> = HashSet::new();
    let render_env = RenderEnv {
        engine: &artifacts.engine,
        config,
        style_asset: &artifacts.style_asset,
        output_dir,
        asset_manifest: &asset_manifest,
        config_hash: &config_hash,
        asset_hash: &asset_hash,
    };
    render_model_pages(
        &render_env,
        &site_model,
        &mut cache,
        &mut current_page_outputs,
        &mut timer,
    )?;

    if matches!(opts.mode, BuildMode::Public | BuildMode::Full) {
        timer.phase("write_assets");
        artifacts.style_asset.write(output_dir)?;
    }

    if matches!(opts.mode, BuildMode::Full) {
        write_asset_headers(output_dir)?;
    }

    if matches!(opts.mode, BuildMode::Public | BuildMode::Full) {
        rewrite_404_stylesheet(output_dir, &artifacts.style_asset)?;
    }

    timer.phase("generate_feeds");
    let feed_items = collect_feed_items(&collections, &site_model.all_items);
    let rss = crate::rss::generate(config, &feed_items);
    std::fs::write(output_dir.join("rss.xml"), rss)?;

    let sitemap_files = crate::sitemap::generate(config, &site_model.pages);
    for (filename, content) in &sitemap_files {
        let sitemap_path = output_dir.join(filename);
        std::fs::write(&sitemap_path, content)?;
        current_page_outputs.insert(sitemap_path);
    }

    let output_count = current_page_outputs.len();
    if let Some(cache) = cache.as_mut() {
        let previous_outputs = cache.page_outputs().clone();
        prune_removed_outputs(output_dir, &previous_outputs, &current_page_outputs)?;
        cache.replace_page_outputs(current_page_outputs);
    }

    std::fs::write(
        output_dir.join("robots.txt"),
        format!(
            "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
            config.site.base_url
        ),
    )?;

    let mut manifest = crate::BuildManifest {
        config_hash: config_hash.clone(),
        ..Default::default()
    };
    record_manifest_entries(
        &mut manifest,
        &site_model,
        &artifacts.style_asset,
        &sitemap_files,
        &artifacts.engine,
        &config.paginate_path,
    );
    manifest.save(output_dir)?;

    timer.finish();
    let date_ordered_count = site_model
        .all_items
        .iter()
        .filter(|i| i.year.is_some())
        .count();

    if opts.emit_report {
        if date_ordered_count == 0 {
            diagnostics.push(
                crate::Diagnostic::warning(
                    std::path::PathBuf::from(&config.paths.content),
                    "no date-ordered items found".into(),
                )
                .with_hint("add a `date` field to frontmatter to enable feeds and archives"),
            );
        }
        timer.print_report(site_model.all_items.len(), date_ordered_count, output_count);
    }

    diagnostics.emit_all();
    crate::print_build_summary(&diagnostics);

    if opts.profile {
        if let Some(cache) = cache.as_ref() {
            let (hits, misses) = cache.cache_stats();
            timer.set_cache_stats(hits, misses);
        }
        timer.print_profile_report();
    }

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
    output_dir: &'a Path,
    asset_manifest: &'a crate::AssetManifest,
    config_hash: &'a str,
    asset_hash: &'a str,
}

/// Computes the 3-level render hash for a page (content + template + config).
fn page_render_key(
    env: &RenderEnv<'_>,
    page: &model::Page,
    site_model: &model::SiteModel,
) -> (String, String) {
    // (content_hash, render_hash)
    let content_hash = page
        .content_item
        .as_ref()
        .map(|item| item.content_hash.clone())
        .unwrap_or_else(|| generic_page_content_hash(page, site_model));
    let template_deps = page_template_deps(env.engine, page, site_model, &env.config.paginate_path);
    let template_hash = template_deps_hash(env.engine, &template_deps);
    let render_hash = BuildCache::build_render_hash(
        &content_hash,
        &template_hash,
        env.config_hash,
        env.asset_hash,
    );
    (content_hash, render_hash)
}

fn render_model_pages(
    env: &RenderEnv<'_>,
    site_model: &model::SiteModel,
    cache: &mut Option<&mut BuildCache>,
    current_page_outputs: &mut HashSet<PathBuf>,
    timer: &mut BuildTimer,
) -> anyhow::Result<()> {
    use std::sync::Arc;

    struct PageTask {
        index: usize,
        page: model::Page,
    }

    let mut cached_results: Vec<(usize, String)> = Vec::new();
    let mut tasks: Vec<PageTask> = Vec::new();

    // Phase 1: pre-check cache, split into hits and misses
    for (i, page) in site_model.pages.iter().enumerate() {
        let output_path = env.output_dir.join(&page.output_path);
        current_page_outputs.insert(output_path);

        let (_, render_hash) = page_render_key(env, page, site_model);
        let cached = cache.as_mut().and_then(|c| {
            if page.kind == model::PageKind::Single {
                page.content_item
                    .as_ref()
                    .and_then(|item| c.cached_render(&item.source_path, &render_hash))
            } else {
                c.cached_generic_render(&page.url, &render_hash)
            }
            .map(|html| html.to_string())
        });

        if let Some(html) = cached {
            cached_results.push((i, html));
        } else {
            tasks.push(PageTask {
                index: i,
                page: page.clone(),
            });
        }
    }

    // Phase 2: write cached results
    for (i, html) in &cached_results {
        let page = &site_model.pages[*i];
        let output_path = env.output_dir.join(&page.output_path);
        write_page_if_changed(&output_path, html)?;
        timer.end_page(false);
    }

    if tasks.is_empty() {
        return Ok(());
    }

    // Phase 3: parallel render
    let template_sources = env.engine.shared_template_sources();
    let config = Arc::new(env.config.clone());
    let style_href = Arc::new(env.style_asset.href.clone());
    let style_content = Arc::new(env.style_asset.content.clone());
    let style_output_path = Arc::new(env.style_asset.output_path.clone());
    let config_hash = Arc::new(env.config_hash.to_string());
    let asset_manifest = Arc::new(env.asset_manifest.clone());
    let shared_site_model = Arc::new(site_model.clone());

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let parallel_start = std::time::Instant::now();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(threads)
        .build()?;

    let results: Vec<(usize, anyhow::Result<String>)> = rt.block_on(async {
        let mut handles = Vec::with_capacity(tasks.len());

        for task in tasks {
            let templates = template_sources.clone();
            let cfg = config.clone();
            let href = style_href.clone();
            let s_content = style_content.clone();
            let s_output = style_output_path.clone();
            let c_hash = config_hash.clone();
            let a_manifest = asset_manifest.clone();
            let sm = shared_site_model.clone();
            let idx = task.index;

            handles.push(tokio::task::spawn_blocking(move || {
                let mut tera = tera::Tera::default();
                for (name, source) in templates.iter() {
                    if let Err(e) = tera.add_raw_template(name, source) {
                        return (
                            idx,
                            Err(anyhow::anyhow!("Failed to add template {}: {}", name, e)),
                        );
                    }
                }
                let asset_mappings: std::sync::Arc<
                    std::sync::RwLock<std::collections::HashMap<String, String>>,
                > = std::sync::Arc::new(std::sync::RwLock::new(a_manifest.mappings.clone()));
                let _ = crate::engine::register_asset_url_fn(&mut tera, asset_mappings);
                let engine = crate::engine::Engine::init_tera_only(tera);

                let style_asset = StyleAsset {
                    href: (*href).clone(),
                    output_path: (*s_output).clone(),
                    content: s_content.to_vec(),
                };
                let manifest: crate::AssetManifest = (*a_manifest).clone();
                let asset_hash = manifest.content_hash();
                let render_env = RenderEnv {
                    engine: &engine,
                    config: &cfg,
                    style_asset: &style_asset,
                    output_dir: std::path::Path::new(""),
                    asset_manifest: &manifest,
                    config_hash: &c_hash,
                    asset_hash: &asset_hash,
                };

                match render_one_page(&render_env, &sm, &task.page) {
                    Ok(html) => (idx, Ok(html)),
                    Err(e) => (idx, Err(e)),
                }
            }));
        }

        let mut results: Vec<(usize, anyhow::Result<String>)> = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok((i, result)) => results.push((i, result)),
                Err(e) => results.push((0, Err(anyhow::anyhow!("task panicked: {}", e)))),
            }
        }
        results
    });

    let wall_time = parallel_start.elapsed().as_millis();
    timer.set_parallel_stats(threads, wall_time);

    // Phase 4: write results in order, update cache
    results
        .into_iter()
        .try_for_each(|(idx, result)| -> anyhow::Result<()> {
            let html = result?;
            let page = &site_model.pages[idx];
            let output_path = env.output_dir.join(&page.output_path);
            write_page_if_changed(&output_path, &html)?;
            timer.end_page(true);

            if let Some(cache) = cache.as_mut() {
                let (_, render_hash) = page_render_key(env, page, site_model);
                if page.kind == model::PageKind::Single {
                    if let Some(item) = &page.content_item {
                        cache.store_render(&item.source_path, render_hash, html);
                    }
                } else {
                    cache.store_generic_render(page.url.clone(), render_hash, html);
                }
            }
            Ok(())
        })?;

    Ok(())
}

/// Render a single page (used in parallel tasks that have their own Engine).
fn render_one_page(
    env: &RenderEnv<'_>,
    site_model: &model::SiteModel,
    page: &model::Page,
) -> anyhow::Result<String> {
    match page.kind {
        model::PageKind::Single => {
            let item = page
                .content_item
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("single page missing content item: {}", page.url))?;
            let is_article = item.raw_date.is_some();
            render_single(env, item, &page.template, is_article)
        }
        model::PageKind::Home => render_home_page(env, site_model, None),
        model::PageKind::Section => render_section_page(env, site_model, page, &page.url, None),
        model::PageKind::TaxonomyIndex => render_taxonomy_index_page(env, site_model, page),
        model::PageKind::Term => render_term_page(env, site_model, page, &page.url, None),
        model::PageKind::NotFound => render_not_found_page(env),
        model::PageKind::Paginate => render_paginate_page(env, site_model, page),
    }
}

/// Compute a content hash for non-Single pages based on their input data.
fn generic_page_content_hash(page: &model::Page, site_model: &model::SiteModel) -> String {
    let mut parts: Vec<String> = Vec::new();
    match page.kind {
        model::PageKind::Home => {
            let mut hashes: Vec<&str> = site_model
                .all_items
                .iter()
                .map(|i| i.content_hash.as_str())
                .collect();
            hashes.sort();
            parts.extend(hashes.iter().map(|s| s.to_string()));
        }
        model::PageKind::Section => {
            let mut hashes: Vec<&str> = site_model
                .all_items
                .iter()
                .filter(|i| crate::model::url_is_under_section(&i.url, &page.url))
                .map(|i| i.content_hash.as_str())
                .collect();
            hashes.sort();
            parts.extend(hashes.iter().map(|s| s.to_string()));
            parts.push(page.url.clone());
        }
        model::PageKind::TaxonomyIndex | model::PageKind::Term => {
            parts.push(page.title.clone());
            let mut hashes: Vec<&str> = site_model
                .all_items
                .iter()
                .map(|i| i.content_hash.as_str())
                .collect();
            hashes.sort();
            parts.extend(hashes.iter().map(|s| s.to_string()));
        }
        model::PageKind::Paginate => {
            let mut hashes: Vec<&str> = site_model
                .all_items
                .iter()
                .map(|i| i.content_hash.as_str())
                .collect();
            hashes.sort();
            parts.extend(hashes.iter().map(|s| s.to_string()));
            parts.push(page.url.clone());
        }
        model::PageKind::NotFound => {
            parts.push("404".into());
        }
        _ => {
            parts.push(page.url.clone());
        }
    }
    if parts.is_empty() {
        return String::new();
    }
    crate::content::fingerprint(parts.join("\0").as_bytes())
}

fn render_single(
    env: &RenderEnv<'_>,
    item: &ContentItem,
    template: &str,
    is_article: bool,
) -> anyhow::Result<String> {
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset, env.asset_manifest);
    ctx.insert("page", &make_item_context(item));
    let body = env.engine.render(template, &ctx)?;
    let body = if !item.shortcodes.is_empty() {
        crate::shortcode::postprocess(&body, &item.shortcodes, env.engine)
            .with_context(|| format!("Failed to process shortcodes for {:?}", item.source_path))?
    } else {
        body
    };
    let dir_path = item.url.trim_start_matches('/').trim_end_matches('/');
    wrap_with_layout(
        env,
        &item.title,
        &item.description,
        &body,
        dir_path,
        is_article,
    )
}

fn render_home_page(
    env: &RenderEnv<'_>,
    site_model: &model::SiteModel,
    paginator: Option<&crate::paginator::Paginator>,
) -> anyhow::Result<String> {
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset, env.asset_manifest);

    let featured: Vec<&ContentItem> = site_model
        .all_items
        .iter()
        .filter(|i| i.featured)
        .take(6)
        .collect();
    ctx.insert(
        "featured_posts",
        &featured
            .iter()
            .map(|i| make_item_base(i))
            .collect::<Vec<_>>(),
    );

    let archive_items: Vec<&ContentItem> = site_model
        .all_items
        .iter()
        .filter(|i| i.year.is_some())
        .collect();
    let paginator = paginator
        .cloned()
        .or_else(|| first_page_paginator_for_home(env, &archive_items));
    let archive_items = paginate_items(&archive_items, paginator.as_ref());
    ctx.insert("archive", &make_archive(&archive_items));
    if let Some(p) = paginator.as_ref() {
        ctx.insert("paginator", p);
    }

    let template = env.engine.resolve_template(&model::PageKind::Home, None);
    let body = env.engine.render(&template, &ctx)?;
    let title = paginator.as_ref().map_or(String::new(), |p| {
        if p.current_index <= 1 {
            String::new()
        } else {
            format!("Page {}", p.current_index)
        }
    });
    wrap_with_layout(env, &title, &env.config.site.description, &body, "", false)
}

fn render_section_page(
    env: &RenderEnv<'_>,
    site_model: &model::SiteModel,
    page: &model::Page,
    base_url: &str,
    paginator: Option<&crate::paginator::Paginator>,
) -> anyhow::Result<String> {
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset, env.asset_manifest);

    let section = site_model.sections.values().find(|s| s.url == base_url);
    let items = section_items(site_model, base_url);
    let paginator = paginator
        .cloned()
        .or_else(|| first_page_paginator(env, &items, base_url));
    let items = paginate_items(&items, paginator.as_ref());

    ctx.insert(
        "section",
        &section_context(section, &items, &site_model.sections),
    );
    if let Some(p) = paginator.as_ref() {
        ctx.insert("paginator", p);
    }

    let collection = section.map(|s| s.collection.as_str());
    let template = env
        .engine
        .resolve_template(&model::PageKind::Section, collection);
    let body = env.engine.render(&template, &ctx)?;
    let dir_path = page.url.trim_start_matches('/').trim_end_matches('/');
    wrap_with_layout(env, &page.title, &page.description, &body, dir_path, false)
}

fn render_taxonomy_index_page(
    env: &RenderEnv<'_>,
    site_model: &model::SiteModel,
    page: &model::Page,
) -> anyhow::Result<String> {
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset, env.asset_manifest);

    for taxonomy in site_model.taxonomies.values() {
        if format!("/{}/", taxonomy.slug) == page.url {
            ctx.insert(
                "taxonomy",
                &serde_json::json!({
                    "name": taxonomy.name,
                    "slug": taxonomy.slug,
                    "terms": taxonomy.terms.iter().map(|t| serde_json::json!({
                        "name": t.name, "slug": t.slug, "url": t.url,
                    })).collect::<Vec<_>>(),
                }),
            );
            break;
        }
    }

    let template = env
        .engine
        .resolve_template(&model::PageKind::TaxonomyIndex, None);
    let body = env.engine.render(&template, &ctx)?;
    let dir_path = page.url.trim_start_matches('/').trim_end_matches('/');
    wrap_with_layout(env, &page.title, &page.description, &body, dir_path, false)
}

fn render_term_page(
    env: &RenderEnv<'_>,
    site_model: &model::SiteModel,
    page: &model::Page,
    base_url: &str,
    paginator: Option<&crate::paginator::Paginator>,
) -> anyhow::Result<String> {
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset, env.asset_manifest);

    for taxonomy in site_model.taxonomies.values() {
        if let Some(term) = taxonomy.terms.iter().find(|t| t.url == base_url) {
            let items: Vec<serde_json::Value> = site_model
                .all_items
                .iter()
                .filter(|item| {
                    item.taxonomy_terms
                        .get(&taxonomy.name)
                        .map(|terms| {
                            terms
                                .iter()
                                .any(|tag| crate::content::slugify(tag) == term.slug)
                        })
                        .unwrap_or(false)
                })
                .map(make_item_base)
                .collect();
            let paginator = paginator
                .cloned()
                .or_else(|| first_page_paginator(env, &items, base_url));
            let items = paginate_items(&items, paginator.as_ref());

            ctx.insert(
                "term",
                &serde_json::json!({
                    "name": term.name,
                    "slug": term.slug,
                    "url": term.url,
                    "taxonomy": taxonomy.name,
                    "pages": items,
                }),
            );
            if let Some(p) = paginator.as_ref() {
                ctx.insert("paginator", p);
            }
            break;
        }
    }

    let taxonomy_slug = base_url
        .trim_matches('/')
        .split('/')
        .next()
        .map(|s| s.to_string());
    let template = effective_term_template(env.engine, &page.template, taxonomy_slug.as_deref());
    let body = env.engine.render(&template, &ctx)?;
    let dir_path = page.url.trim_start_matches('/').trim_end_matches('/');
    wrap_with_layout(env, &page.title, &page.description, &body, dir_path, false)
}

fn render_paginate_page(
    env: &RenderEnv<'_>,
    site_model: &model::SiteModel,
    page: &model::Page,
) -> anyhow::Result<String> {
    let base_url = derive_paginate_base(&page.url, &env.config.paginate_path);

    if base_url == "/" {
        let date_ordered: Vec<&ContentItem> = site_model
            .all_items
            .iter()
            .filter(|i| i.year.is_some())
            .collect();
        let paginator = build_paginator_for_url(
            &date_ordered,
            env.config.paginate_by,
            &base_url,
            &page.url,
            &env.config.paginate_path,
        );
        render_home_page(env, site_model, paginator.as_ref())
    } else if site_model.sections.values().any(|s| s.url == base_url) {
        let paginator = build_paginator_for_url(
            &section_items(site_model, &base_url),
            env.config.paginate_by,
            &base_url,
            &page.url,
            &env.config.paginate_path,
        );
        render_section_page(env, site_model, page, &base_url, paginator.as_ref())
    } else {
        // Term pagination — find matching term
        let term = site_model.taxonomies.values().find_map(|taxonomy| {
            taxonomy
                .terms
                .iter()
                .find(|t| t.url == base_url)
                .map(|term| (taxonomy.name.clone(), term))
        });
        if let Some((taxonomy_name, term)) = term {
            let term_items: Vec<&ContentItem> = site_model
                .all_items
                .iter()
                .filter(|item| {
                    item.taxonomy_terms
                        .get(&taxonomy_name)
                        .map(|terms| {
                            terms
                                .iter()
                                .any(|tag| crate::content::slugify(tag) == term.slug)
                        })
                        .unwrap_or(false)
                })
                .collect();
            let paginator = build_paginator_for_url(
                &term_items,
                env.config.paginate_by,
                &base_url,
                &page.url,
                &env.config.paginate_path,
            );
            return render_term_page(env, site_model, page, &base_url, paginator.as_ref());
        }
        anyhow::bail!("unknown paginate base URL: {}", base_url)
    }
}

fn derive_paginate_base(url: &str, paginate_path: &str) -> String {
    let trimmed = url.trim_matches('/');
    let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return url.to_string();
    }
    let page_segment = segments[segments.len() - 1];
    let paginate_segment = segments[segments.len() - 2];
    if paginate_segment != paginate_path || !page_segment.chars().all(|c| c.is_ascii_digit()) {
        return url.to_string();
    }

    let base = segments[..segments.len() - 2].join("/");
    if base.is_empty() {
        return "/".into();
    }
    format!("/{}/", base)
}

fn build_paginator_for_url<T>(
    all_items: &[T],
    per_page: usize,
    base_url: &str,
    current_url: &str,
    paginate_path: &str,
) -> Option<crate::paginator::Paginator> {
    if per_page == 0 || all_items.is_empty() {
        return None;
    }
    let paginators = crate::paginator::paginate(all_items.len(), per_page, base_url, paginate_path);
    paginators
        .into_iter()
        .find(|p| p.current_url == current_url)
}

fn first_page_paginator_for_home(
    env: &RenderEnv<'_>,
    items: &[&ContentItem],
) -> Option<crate::paginator::Paginator> {
    build_paginator_for_url(
        items,
        env.config.paginate_by,
        "/",
        "/",
        &env.config.paginate_path,
    )
}

fn first_page_paginator(
    env: &RenderEnv<'_>,
    items: &[serde_json::Value],
    base_url: &str,
) -> Option<crate::paginator::Paginator> {
    build_paginator_for_url(
        items,
        env.config.paginate_by,
        base_url,
        base_url,
        &env.config.paginate_path,
    )
}

fn render_not_found_page(env: &RenderEnv<'_>) -> anyhow::Result<String> {
    let mut ctx = tera::Context::new();
    insert_template_context(&mut ctx, env.config, env.style_asset, env.asset_manifest);
    let body = env.engine.render("404.html", &ctx)?;
    wrap_with_layout(env, "Page Not Found", "", &body, "", false)
}

fn section_items(site_model: &model::SiteModel, section_url: &str) -> Vec<serde_json::Value> {
    site_model
        .all_items
        .iter()
        .filter(|item| model::url_is_under_section(&item.url, section_url))
        .map(make_item_base)
        .collect()
}

fn paginate_items<T: Clone>(
    items: &[T],
    paginator: Option<&crate::paginator::Paginator>,
) -> Vec<T> {
    let Some(paginator) = paginator else {
        return items.to_vec();
    };
    slice_for_page(items, paginator)
}

fn slice_for_page<T: Clone>(items: &[T], paginator: &crate::paginator::Paginator) -> Vec<T> {
    if items.is_empty() {
        return Vec::new();
    }
    let start = paginator
        .current_index
        .saturating_sub(1)
        .saturating_mul(paginator.per_page);
    let start = start.min(items.len());
    let end = (start + paginator.per_page).min(items.len());
    items[start..end].to_vec()
}

fn section_context(
    section: Option<&model::Section>,
    items: &[serde_json::Value],
    all_sections: &std::collections::HashMap<String, model::Section>,
) -> serde_json::Value {
    let sec = match section {
        Some(s) => s,
        None => {
            return serde_json::json!({
                "title": "",
                "pages": items,
                "breadcrumb": [],
            })
        }
    };

    let mut children_sections: Vec<&model::Section> = sec
        .children_slugs
        .iter()
        .filter_map(|key| all_sections.get(key))
        .collect();
    children_sections.sort_by(|a, b| {
        a.weight
            .cmp(&b.weight)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.url.cmp(&b.url))
    });

    let children: Vec<serde_json::Value> = children_sections
        .into_iter()
        .map(|child| {
            serde_json::json!({
                "title": child.title,
                "slug": child.slug,
                "url": child.url,
            })
        })
        .collect();

    let parent = sec
        .parent_slug
        .as_ref()
        .and_then(|key| all_sections.get(key))
        .map(|ps| {
            serde_json::json!({
                "title": ps.title,
                "slug": ps.slug,
                "url": ps.url,
            })
        });

    serde_json::json!({
        "title": sec.title,
        "slug": sec.slug,
        "url": sec.url,
        "pages": items,
        "breadcrumb": sec.breadcrumb,
        "parent": parent,
        "children": children,
    })
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

fn record_manifest_entries(
    manifest: &mut crate::BuildManifest,
    site_model: &model::SiteModel,
    style_asset: &StyleAsset,
    sitemap_files: &[(String, String)],
    engine: &Engine,
    paginate_path: &str,
) {
    for page in &site_model.pages {
        let source = page
            .source_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(&page.output_path));
        let content_hash = page
            .content_item
            .as_ref()
            .map(|item| item.content_hash.clone())
            .unwrap_or_else(|| page.output_path.to_string_lossy().to_string());
        let template_deps = page_template_deps(engine, page, site_model, paginate_path);
        let template_hash = template_deps_hash(engine, &template_deps);
        manifest.record(
            source,
            vec![page.output_path.clone()],
            content_hash,
            template_deps,
            template_hash,
        );
    }

    manifest.record(
        PathBuf::from("assets/styles.css"),
        vec![PathBuf::from(&style_asset.output_path)],
        style_asset.output_path.clone(),
        Vec::new(),
        String::new(),
    );
    manifest.record(
        PathBuf::from("generated/rss.xml"),
        vec![PathBuf::from("rss.xml")],
        "generated".into(),
        Vec::new(),
        String::new(),
    );
    manifest.record(
        PathBuf::from("generated/robots.txt"),
        vec![PathBuf::from("robots.txt")],
        "generated".into(),
        Vec::new(),
        String::new(),
    );

    for (filename, _) in sitemap_files {
        manifest.record(
            PathBuf::from(format!("generated/{}", filename)),
            vec![PathBuf::from(filename)],
            "generated".into(),
            Vec::new(),
            String::new(),
        );
    }
}

/// Returns the full template dependency chain for a page, including
/// the transitive deps of both the page template and layout.html.
/// Unified term template selection: matches the logic in `render_term_page`.
/// If `page_template` is non-empty and exists, use it; otherwise resolve via `PageKind::Term`.
fn effective_term_template(
    engine: &Engine,
    page_template: &str,
    taxonomy_slug: Option<&str>,
) -> String {
    if !page_template.is_empty() && engine.template_exists(page_template) {
        page_template.to_string()
    } else {
        engine.resolve_template(&model::PageKind::Term, taxonomy_slug)
    }
}

/// Returns the actual template used for rendering this page,
/// matching the resolve_template() logic in the render functions.
fn effective_template_for_page(
    engine: &Engine,
    page: &model::Page,
    site_model: &model::SiteModel,
    paginate_path: &str,
) -> String {
    match page.kind {
        model::PageKind::Single | model::PageKind::Home | model::PageKind::NotFound => {
            page.template.clone()
        }
        model::PageKind::Paginate => {
            let base_url = derive_paginate_base(&page.url, paginate_path);
            if base_url == "/" {
                engine.resolve_template(&model::PageKind::Home, None)
            } else if site_model.sections.values().any(|s| s.url == base_url) {
                let collection = site_model
                    .sections
                    .values()
                    .find(|s| s.url == base_url)
                    .map(|s| s.collection.as_str());
                engine.resolve_template(&model::PageKind::Section, collection)
            } else {
                let tax_slug = site_model.taxonomies.values().find_map(|t| {
                    t.terms
                        .iter()
                        .find(|term| term.url == base_url)
                        .map(|_| t.slug.as_str())
                });
                effective_term_template(engine, &page.template, tax_slug)
            }
        }
        model::PageKind::Section => {
            let collection = site_model
                .sections
                .values()
                .find(|s| s.url == page.url)
                .map(|s| s.collection.as_str());
            engine.resolve_template(&page.kind, collection)
        }
        model::PageKind::TaxonomyIndex => engine.resolve_template(&page.kind, None),
        model::PageKind::Term => {
            let tax_slug = site_model.taxonomies.values().find_map(|t| {
                t.terms
                    .iter()
                    .find(|term| term.url == page.url)
                    .map(|_| t.slug.as_str())
            });
            effective_term_template(engine, &page.template, tax_slug)
        }
    }
}

fn page_template_deps(
    engine: &Engine,
    page: &model::Page,
    site_model: &model::SiteModel,
    paginate_path: &str,
) -> Vec<String> {
    let effective = effective_template_for_page(engine, page, site_model, paginate_path);
    let mut deps: Vec<String> = engine.template_deps(&effective);
    // All pages go through wrap_with_layout → layout.html
    if effective != "layout.html" {
        for dep in engine.template_deps("layout.html") {
            deps.push(dep);
        }
    }
    // Shortcode templates used by this page
    if let Some(item) = &page.content_item {
        for sc in &item.shortcodes {
            let sc_tpl = format!("shortcodes/{}.html", sc.name);
            if engine.template_exists(&sc_tpl) {
                deps.push(sc_tpl.clone());
                for dep in engine.template_deps(&sc_tpl) {
                    deps.push(dep);
                }
            }
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

/// Computes a hash of the combined template source files for the given deps.
fn template_deps_hash(engine: &Engine, deps: &[String]) -> String {
    let mut parts = Vec::with_capacity(deps.len());
    for dep in deps {
        if let Some(source) = engine.template_source(dep) {
            parts.push(source);
        }
    }
    let combined = parts.join("\0");
    if combined.is_empty() {
        return String::new();
    }
    crate::content::fingerprint(combined.as_bytes())
}

fn build_config_hash(config: &SiteConfig) -> String {
    let mut parts = vec![
        config.site.title.clone(),
        config.site.subtitle.clone(),
        config.site.description.clone(),
        config.site.language.clone(),
        config.site.base_url.clone(),
        config.paginate_by.to_string(),
        config.paginate_path.clone(),
    ];

    for collection in &config.collections {
        parts.push(collection.name.clone());
        parts.push(collection.directory.clone());
        parts.push(collection.route.clone());
        parts.push(collection.template.clone());
    }

    for taxonomy in &config.taxonomies {
        parts.push(taxonomy.name.clone());
        parts.push(taxonomy.slug.clone());
        parts.push(taxonomy.template.clone());
    }

    content::fingerprint(parts.join("|").as_bytes())
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
    insert_template_context(&mut ctx, env.config, env.style_asset, env.asset_manifest);
    ctx.insert("title", title);
    ctx.insert("description", description);
    ctx.insert("body", body);
    ctx.insert("path", path);
    ctx.insert("og_type", og_type);
    env.engine.render("layout.html", &ctx)
}

fn insert_template_context(
    ctx: &mut tera::Context,
    config: &SiteConfig,
    style_asset: &StyleAsset,
    asset_manifest: &crate::AssetManifest,
) {
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

    let menus = if config.menus.is_empty() {
        None
    } else {
        let menus: serde_json::Map<String, serde_json::Value> = config
            .menus
            .iter()
            .map(|(name, items)| {
                let mut sorted = items.clone();
                sorted.sort_by_key(|item| item.weight);
                let entries: Vec<serde_json::Value> = sorted
                    .iter()
                    .map(|item| {
                        serde_json::json!({
                            "name": item.name,
                            "url": item.url,
                            "weight": item.weight,
                        })
                    })
                    .collect();
                (name.clone(), serde_json::Value::Array(entries))
            })
            .collect();
        Some(serde_json::Value::Object(menus))
    };

    ctx.insert("site", &site);
    ctx.insert("theme", &theme);
    ctx.insert(
        "config",
        &merge_config_context(&site, &theme, menus.as_ref()),
    );
    if let Some(menus) = menus {
        ctx.insert("menus", &menus);
    }
    // Inject asset manifest for template access: {{ asset_manifest["style.css"] | safe }}
    ctx.insert(
        "asset_manifest",
        &serde_json::to_value(&asset_manifest.mappings)
            .expect("HashMap<String, String> -> serde_json::Value is infallible"),
    );
}

fn merge_config_context(
    site: &serde_json::Value,
    theme: &serde_json::Value,
    menus: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut merged = serde_json::Map::new();

    if let Some(site_obj) = site.as_object() {
        for (key, value) in site_obj {
            merged.insert(key.clone(), value.clone());
        }
    }

    if let Some(theme_obj) = theme.as_object() {
        for (key, value) in theme_obj {
            merged.insert(key.clone(), value.clone());
        }
    }

    if let Some(menus) = menus {
        merged.insert("menus".into(), menus.clone());
    }

    serde_json::Value::Object(merged)
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
        obj.insert("taxonomies".into(), serde_json::json!(item.taxonomy_terms));
        obj.insert("type".into(), serde_json::json!(item.collection));
        obj.insert("toc".into(), serde_json::json!(item.headings));
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

pub struct StyleAsset {
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
    use super::{build, derive_paginate_base, prune_removed_outputs, section_context};
    use crate::config::{AuthorConfig, FeedConfig, PathsConfig, SiteConfig, SiteMeta};
    use crate::content::ContentItem;
    use crate::model::{BreadcrumbItem, Section};
    use std::collections::HashMap;
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
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
        };

        build(&config, &output, false, false).unwrap();

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

    /// Parallel rendering must produce the same output as a subsequent
    /// rebuild (cache-aware) — verifies determinism.
    #[test]
    fn parallel_render_is_deterministic() {
        let root = temp_dir("kiln-determinism-test");
        let content = root.join("content");
        let posts = content.join("posts");
        let styles = root.join("styles.css");
        let output = root.join("dist");

        std::fs::create_dir_all(&posts).unwrap();
        for i in 1..=5 {
            std::fs::write(
                posts.join(format!("2026-06-0{}-post-{}.md", i, i)),
                format!(
                    "---\ntitle: \"Post {}\"\ndate: \"2026-06-0{}\"\n---\n\nBody {}.\n",
                    i, i, i
                ),
            )
            .unwrap();
        }
        std::fs::write(&styles, "body {{ margin: 0; }}\n").unwrap();

        let config = SiteConfig {
            paths: PathsConfig {
                content: content.to_string_lossy().to_string(),
                templates: root.join("missing-templates").to_string_lossy().to_string(),
                public: root.join("missing-public").to_string_lossy().to_string(),
                styles: styles.to_string_lossy().to_string(),
            },
            site: SiteMeta {
                title: "Determinism Test".into(),
                subtitle: String::new(),
                description: String::new(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: None,
            feed: FeedConfig { item_count: 20 },
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
        };

        // Build twice with cache cleared between runs
        let collect_html_files = |output: &Path| -> std::collections::BTreeMap<String, String> {
            let mut files = std::collections::BTreeMap::new();
            let mut dirs = vec![output.to_path_buf()];
            while let Some(dir) = dirs.pop() {
                for entry in std::fs::read_dir(&dir).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.file_name().is_some_and(|n| n == ".DS_Store") {
                        continue;
                    }
                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.extension().is_some_and(|ext| ext == "html") {
                        let rel = path.strip_prefix(output).unwrap();
                        files.insert(
                            rel.to_string_lossy().to_string(),
                            std::fs::read_to_string(&path).unwrap(),
                        );
                    }
                }
            }
            files
        };

        crate::build(&config, &output, false, false).unwrap();
        let first = collect_html_files(&output);

        std::fs::remove_dir_all(&output).unwrap();
        crate::build(&config, &output, false, false).unwrap();
        let second = collect_html_files(&output);

        assert_eq!(first.len(), second.len());
        for (path, html) in &first {
            assert_eq!(
                second.get(path),
                Some(html),
                "HTML output for {} differs between builds",
                path
            );
        }

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
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
        };

        build(&config, &output, false, false).unwrap();

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
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
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

        let asset_manifest = crate::AssetManifest::default();
        let mut ctx = tera::Context::new();
        super::insert_template_context(&mut ctx, &config, &style_asset, &asset_manifest);

        let site = ctx.get("site").unwrap();
        let theme = ctx.get("theme").unwrap();

        assert_eq!(site["title"], "Test");
        assert_eq!(site["stylesheet_href"], "/assets/styles.abc.css");
        assert_eq!(theme["intro"], "Hello");
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
                taxonomy_terms: Default::default(),
                raw_date: chrono::NaiveDate::from_ymd_opt(2026, 5, 1),
                headings: vec![],
                shortcodes: vec![],
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
                taxonomy_terms: Default::default(),
                raw_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 1),
                headings: vec![],
                shortcodes: vec![],
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
                taxonomy_terms: Default::default(),
                raw_date: None,
                headings: vec![],
                shortcodes: vec![],
            },
        ];

        let feed_items = super::collect_feed_items(&collections, &items);
        assert_eq!(feed_items.len(), 2);
        assert_eq!(feed_items[0].title, "Newer");
        assert_eq!(feed_items[1].title, "Older");
    }

    #[test]
    fn derive_paginate_base_strips_only_trailing_paginate_segments() {
        assert_eq!(derive_paginate_base("/page/2/", "page"), "/");
        assert_eq!(
            derive_paginate_base("/docs/page/intro/page/2/", "page"),
            "/docs/page/intro/"
        );
        assert_eq!(
            derive_paginate_base("/docs/page/intro/", "page"),
            "/docs/page/intro/"
        );
    }

    #[test]
    fn section_context_sorts_children_by_weight() {
        let parent = Section {
            slug: "docs".into(),
            title: "Docs".into(),
            url: "/blog/docs/".into(),
            collection: "posts".into(),
            parent_slug: None,
            children_slugs: vec!["blog:docs/b".into(), "blog:docs/a".into()],
            weight: 0,
            breadcrumb: vec![BreadcrumbItem {
                title: "Docs".into(),
                url: "/blog/docs/".into(),
            }],
        };
        let child_a = Section {
            slug: "a".into(),
            title: "Alpha".into(),
            url: "/blog/docs/a/".into(),
            collection: "posts".into(),
            parent_slug: Some("blog:docs".into()),
            children_slugs: vec![],
            weight: 20,
            breadcrumb: vec![],
        };
        let child_b = Section {
            slug: "b".into(),
            title: "Beta".into(),
            url: "/blog/docs/b/".into(),
            collection: "posts".into(),
            parent_slug: Some("blog:docs".into()),
            children_slugs: vec![],
            weight: 10,
            breadcrumb: vec![],
        };
        let mut sections = HashMap::new();
        sections.insert("blog:docs".into(), parent.clone());
        sections.insert("blog:docs/a".into(), child_a);
        sections.insert("blog:docs/b".into(), child_b);

        let ctx = section_context(Some(&parent), &[], &sections);
        let children = ctx["children"].as_array().unwrap();
        assert_eq!(children[0]["title"], "Beta");
        assert_eq!(children[1]["title"], "Alpha");
    }

    #[test]
    fn prune_removed_outputs_deletes_stale_sitemap_shards() {
        let root = temp_dir("kiln-sitemap-prune");
        let output_dir = root.join("dist");
        std::fs::create_dir_all(&output_dir).unwrap();
        std::fs::write(output_dir.join("sitemap.xml"), "index").unwrap();
        std::fs::write(output_dir.join("sitemap-1.xml"), "chunk1").unwrap();
        std::fs::write(output_dir.join("sitemap-2.xml"), "chunk2").unwrap();

        let previous: std::collections::HashSet<PathBuf> = [
            output_dir.join("sitemap.xml"),
            output_dir.join("sitemap-1.xml"),
            output_dir.join("sitemap-2.xml"),
        ]
        .into_iter()
        .collect();
        let current: std::collections::HashSet<PathBuf> =
            [output_dir.join("sitemap.xml")].into_iter().collect();

        prune_removed_outputs(&output_dir, &previous, &current).unwrap();

        assert!(output_dir.join("sitemap.xml").exists());
        assert!(!output_dir.join("sitemap-1.xml").exists());
        assert!(!output_dir.join("sitemap-2.xml").exists());

        let _ = std::fs::remove_dir_all(root);
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
