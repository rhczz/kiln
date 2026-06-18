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
            let output = crate::output_safety::ensure_safe_output_target(
                "build",
                &output,
                &config,
                &_base_dir,
                &site_config,
            )?;
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
    std::fs::create_dir_all(path.join("content/pages"))?;
    std::fs::create_dir_all(path.join("templates"))?;
    std::fs::create_dir_all(path.join("public"))?;

    write_new_file(
        &path.join("site.config.toml"),
        &format!(
            r#"[site]
title = "{}"
subtitle = "A site built with kiln"
description = "A tiny site baked by kiln"
language = "en"
base_url = "{}"

[author]
name = "Your Name"

[paths]
content = "content"
templates = "templates"
public = "public"
styles = "styles.css"

[[menu.main]]
name = "Home"
url = "/"
weight = 1

[[menu.main]]
name = "About"
url = "/about/"
weight = 2

[[menu.main]]
name = "Tags"
url = "/tags/"
weight = 3
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
tags: ["intro", "kiln"]
featured: true
---

Welcome to your new kiln site. This is your first post, generated by the
lightweight static site compiler that turns Markdown into fast HTML.

## Getting started

Edit this file in `content/posts/hello.md`, then rebuild:

```bash
kiln build --config site.config.toml --output dist
```

Or start the dev server with live reload:

```bash
kiln serve --config site.config.toml
```

## What you can do

kiln supports **bold**, *italic*, ~~strikethrough~~, `inline code`, and more.

> A blockquote for highlighting ideas.

### Lists

- Task lists with `- [x]` syntax
- Footnotes[^1]
- Tables, definition lists, and superscript

### Code

```rust
fn main() {
    println!("Built with kiln");
}
```

## What next?

- Add more posts under `content/posts/`
- Create pages under `content/pages/`
- Customize the stylesheet in `styles.css`
- Add navigation links in `site.config.toml`

[^1]: This is a footnote rendered by kiln.
"#,
    )?;
    write_new_file(
        &path.join("content/pages/about.md"),
        r#"---
title: "About"
---

This site is built with kiln, a lean static site compiler written in Rust.

kiln compiles Markdown to HTML with zero runtime dependencies. The output is
just static files, ready to deploy anywhere.
"#,
    )?;
    write_new_file(
        &path.join("styles.css"),
        r##"/* ===== Theme tokens ===== */

:root {
  color-scheme: light dark;

  --bg: #fafafa;
  --surface: #fff;
  --ink: #18181b;
  --ink-2: #3f3f46;
  --muted: #71717a;
  --border: #e4e4e7;
  --accent: #0f766e;
  --accent-hover: #115e59;
  --accent-bg: #f0fdfa;
  --code-bg: #f4f4f5;
  --radius: 8px;
  --radius-sm: 4px;
  --shadow-sm: 0 1px 2px rgb(0 0 0 / .05);
  --shadow: 0 1px 3px rgb(0 0 0 / .08), 0 1px 2px rgb(0 0 0 / .04);
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #09090b;
    --surface: #18181b;
    --ink: #fafafa;
    --ink-2: #d4d4d8;
    --muted: #a1a1aa;
    --border: #27272a;
    --accent: #2dd4bf;
    --accent-hover: #5eead4;
    --accent-bg: #042f2e;
    --code-bg: #27272a;
    --shadow-sm: 0 1px 2px rgb(0 0 0 / .2);
    --shadow: 0 1px 3px rgb(0 0 0 / .3), 0 1px 2px rgb(0 0 0 / .2);
  }
}

/* ===== Reset ===== */

*,
*::before,
*::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

/* ===== Base ===== */

html {
  -webkit-text-size-adjust: 100%;
  text-size-adjust: 100%;
  scroll-behavior: smooth;
  scroll-padding-top: 5rem;
}

body {
  background: var(--bg);
  color: var(--ink);
  font-family: ui-serif, Georgia, "Times New Roman", serif;
  font-size: clamp(1rem, 0.95rem + 0.25vw, 1.0625rem);
  line-height: 1.7;
  min-height: 100dvh;
  display: flex;
  flex-direction: column;
  font-feature-settings: "kern" 1, "liga" 1, "calt" 1;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

::selection {
  background: var(--accent);
  color: var(--bg);
}

:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
  border-radius: 2px;
}

/* ===== Layout ===== */

.container {
  width: 100%;
  max-width: 50rem;
  margin-inline: auto;
  padding-inline: 1.5rem;
}

.skip-link {
  position: absolute;
  left: -9999px;
  top: auto;
  z-index: 100;
  padding: 0.5rem 1rem;
  background: var(--accent);
  color: #fff;
  font-family: system-ui, sans-serif;
  font-size: 0.875rem;
  font-weight: 500;
  text-decoration: none;
  border-radius: var(--radius-sm);
}

.skip-link:focus {
  left: 1rem;
  top: 1rem;
}

/* ===== Typography ===== */

h1, h2, h3, h4, h5, h6 {
  font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
  font-weight: 650;
  line-height: 1.25;
  color: var(--ink);
  text-wrap: balance;
}

h1 { font-size: clamp(1.75rem, 1.5rem + 1.25vw, 2.5rem); letter-spacing: -0.025em; }
h2 { font-size: clamp(1.25rem, 1.1rem + 0.75vw, 1.75rem); letter-spacing: -0.02em; }
h3 { font-size: clamp(1.0625rem, 1rem + 0.3vw, 1.3125rem); }

a {
  color: var(--accent);
  text-decoration-thickness: 1px;
  text-underline-offset: 3px;
  transition: color 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

a:hover {
  color: var(--accent-hover);
}

.heading-anchor {
  color: var(--muted);
  text-decoration: none;
  opacity: 0;
  margin-left: 0.375em;
  font-weight: 400;
  transition: opacity 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              color 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

h1:hover .heading-anchor,
h2:hover .heading-anchor,
h3:hover .heading-anchor,
h4:hover .heading-anchor,
.heading-anchor:focus {
  opacity: 0.6;
}

.heading-anchor:hover {
  opacity: 1;
  color: var(--accent);
}

time {
  color: var(--muted);
  font-family: system-ui, sans-serif;
  font-size: 0.875em;
  font-variant-numeric: tabular-nums;
}

/* ===== Header ===== */

.site-header {
  padding-block: 1rem;
  border-bottom: 1px solid var(--border);
  position: sticky;
  top: 0;
  z-index: 10;
  background: color-mix(in srgb, var(--bg) 85%, transparent);
  backdrop-filter: blur(12px) saturate(1.6);
  -webkit-backdrop-filter: blur(12px) saturate(1.6);
}

.site-header .container {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1.5rem;
}

.site-title {
  font-family: system-ui, sans-serif;
  font-size: 1.125rem;
  font-weight: 700;
  color: var(--ink);
  text-decoration: none;
  letter-spacing: -0.02em;
  flex-shrink: 0;
}

.site-title:hover {
  color: var(--accent);
}

.site-header nav {
  display: flex;
  gap: 1.25rem;
  align-items: center;
  flex-wrap: wrap;
}

.site-header nav a {
  font-family: system-ui, sans-serif;
  font-size: 0.875rem;
  font-weight: 450;
  color: var(--muted);
  text-decoration: none;
  padding-bottom: 2px;
  border-bottom: 1.5px solid transparent;
  transition: color 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              border-color 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.site-header nav a:hover {
  color: var(--ink);
  border-bottom-color: var(--accent);
}

/* ===== Main ===== */

main {
  flex: 1;
  padding-block: 3rem;
}

/* ===== Footer ===== */

.site-footer {
  padding-block: 2rem;
  border-top: 1px solid var(--border);
  margin-top: auto;
}

.footer-inner {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem 1.5rem;
  align-items: center;
  font-family: system-ui, sans-serif;
  font-size: 0.8125rem;
}

.footer-copy {
  color: var(--muted);
}

.footer-nav {
  display: flex;
  gap: 1rem;
}

.footer-nav a {
  color: var(--muted);
  text-decoration: none;
}

.footer-nav a:hover {
  color: var(--ink);
}

.footer-meta {
  margin-left: auto;
}

.rss-link {
  color: var(--muted);
  text-decoration: none;
  font-size: 0.8125rem;
}

.rss-link:hover {
  color: var(--accent);
}

/* ===== Hero ===== */

.hero {
  margin-bottom: 3rem;
  padding-bottom: 2.5rem;
  border-bottom: 1px solid var(--border);
}

.hero h1 {
  margin-bottom: 0.5rem;
}

.hero-subtitle {
  font-size: clamp(1.0625rem, 1rem + 0.3vw, 1.25rem);
  color: var(--ink-2);
  margin-top: 0.5rem;
  line-height: 1.5;
}

.hero-intro {
  color: var(--muted);
  margin-top: 1rem;
  max-width: 40rem;
}

/* ===== Featured ===== */

.featured {
  margin-bottom: 3rem;
}

.featured > h2 {
  margin-bottom: 1.25rem;
}

.featured-grid {
  display: grid;
  gap: 1rem;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 18rem), 1fr));
}

.featured-card {
  padding: 1.25rem 1.5rem;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  transition: border-color 0.25s cubic-bezier(0.16, 1, 0.3, 1),
              box-shadow 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.featured-card:hover {
  border-color: var(--accent);
  box-shadow: var(--shadow);
}

.featured-card h3 {
  font-size: 1.0625rem;
  margin-bottom: 0.375rem;
}

.featured-card h3 a {
  text-decoration: none;
}

.featured-card p {
  color: var(--muted);
  font-size: 0.9375rem;
  line-height: 1.5;
}

.featured-card time {
  display: block;
  margin-top: 0.75rem;
  font-size: 0.8125rem;
}

/* ===== Archive ===== */

.archive {
  margin-bottom: 2rem;
}

.archive-year {
  margin-bottom: 2rem;
}

.archive-year-label {
  font-size: 0.8125rem;
  font-family: system-ui, sans-serif;
  font-weight: 500;
  color: var(--muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: 0.75rem;
}

.archive-list {
  list-style: none;
}

.archive-item {
  display: flex;
  gap: 1rem;
  padding-block: 0.625rem;
  border-bottom: 1px solid var(--border);
}

.archive-item:last-child {
  border-bottom: none;
}

.archive-date {
  flex-shrink: 0;
  width: 5.5rem;
  padding-top: 0.125rem;
}

.archive-content {
  flex: 1;
  min-width: 0;
}

.archive-link {
  font-family: system-ui, sans-serif;
  font-weight: 500;
  text-decoration: none;
  font-size: 0.9375rem;
}

.archive-link:hover {
  text-decoration: underline;
  text-underline-offset: 3px;
}

.archive-desc {
  color: var(--muted);
  font-size: 0.875rem;
  margin-top: 0.125rem;
  line-height: 1.5;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 1;
  -webkit-box-orient: vertical;
}

/* ===== Post ===== */

.post-header {
  margin-bottom: 2rem;
  padding-bottom: 1.5rem;
  border-bottom: 1px solid var(--border);
}

.post-title {
  margin-top: 0.5rem;
}

.post-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem 1rem;
  align-items: center;
  margin-top: 0.75rem;
}

.post-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.375rem;
}

.tag {
  font-family: system-ui, sans-serif;
  font-size: 0.75rem;
  font-weight: 500;
  padding: 0.125rem 0.625rem;
  background: var(--accent-bg);
  color: var(--accent);
  border-radius: 999px;
  white-space: nowrap;
  transition: background 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              color 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.tag:hover {
  background: var(--accent);
  color: var(--bg);
}

.post-description {
  color: var(--ink-2);
  margin-top: 0.75rem;
  font-size: 1.0625rem;
  line-height: 1.5;
}

/* ===== Page ===== */

.page-header {
  margin-bottom: 2rem;
}

.page-title {
  margin-top: 0.5rem;
}

.page-description {
  color: var(--ink-2);
  margin-top: 0.5rem;
  font-size: 1.0625rem;
}

/* ===== Back link ===== */

.back-link {
  display: inline-flex;
  align-items: center;
  gap: 0.25em;
  font-family: system-ui, sans-serif;
  font-size: 0.8125rem;
  font-weight: 450;
  color: var(--muted);
  text-decoration: none;
  margin-bottom: 0.75rem;
  transition: color 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              gap 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.back-link:hover {
  color: var(--accent);
  gap: 0.5em;
}

/* ===== Table of contents ===== */

.toc {
  margin-bottom: 2rem;
  padding: 1rem 1.25rem;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  font-family: system-ui, sans-serif;
  font-size: 0.875rem;
}

.toc summary {
  font-weight: 600;
  cursor: pointer;
  user-select: none;
  color: var(--ink);
  font-size: 0.8125rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.toc nav {
  margin-top: 0.75rem;
}

.toc ol {
  list-style: none;
  counter-reset: toc;
}

.toc li {
  counter-increment: toc;
  padding-block: 0.2rem;
}

.toc li::before {
  content: counter(toc) ".";
  color: var(--muted);
  margin-right: 0.5rem;
  font-variant-numeric: tabular-nums;
  font-size: 0.8125rem;
}

.toc a {
  text-decoration: none;
  color: var(--ink-2);
}

.toc a:hover {
  color: var(--accent);
}

/* ===== Prose ===== */

.prose {
  line-height: 1.8;
  overflow-wrap: break-word;
}

.prose > * + * {
  margin-top: 1.25em;
}

.prose h2,
.prose h3,
.prose h4 {
  margin-top: 2em;
  margin-bottom: 0.5em;
}

.prose p {
  max-width: 65ch;
}

.prose a {
  text-decoration-thickness: 1px;
  text-underline-offset: 2px;
}

.prose strong {
  font-weight: 600;
  color: var(--ink);
}

.prose blockquote {
  border-left: 3px solid var(--accent);
  padding-left: 1.25rem;
  margin-left: 0;
  color: var(--ink-2);
  font-style: italic;
}

.prose ul,
.prose ol {
  padding-left: 1.5rem;
}

.prose li + li {
  margin-top: 0.375em;
}

.prose li > ul,
.prose li > ol {
  margin-top: 0.375em;
}

.prose code {
  font-family: ui-monospace, "SF Mono", "Cascadia Code", "Fira Code", monospace;
  font-size: 0.875em;
  background: var(--code-bg);
  padding: 0.125em 0.375em;
  border-radius: var(--radius-sm);
}

.prose pre {
  background: var(--code-bg);
  padding: 1rem 1.25rem;
  border-radius: var(--radius);
  overflow-x: auto;
  line-height: 1.6;
}

.prose pre code {
  background: none;
  padding: 0;
  border-radius: 0;
  font-size: 0.8125rem;
}

.prose img {
  max-width: 100%;
  height: auto;
  border-radius: var(--radius);
}

.prose figure {
  text-align: center;
}

.prose figcaption {
  font-size: 0.875rem;
  color: var(--muted);
  margin-top: 0.5rem;
}

.prose hr {
  border: none;
  border-top: 1px solid var(--border);
  margin: 2rem 0;
}

.prose table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9375rem;
}

.table-scroll {
  overflow-x: auto;
  margin-block: 1.25em;
}

.prose th,
.prose td {
  padding: 0.5rem 0.75rem;
  text-align: left;
  border-bottom: 1px solid var(--border);
}

.prose th {
  font-family: system-ui, sans-serif;
  font-weight: 600;
  font-size: 0.8125rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--muted);
}

.prose tbody tr:nth-child(even) td {
  background: color-mix(in srgb, var(--surface) 50%, var(--bg));
}

.prose tr:hover td {
  background: var(--accent-bg);
}

.prose kbd {
  font-family: ui-monospace, "SF Mono", "Cascadia Code", "Fira Code", monospace;
  font-size: 0.8125em;
  padding: 0.125em 0.375em;
  border: 1px solid var(--border);
  border-bottom-width: 2px;
  border-radius: var(--radius-sm);
  background: var(--surface);
  white-space: nowrap;
}

.prose dl dt {
  font-weight: 600;
  margin-top: 1em;
}

.prose dl dd {
  padding-left: 1.5rem;
  color: var(--ink-2);
}

.prose .task-list-item {
  list-style: none;
  margin-left: -1.5rem;
}

.prose .task-list-item-checkbox {
  margin-right: 0.5rem;
  accent-color: var(--accent);
}

.prose sup {
  font-size: 0.75em;
}

.prose .footnote-definition {
  font-size: 0.875rem;
  color: var(--ink-2);
}

.prose del {
  color: var(--muted);
}

/* ===== Pagination ===== */

.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 1rem;
  margin-top: 2.5rem;
  padding-top: 1.5rem;
  border-top: 1px solid var(--border);
  font-family: system-ui, sans-serif;
  font-size: 0.875rem;
}

.pagination-link {
  padding: 0.375rem 0.75rem;
  text-decoration: none;
  font-weight: 500;
  border-radius: var(--radius-sm);
  transition: background 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              color 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.pagination-link:hover {
  background: var(--accent-bg);
  color: var(--accent);
}

.pagination-info {
  color: var(--muted);
}

/* ===== Section listing ===== */

.section-page h1 {
  margin-bottom: 1.5rem;
}

.breadcrumb {
  font-family: system-ui, sans-serif;
  font-size: 0.8125rem;
  color: var(--muted);
  margin-bottom: 1rem;
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem;
}

.breadcrumb a {
  color: var(--muted);
  text-decoration: none;
}

.breadcrumb a:hover {
  color: var(--accent);
}

.breadcrumb-sep {
  color: var(--border);
}

.subsections {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-bottom: 2rem;
}

.subsection-link {
  font-family: system-ui, sans-serif;
  font-size: 0.8125rem;
  font-weight: 500;
  padding: 0.25rem 0.75rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  text-decoration: none;
  transition: border-color 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              color 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              background 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.subsection-link:hover {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-bg);
}

.section-list {
  list-style: none;
}

.section-item {
  padding-block: 0.75rem;
  border-bottom: 1px solid var(--border);
}

.section-item:last-child {
  border-bottom: none;
}

.section-item-link {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 1rem;
  text-decoration: none;
  font-family: system-ui, sans-serif;
}

.section-item-link:hover .section-item-title {
  color: var(--accent);
}

.section-item-title {
  font-weight: 500;
  transition: color 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.section-item-date {
  flex-shrink: 0;
}

.section-item-desc {
  color: var(--muted);
  font-size: 0.875rem;
  margin-top: 0.125rem;
  line-height: 1.5;
}

/* ===== Taxonomy ===== */

.taxonomy-title {
  margin-bottom: 1.5rem;
}

.term-cloud {
  list-style: none;
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.term-link {
  display: inline-block;
  font-family: system-ui, sans-serif;
  font-size: 0.875rem;
  font-weight: 450;
  padding: 0.375rem 0.875rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  text-decoration: none;
  transition: border-color 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              background 0.2s cubic-bezier(0.16, 1, 0.3, 1),
              color 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.term-link:hover {
  border-color: var(--accent);
  background: var(--accent-bg);
  color: var(--accent);
}

.term-page-title {
  margin-bottom: 1.5rem;
}

/* ===== Home image ===== */

.home-image {
  margin-bottom: 2.5rem;
}

.home-image img {
  width: 100%;
  height: auto;
  border-radius: var(--radius);
}

/* ===== 404 ===== */

.error-page {
  text-align: center;
  padding: 4rem 0;
}

.error-code {
  font-size: clamp(4rem, 3rem + 5vw, 7rem);
  font-weight: 800;
  color: var(--border);
  line-height: 1;
  letter-spacing: -0.04em;
}

.error-message {
  font-family: system-ui, sans-serif;
  font-size: 1.125rem;
  color: var(--muted);
  margin-top: 1rem;
}

.error-link {
  display: inline-block;
  margin-top: 1.5rem;
  font-family: system-ui, sans-serif;
  font-weight: 500;
}

/* ===== Responsive ===== */

@media (max-width: 640px) {
  .container {
    padding-inline: 1rem;
  }

  main {
    padding-block: 2rem;
  }

  .site-header nav {
    gap: 0.75rem;
  }

  .archive-item {
    flex-direction: column;
    gap: 0.125rem;
  }

  .archive-date {
    width: auto;
  }

  .section-item-link {
    flex-direction: column;
    gap: 0.125rem;
  }

  .footer-inner {
    flex-direction: column;
    align-items: flex-start;
  }

  .footer-meta {
    margin-left: 0;
  }

  .featured-grid {
    grid-template-columns: 1fr;
  }
}

/* ===== Motion ===== */

@media (prefers-reduced-motion: reduce) {
  html { scroll-behavior: auto; }
  *, *::before, *::after {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}

/* ===== Print ===== */

@media print {
  .site-header, .site-footer, .skip-link, .pagination, .back-link, .toc {
    display: none;
  }
  body {
    font-size: 12pt;
    color: #000;
    background: #fff;
  }
  main { padding: 0; }
  .container { max-width: none; padding: 0; }
  a { color: inherit; text-decoration: underline; }
}
"##,
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
