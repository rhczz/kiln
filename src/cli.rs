use clap::{Parser, Subcommand};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "kiln", about = "A lean static site compiler", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a minimal kiln site
    Init {
        /// Directory to create
        path: PathBuf,
        /// Site title to write into site.config.toml
        #[arg(long, default_value = "My kiln site")]
        title: String,
        /// Public base URL for generated feeds and sitemap
        #[arg(long, default_value = "https://example.com")]
        base_url: String,
    },
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
        /// Emit machine-readable build profile JSON
        #[arg(long, conflicts_with = "profile")]
        profile_json: bool,
    },
    /// Validate site config and content without building
    Check {
        /// Path to site.config.toml
        #[arg(long, default_value = "site/site.config.toml")]
        config: PathBuf,
    },
    /// Inspect project structure, config, content, templates, and assets
    Doctor {
        /// Path to site.config.toml
        #[arg(long, default_value = "site/site.config.toml")]
        config: PathBuf,
    },
    /// Remove generated output and/or kiln cache state
    Clean {
        /// Output directory to clean
        #[arg(long, default_value = "dist")]
        output: PathBuf,
        /// Clean only the .kiln state directory inside the output directory
        #[arg(long)]
        cache: bool,
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
        Command::Init {
            path,
            title,
            base_url,
        } => {
            init_site(&path, &title, &base_url)?;
        }
        Command::Build {
            config,
            output,
            drafts,
            profile,
            profile_json,
        } => {
            let (site_config, _base_dir) = crate::config::SiteConfig::load(&config)?;
            if profile_json {
                let artifacts = crate::site::BuildArtifacts::load(&site_config)?;
                let mut cache = crate::BuildCache::new();
                crate::site::build_with_artifacts(
                    &site_config,
                    &output,
                    Some(&mut cache),
                    &artifacts,
                    crate::site::BuildOptions {
                        include_drafts: drafts,
                        mode: crate::site::BuildMode::Full,
                        emit_report: false,
                        profile,
                        profile_json: true,
                    },
                )?;
            } else {
                crate::site::build(&site_config, &output, drafts, profile)?;
            }
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
                    profile_json: false,
                },
            )?;
            eprintln!("Check passed.");
        }
        Command::Doctor { config } => {
            doctor(&config)?;
        }
        Command::Clean { output, cache } => {
            clean(&output, cache)?;
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

fn init_site(path: &Path, title: &str, base_url: &str) -> anyhow::Result<()> {
    if path.exists() {
        if !path.is_dir() {
            anyhow::bail!(
                "Cannot initialize {:?}: path exists and is not a directory",
                path
            );
        }
        if path.read_dir()?.next().is_some() {
            anyhow::bail!("Cannot initialize {:?}: directory is not empty", path);
        }
    }

    std::fs::create_dir_all(path.join("content/posts"))?;
    std::fs::create_dir_all(path.join("templates"))?;
    std::fs::create_dir_all(path.join("public"))?;

    write_new_file(
        &path.join("site.config.toml"),
        &format!(
            r#"[site]
title = "{}"
description = "A tiny site baked by kiln"
base_url = "{}"

[author]
name = "Your Name"

[paths]
content = "content"
templates = "templates"
public = "public"
styles = "styles.css"
"#,
            escape_toml_string(title),
            escape_toml_string(base_url)
        ),
    )?;
    write_new_file(
        &path.join("content/posts/hello.md"),
        r#"---
title: "Hello from kiln"
date: "2026-01-01"
tags: ["intro"]
---

This is your first kiln post. Edit this file, then run:

```bash
kiln build --config site.config.toml --output dist
```
"#,
    )?;
    write_new_file(
        &path.join("styles.css"),
        r#"body {
  max-width: 760px;
  margin: 3rem auto;
  padding: 0 1.25rem;
  font-family: system-ui, sans-serif;
  line-height: 1.6;
}

a {
  color: #0f766e;
}
"#,
    )?;

    eprintln!("Initialized kiln site at {}", path.display());
    eprintln!(
        "Next: kiln build --config {} --output {}",
        path.join("site.config.toml").display(),
        path.join("dist").display()
    );
    Ok(())
}

