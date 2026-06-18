use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

use notify::Watcher;
use tiny_http::{Header, Response, Server};

enum RebuildMode {
    Content,
    Public,
    Full { changed_templates: Vec<String> },
}

pub fn start(
    config_path: &std::path::Path,
    output_dir: &std::path::Path,
    port: u16,
) -> anyhow::Result<()> {
    let (config, base_dir) = crate::config::SiteConfig::load(config_path)?;
    let output_dir = crate::output_safety::ensure_safe_output_target(
        "serve",
        output_dir,
        config_path,
        &base_dir,
        &config,
    )?;
    let mut artifacts = crate::site::BuildArtifacts::load(&config)?;

    let mut prefixes = collection_prefixes(&config);
    let mut cache = crate::cache::BuildCache::new();

    // Initial build
    println!("Building site...");
    crate::site::build_with_artifacts(
        &config,
        &output_dir,
        Some(&mut cache),
        &artifacts,
        crate::site::BuildOptions {
            include_drafts: false,
            mode: crate::site::BuildMode::Full,
            emit_report: true,
            profile: false,
            profile_json: false,
        },
    )?;

    // Start file watcher in background
    let (tx, rx) = channel::<Vec<PathBuf>>();
    let watch_paths = vec![
        PathBuf::from(&config.paths.content),
        PathBuf::from(&config.paths.templates),
        PathBuf::from(&config.paths.public),
        PathBuf::from(&config.paths.styles),
        config_path.to_path_buf(),
    ];

    std::thread::spawn(move || {
        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event.paths);
                }
            })
            .expect("Failed to create file watcher");

        for path in &watch_paths {
            if path.exists() {
                let _ = watcher.watch(path, notify::RecursiveMode::Recursive);
            }
        }

        // Keep watcher alive
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    });

    // Start HTTP server
    let addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to start server on http://{}: {}", addr, e))?;

    println!("Serving on http://{}", addr);
    println!("Watching for file changes...");

    // Debounce: wait 100ms after last change before rebuild
    let mut pending_mode: Option<RebuildMode> = None;

    loop {
        // Check for file changes with a short timeout
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(paths) => {
                let mode = classify_rebuild(&paths, config_path, &config);
                pending_mode = Some(match (pending_mode.take(), mode) {
                    // Full always dominates; merge template lists
                    (
                        Some(RebuildMode::Full {
                            changed_templates: mut prev,
                        }),
                        RebuildMode::Full { changed_templates },
                    ) => {
                        // If either side has empty list → config/style change, full invalidation
                        if prev.is_empty() || changed_templates.is_empty() {
                            RebuildMode::Full {
                                changed_templates: vec![],
                            }
                        } else {
                            for t in changed_templates {
                                if !prev.contains(&t) {
                                    prev.push(t);
                                }
                            }
                            RebuildMode::Full {
                                changed_templates: prev,
                            }
                        }
                    }
                    (Some(prev @ RebuildMode::Full { .. }), _) => prev,
                    (_, next @ RebuildMode::Full { .. }) => next,
                    (Some(RebuildMode::Public), RebuildMode::Content)
                    | (Some(RebuildMode::Content), RebuildMode::Public)
                    | (Some(RebuildMode::Public), RebuildMode::Public) => RebuildMode::Public,
                    (Some(RebuildMode::Content), RebuildMode::Content) => RebuildMode::Content,
                    _ => RebuildMode::Content,
                });
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No new changes, rebuild if needed
                if let Some(mode) = pending_mode.take() {
                    println!("Changes detected, rebuilding...");
                    let rebuild = crate::config::SiteConfig::load(config_path).and_then(
                        |(config, base_dir)| {
                            let output_dir = crate::output_safety::ensure_safe_output_target(
                                "serve",
                                &output_dir,
                                config_path,
                                &base_dir,
                                &config,
                            )?;
                            match mode {
                                RebuildMode::Content => crate::site::build_with_artifacts(
                                    &config,
                                    &output_dir,
                                    Some(&mut cache),
                                    &artifacts,
                                    crate::site::BuildOptions {
                                        include_drafts: false,
                                        mode: crate::site::BuildMode::Content,
                                        emit_report: true,
                                        profile: false,
                                        profile_json: false,
                                    },
                                ),
                                RebuildMode::Public => crate::site::build_public_incremental(
                                    &config,
                                    &output_dir,
                                    false,
                                    &mut cache,
                                    &artifacts,
                                ),
                                RebuildMode::Full { changed_templates } => {
                                    // Reload artifacts first (templates may have changed)
                                    artifacts = crate::site::BuildArtifacts::load(&config)?;
                                    prefixes = collection_prefixes(&config);

                                    if changed_templates.is_empty() {
                                        // Config/style change → full rebuild
                                        cache.clear_renders();
                                    } else {
                                        // Template-only change → selective invalidation
                                        let manifest = crate::BuildManifest::load(&output_dir)
                                            .unwrap_or_default();
                                        let mut deps_map: std::collections::HashMap<
                                            std::path::PathBuf,
                                            Vec<String>,
                                        > = std::collections::HashMap::new();
                                        for entry in &manifest.entries {
                                            deps_map.insert(
                                                entry.source.clone(),
                                                entry.template_deps.clone(),
                                            );
                                        }
                                        for tpl in &changed_templates {
                                            cache.invalidate_by_template(tpl, &deps_map);
                                        }
                                    }

                                    // Build into a staging directory so that if
                                    // template rendering fails partway through, the
                                    // previous successful build remains intact.
                                    let staging = staging_dir(&output_dir);
                                    if staging.exists() {
                                        let _ = std::fs::remove_dir_all(&staging);
                                    }

                                    let build_result = crate::site::build_with_artifacts(
                                        &config,
                                        &staging,
                                        Some(&mut cache),
                                        &artifacts,
                                        crate::site::BuildOptions {
                                            include_drafts: false,
                                            mode: crate::site::BuildMode::Full,
                                            emit_report: true,
                                            profile: false,
                                            profile_json: false,
                                        },
                                    );

                                    if let Err(e) = build_result {
                                        // Clean up the failed staging build
                                        let _ = std::fs::remove_dir_all(&staging);
                                        return Err(e);
                                    }

                                    // Atomically swap the staged build into place.
                                    // The previous output is moved aside (not
                                    // deleted) and restored if the swap fails.
                                    // If output_dir is a symlink, resolve it to
                                    // the real target so siblings share a filesystem.
                                    let resolved = resolve_output_for_swap(&output_dir);
                                    commit_staged_build(&staging, &resolved)?;

                                    // Update cache output paths to point at the
                                    // real output directory
                                    cache.remap_outputs(&staging, &resolved);

                                    Ok(())
                                }
                            }
                        },
                    );
                    if let Err(e) = rebuild {
                        eprintln!("Build error: {:#}", e);
                        eprintln!("Continuing to serve the last successful build");
                    } else {
                        println!("Rebuild complete");
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // Handle one HTTP request (non-blocking)
        if let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(100)) {
            let url = request.url().to_string();
            let path = url
                .split('?')
                .next()
                .unwrap_or("")
                .trim_start_matches('/')
                .to_string();
            serve_file(request, &output_dir, &path, &prefixes);
        }
    }

    Ok(())
}

fn classify_rebuild(
    paths: &[PathBuf],
    config_path: &Path,
    config: &crate::config::SiteConfig,
) -> RebuildMode {
    if paths.is_empty() {
        return RebuildMode::Full {
            changed_templates: vec![],
        };
    }

    let template_root = Path::new(&config.paths.templates);
    let public_root = Path::new(&config.paths.public);
    let content_root = Path::new(&config.paths.content);
    let styles_path = Path::new(&config.paths.styles);

    let mut saw_content = false;
    let mut saw_public = false;
    let mut changed_templates: Vec<String> = Vec::new();

    for path in paths {
        // Config or styles change → full rebuild (no selective invalidation)
        if path == config_path || path.starts_with(styles_path) {
            return RebuildMode::Full {
                changed_templates: vec![],
            };
        }
        // Template change → track which templates changed
        if path.starts_with(template_root) {
            if let Ok(rel) = path.strip_prefix(template_root) {
                let name = rel.to_string_lossy().replace('\\', "/");
                changed_templates.push(name);
            }
            continue;
        }
        if path.starts_with(public_root) {
            saw_public = true;
        } else if path.starts_with(content_root) {
            saw_content = true;
        } else {
            // Unknown path → full rebuild
            return RebuildMode::Full {
                changed_templates: vec![],
            };
        }
    }

    // Template changes → selective invalidation instead of full rebuild
    if !changed_templates.is_empty() {
        if saw_content || saw_public {
            return RebuildMode::Full {
                changed_templates: vec![],
            };
        }
        return RebuildMode::Full { changed_templates };
    }

    if saw_public && saw_content {
        RebuildMode::Full {
            changed_templates: vec![],
        }
    } else if saw_public {
        RebuildMode::Public
    } else {
        RebuildMode::Content
    }
}

fn serve_file(
    request: tiny_http::Request,
    dist_dir: &std::path::Path,
    path: &str,
    prefixes: &[String],
) {
    if path.split('/').any(|part| part == "..") {
        let _ = request.respond(Response::from_string("404").with_status_code(404));
        return;
    }

    let (file_path, status) = resolve_file(dist_dir, path, prefixes);

    match std::fs::read(&file_path) {
        Ok(content) => {
            let mime = mime_type(&file_path);
            let header = Header::from_bytes("Content-Type", mime.as_bytes()).unwrap();
            let response = Response::from_data(content)
                .with_header(header)
                .with_status_code(status);
            let _ = request.respond(response);
        }
        Err(_) => {
            // Try 404 page
            if let Ok(content) = std::fs::read(dist_dir.join("404.html")) {
                let header =
                    Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
                let response = Response::from_data(content)
                    .with_header(header)
                    .with_status_code(404);
                let _ = request.respond(response);
            } else {
                let _ = request.respond(Response::from_string("404").with_status_code(404));
            }
        }
    }
}

/// Returns a staging directory path for an output directory.
/// The staging directory lives next to the output directory under a
/// hidden name (`.{name}.staging`) so that `std::fs::rename` works
/// — both paths share the same parent filesystem.
fn staging_dir(output_dir: &Path) -> PathBuf {
    let name = output_dir.file_name().unwrap_or_default().to_string_lossy();
    output_dir.with_file_name(format!(".{}.staging", name))
}

/// Path used to park the previous output while the new one is swapped in.
fn backup_dir(output_dir: &Path) -> PathBuf {
    let name = output_dir.file_name().unwrap_or_default().to_string_lossy();
    output_dir.with_file_name(format!(".{}.old", name))
}

/// Resolve a symlinked output path to its real location so that the
/// staging/backup siblings sit on the same filesystem as the actual
/// target (required for an atomic `rename`).
fn resolve_output_for_swap(output_dir: &Path) -> PathBuf {
    std::fs::canonicalize(output_dir).unwrap_or_else(|_| output_dir.to_path_buf())
}

/// Atomically replace `output_dir` with `staging`.
///
/// Uses the move-aside pattern: the old output is *renamed* aside (not
/// deleted) before the staging directory is renamed into place. This avoids
/// the window where `output_dir` is empty if the second rename fails — if the
/// swap fails, the old output is restored.
///
/// If `output_dir` is a symlink, it is resolved to its real target first so
/// that the staging/backup siblings share its filesystem (otherwise `rename`
/// would fail with `EXDEV`). Because all three paths are siblings under the
/// same parent directory, the renames are same-filesystem and atomic on POSIX.
fn commit_staged_build(staging: &Path, output_dir: &Path) -> std::io::Result<()> {
    let resolved = resolve_output_for_swap(output_dir);
    let backup = backup_dir(&resolved);
    // Clear any stale backup left by a previous crashed run.
    let _ = std::fs::remove_dir_all(&backup);

    if resolved.exists() {
        // Move the previous build aside instead of deleting it, so it can be
        // restored if the swap fails.
        std::fs::rename(&resolved, &backup)?;
    }

    // Move the new build into place. If this fails, restore the old one.
    if let Err(e) = std::fs::rename(staging, &resolved) {
        if backup.exists() {
            let _ = std::fs::rename(&backup, &resolved);
        }
        return Err(e);
    }

    // Best-effort cleanup of the parked previous build.
    let _ = std::fs::remove_dir_all(&backup);
    Ok(())
}

fn collection_prefixes(config: &crate::config::SiteConfig) -> Vec<String> {
    let collections = if config.collections.is_empty() {
        crate::config::default_collections()
    } else {
        config.collections.clone()
    };
    collections
        .iter()
        .filter_map(|c| {
            let route = c.route.trim_start_matches('/');
            let idx = route.find('{')?;
            let prefix = &route[..idx];
            let prefix = prefix.trim_end_matches('/');
            if prefix.is_empty() {
                None
            } else {
                Some(prefix.to_string())
            }
        })
        .collect()
}

fn resolve_file(dist_dir: &Path, path: &str, prefixes: &[String]) -> (PathBuf, u16) {
    if path.is_empty() {
        return (dist_dir.join("index.html"), 200);
    }

    let exact = dist_dir.join(path);
    if exact.is_file() {
        return (exact, 200);
    }

    let index = dist_dir.join(path).join("index.html");
    if index.exists() {
        return (index, 200);
    }

    for prefix in prefixes {
        let candidate = dist_dir.join(prefix).join(path).join("index.html");
        if candidate.exists() {
            return (candidate, 200);
        }
    }

    (dist_dir.join("404.html"), 404)
}

fn mime_type(path: &std::path::Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8".into(),
        Some("css") => "text/css".into(),
        Some("js") => "application/javascript".into(),
        Some("svg") => "image/svg+xml".into(),
        Some("png") => "image/png".into(),
        Some("jpg") | Some("jpeg") => "image/jpeg".into(),
        Some("xml") => "application/xml".into(),
        Some("json") => "application/json".into(),
        _ => "application/octet-stream".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        backup_dir, classify_rebuild, commit_staged_build, resolve_file, staging_dir,
        RebuildMode,
    };
    use crate::config::{PathsConfig, SiteConfig, SiteMeta};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolves_missing_routes_to_404_status() {
        let root = temp_dir("kiln-serve-test");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("404.html"), "not found").unwrap();

        let (path, status) = resolve_file(&root, "missing", &[]);
        assert_eq!(status, 404);
        assert_eq!(path, root.join("404.html"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_clean_urls_to_index_with_200_status() {
        let root = temp_dir("kiln-serve-test");
        std::fs::create_dir_all(root.join("about")).unwrap();
        std::fs::write(root.join("about/index.html"), "about").unwrap();

        let (path, status) = resolve_file(&root, "about", &[]);
        assert_eq!(status, 200);
        assert_eq!(path, root.join("about/index.html"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn classify_rebuild_escalates_mixed_public_and_content_changes_to_full() {
        let config = test_config();
        let paths = vec![
            PathBuf::from("/site/content/posts/hello.md"),
            PathBuf::from("/site/public/logo.svg"),
        ];

        assert!(matches!(
            classify_rebuild(&paths, Path::new("/site/site.config.toml"), &config),
            RebuildMode::Full { .. }
        ));
    }

    #[test]
    fn classify_rebuild_keeps_public_only_changes_as_public() {
        let config = test_config();
        let paths = vec![PathBuf::from("/site/public/logo.svg")];

        assert!(matches!(
            classify_rebuild(&paths, Path::new("/site/site.config.toml"), &config),
            RebuildMode::Public
        ));
    }

    #[test]
    fn classify_rebuild_template_change_returns_full_with_templates() {
        let config = test_config();
        let paths = vec![PathBuf::from("/site/templates/post.html")];

        let mode = classify_rebuild(&paths, Path::new("/site/site.config.toml"), &config);
        assert!(matches!(&mode, RebuildMode::Full { .. }));
        if let RebuildMode::Full { changed_templates } = &mode {
            assert_eq!(changed_templates, &vec!["post.html".to_string()]);
        }
    }

    #[test]
    fn staging_dir_is_a_hidden_sibling_of_output() {
        let output = Path::new("/srv/site/dist");
        assert_eq!(staging_dir(output), Path::new("/srv/site/.dist.staging"));
        assert_eq!(backup_dir(output), Path::new("/srv/site/.dist.old"));
    }

    #[test]
    fn commit_staged_build_swaps_new_output_into_place() {
        let root = temp_dir("kiln-commit-swap");
        let output = root.join("dist");
        let staging = staging_dir(&output);

        // Seed an existing successful build
        std::fs::create_dir_all(output.join("posts/old")).unwrap();
        std::fs::write(output.join("posts/old/index.html"), "old").unwrap();

        // Build the new output in staging
        std::fs::create_dir_all(staging.join("posts/new")).unwrap();
        std::fs::write(staging.join("posts/new/index.html"), "new").unwrap();

        commit_staged_build(&staging, &output).unwrap();

        // New content is live
        assert_eq!(
            std::fs::read_to_string(output.join("posts/new/index.html")).unwrap(),
            "new"
        );
        // Old content was replaced (Full rebuild deletes everything)
        assert!(!output.join("posts/old/index.html").exists());
        // No staging or backup dirs left behind
        assert!(!staging.exists());
        assert!(!backup_dir(&output).exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn commit_staged_build_restores_old_output_if_swap_fails() {
        let root = temp_dir("kiln-commit-restore");
        let output = root.join("dist");
        let staging = staging_dir(&output);

        // Seed an existing successful build
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(output.join("index.html"), "old-build").unwrap();

        // staging does not exist → the swap rename fails and commit must
        // restore the previous output.
        let result = commit_staged_build(&staging, &output);
        assert!(result.is_err());

        // Old output is intact
        assert_eq!(
            std::fs::read_to_string(output.join("index.html")).unwrap(),
            "old-build"
        );
        // No backup left behind
        assert!(!backup_dir(&output).exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn commit_staged_build_creates_output_when_none_exists() {
        let root = temp_dir("kiln-commit-first");
        let output = root.join("dist");
        let staging = staging_dir(&output);

        // No existing output — first successful rebuild
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("index.html"), "first").unwrap();

        commit_staged_build(&staging, &output).unwrap();

        assert!(output.is_dir());
        assert_eq!(
            std::fs::read_to_string(output.join("index.html")).unwrap(),
            "first"
        );
        assert!(!staging.exists());

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), now))
    }

    fn test_config() -> SiteConfig {
        SiteConfig {
            paths: PathsConfig {
                content: "/site/content".into(),
                templates: "/site/templates".into(),
                public: "/site/public".into(),
                styles: "/site/styles.css".into(),
            },
            site: SiteMeta {
                title: "Test".into(),
                subtitle: String::new(),
                description: String::new(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: None,
            feed: Default::default(),
            collections: vec![],
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: HashMap::new(),
            extra: toml::Value::Table(Default::default()),
        }
    }
}