fn write_new_file(path: &Path, content: &str) -> anyhow::Result<()> {
    if path.exists() {
        anyhow::bail!("Refusing to overwrite existing file {:?}", path);
    }
    std::fs::write(path, content)?;
    Ok(())
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn doctor(config_path: &Path) -> anyhow::Result<()> {
    let mut report = DoctorReport::default();
    if !config_path.is_file() {
        report.error(
            format!("config not found: {}", config_path.display()),
            "run `kiln init <dir>` or pass --config <path>",
        );
        report.emit();
        anyhow::bail!("Doctor found errors.");
    }

    let (site_config, _base_dir) = match crate::config::SiteConfig::load(config_path) {
        Ok(loaded) => loaded,
        Err(e) => {
            report.error(
                format!("config failed to load: {}", e),
                "fix site.config.toml",
            );
            report.emit();
            anyhow::bail!("Doctor found errors.");
        }
    };
    report.ok(format!("config loaded: {}", config_path.display()));

    if site_config.site.base_url.contains("example.com") {
        report.warning(
            "site.base_url still points at example.com",
            "set the final public URL before publishing feeds or sitemap",
        );
    } else if site_config.site.base_url.starts_with("http://") {
        report.warning(
            "site.base_url uses http://",
            "use https:// unless this is a local or private site",
        );
    } else {
        report.ok(format!(
            "base_url looks usable: {}",
            site_config.site.base_url
        ));
    }

    check_required_path(
        &mut report,
        "content",
        Path::new(&site_config.paths.content),
        true,
        "create content/posts or update [paths].content",
    );
    check_required_path(
        &mut report,
        "styles",
        Path::new(&site_config.paths.styles),
        false,
        "create styles.css or update [paths].styles",
    );
    check_optional_dir(
        &mut report,
        "templates",
        Path::new(&site_config.paths.templates),
        "kiln will use built-in default templates",
    );
    check_optional_dir(
        &mut report,
        "public",
        Path::new(&site_config.paths.public),
        "create it when you need static assets",
    );

    match crate::site::BuildArtifacts::load(&site_config) {
        Ok(_) => report.ok("templates and stylesheet parsed"),
        Err(e) => report.error(
            format!("template or stylesheet check failed: {}", e),
            "fix the referenced template/CSS file and run doctor again",
        ),
    }

    check_content_routes(&mut report, &site_config);
    check_public_assets(&mut report, Path::new(&site_config.paths.public));
    run_doctor_build(&mut report, &site_config);

    report.emit();
    if report.errors == 0 {
        eprintln!("Doctor passed with {} warning(s).", report.warnings);
        Ok(())
    } else {
        anyhow::bail!("Doctor found {} error(s).", report.errors)
    }
}

fn check_required_path(report: &mut DoctorReport, label: &str, path: &Path, dir: bool, hint: &str) {
    let exists = if dir { path.is_dir() } else { path.is_file() };
    if exists {
        report.ok(format!("{} found: {}", label, path.display()));
    } else {
        report.error(format!("{} missing: {}", label, path.display()), hint);
    }
}

fn check_optional_dir(report: &mut DoctorReport, label: &str, path: &Path, hint: &str) {
    if path.is_dir() {
        report.ok(format!("{} found: {}", label, path.display()));
    } else {
        report.warning(
            format!("{} directory not found: {}", label, path.display()),
            hint,
        );
    }
}

fn check_content_routes(report: &mut DoctorReport, config: &crate::config::SiteConfig) {
    let mut all_items = Vec::new();
    let mut item_count = 0usize;
    for collection in &config.collections {
        match crate::content::load_collection(&config.paths.content, collection, false) {
            Ok(items) => {
                item_count += items.len();
                all_items.extend(items);
            }
            Err(e) => report.error(
                format!("failed to load collection {:?}: {}", collection.name, e),
                "fix the content frontmatter and collection config",
            ),
        }
    }
    if item_count == 0 {
        report.warning(
            "no publishable content found",
            "add Markdown files under content/posts or pass --drafts when building drafts",
        );
    } else {
        report.ok(format!("{} publishable content item(s) found", item_count));
    }

    check_model_output_collisions(report, config, all_items);
}

fn check_model_output_collisions(
    report: &mut DoctorReport,
    config: &crate::config::SiteConfig,
    all_items: Vec<crate::content::ContentItem>,
) {
    let site_model = crate::model::build_site_model(all_items, &config.collections, config);
    if let Err(err) = crate::model::validate_unique_page_outputs(&site_model) {
        report.error(
            err.to_string(),
            "change one slug, collection route, taxonomy slug, or section path",
        );
    }
}

fn check_public_assets(report: &mut DoctorReport, public_dir: &Path) {
    if !public_dir.is_dir() {
        return;
    }
    let mut lower_paths = HashSet::new();
    match collect_files(public_dir) {
        Ok(files) => {
            for file in files {
                let rel = file.strip_prefix(public_dir).unwrap_or(&file);
                let rel_str = rel.to_string_lossy();
                if rel.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir | std::path::Component::RootDir
                    )
                }) {
                    report.error(
                        format!("unsafe public asset path: {}", rel.display()),
                        "keep public assets inside the configured public directory",
                    );
                }
                let normalized = rel_str.to_lowercase();
                if !lower_paths.insert(normalized) {
                    report.warning(
                        format!(
                            "case-insensitive duplicate public asset path: {}",
                            rel.display()
                        ),
                        "rename one asset to avoid deploy differences across filesystems",
                    );
                }
            }
        }
        Err(e) => report.error(
            format!("failed to scan public assets: {}", e),
            "check permissions under the public directory",
        ),
    }
}

fn run_doctor_build(report: &mut DoctorReport, config: &crate::config::SiteConfig) {
    let temp_output = match temp_check_dir() {
        Ok(dir) => dir,
        Err(e) => {
            report.error(
                format!("failed to create temporary doctor output: {}", e),
                "check the system temp directory",
            );
            return;
        }
    };
    let _guard = TempDirGuard::new(temp_output.clone());
    let artifacts = match crate::site::BuildArtifacts::load(config) {
        Ok(artifacts) => artifacts,
        Err(_) => return,
    };
    match crate::site::build_with_artifacts(
        config,
        &temp_output,
        None,
        &artifacts,
        crate::site::BuildOptions {
            include_drafts: false,
            mode: crate::site::BuildMode::Full,
            emit_report: false,
            profile: false,
            profile_json: false,
        },
    ) {
        Ok(_) => report.ok("dry build completed without writing dist"),
        Err(e) => report.error(
            format!("dry build failed: {}", e),
            "fix render, route, or content errors before building",
        ),
    }
}

#[derive(Default)]
struct DoctorReport {
    lines: Vec<String>,
    errors: usize,
    warnings: usize,
}

impl DoctorReport {
    fn ok(&mut self, message: impl Into<String>) {
        self.lines.push(format!("ok: {}", message.into()));
    }

    fn warning(&mut self, message: impl Into<String>, hint: &str) {
        self.warnings += 1;
        self.lines
            .push(format!("warning: {}\n  hint: {}", message.into(), hint));
    }

    fn error(&mut self, message: impl Into<String>, hint: &str) {
        self.errors += 1;
        self.lines
            .push(format!("error: {}\n  hint: {}", message.into(), hint));
    }

    fn emit(&self) {
        eprintln!("kiln doctor");
        for line in &self.lines {
            eprintln!("{}", line);
        }
    }
}

fn clean(output: &Path, cache_only: bool) -> anyhow::Result<()> {
    let output = absolutize(output)?;
    ensure_safe_clean_target(&output)?;
    let manifest_path = output.join(".kiln").join("manifest.json");

    if cache_only {
        let cache_dir = output.join(".kiln");
        if cache_dir.exists() {
            if !manifest_path.is_file() {
                anyhow::bail!(
                    "Refusing to clean cache state in {} because it does not look like a kiln output directory (missing .kiln/manifest.json)",
                    output.display()
                );
            }
            std::fs::remove_dir_all(&cache_dir)?;
            eprintln!("Removed kiln cache state: {}", cache_dir.display());
        } else {
            eprintln!("No kiln cache state found at {}", cache_dir.display());
        }
        return Ok(());
    }

    if !output.exists() {
        eprintln!("Nothing to clean: {}", output.display());
        return Ok(());
    }
    if !output.is_dir() {
        anyhow::bail!(
            "Refusing to clean non-directory output path {}",
            output.display()
        );
    }

    if !manifest_path.is_file() {
        anyhow::bail!(
            "Refusing to clean {} because it does not look like a kiln output directory (missing .kiln/manifest.json)",
            output.display()
        );
    }

    let manifest = crate::BuildManifest::load(&output)?;
    let mut candidates: HashSet<PathBuf> = HashSet::new();
    for entry in manifest.entries {
        for generated in entry.outputs {
            candidates.insert(generated);
        }
    }
    let asset_manifest = crate::AssetManifest::load(&output).unwrap_or_default();
    for generated in asset_manifest.mappings.values() {
        candidates.insert(PathBuf::from(generated));
    }
    candidates.insert(PathBuf::from("asset_manifest.json"));
    candidates.insert(PathBuf::from("rss.xml"));
    candidates.insert(PathBuf::from("robots.txt"));
    candidates.insert(PathBuf::from("sitemap.xml"));
    candidates.insert(PathBuf::from("sitemap-pages.xml"));
    candidates.insert(PathBuf::from("sitemap-posts.xml"));
    candidates.insert(PathBuf::from("_headers"));

    let mut removed_files = 0usize;
    for relative in candidates {
        let path = safe_output_child(&output, &relative)?;
        if path.is_file() {
            std::fs::remove_file(&path)?;
            removed_files += 1;
            remove_empty_parents(&output, path.parent())?;
        }
    }
    eprintln!(
        "Removed {} generated file(s) from {}",
        removed_files,
        output.display()
    );
    eprintln!("Kept cache state at {}", output.join(".kiln").display());
    Ok(())
}

fn safe_output_child(output: &Path, relative: &Path) -> anyhow::Result<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        anyhow::bail!(
            "Refusing to clean unsafe generated path {}",
            relative.display()
        );
    }
    Ok(output.join(relative))
}

fn remove_empty_parents(output: &Path, mut current: Option<&Path>) -> anyhow::Result<()> {
    while let Some(dir) = current {
        if dir == output || dir == output.join(".kiln") {
            break;
        }
        match std::fs::remove_dir(dir) {
            Ok(()) => current = dir.parent(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => current = dir.parent(),
            Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn absolutize(path: &Path) -> anyhow::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn ensure_safe_clean_target(path: &Path) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if path == Path::new("/") || path == cwd || home.as_deref() == Some(path) {
        anyhow::bail!("Refusing to clean unsafe output path {}", path.display());
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        anyhow::bail!("Refusing to clean path containing '..': {}", path.display());
    }
    Ok(())
}

fn collect_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_into(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_into(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_into(&path, files)?;
        } else if path.is_file() {
            files.push(path);
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
