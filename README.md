[简体中文](README.zh-CN.md)

# kiln

A lean static site compiler that bakes Markdown into HTML.

Built with Rust. Zero runtime dependencies. Single binary.

## Features

- Markdown to HTML with syntax highlighting, tables, task lists (comrak)
- TOML frontmatter for posts and pages
- Tera templating engine with built-in default templates
- Collections with custom routes and templates
- Taxonomies (tags, categories, or custom groups) with dedicated templates
- Paginated archives and section pages
- RSS feed and sitemap generation
- Built-in dev server with live reload on file changes
- Content hashing for cache busting
- Incremental builds with content-aware caching
- Draft support
- Build profiling (`--profile`) with cache hit rate and per-page timing
- Structured diagnostics with colored terminal output

## Quick Start

```bash
# Build your site
kiln build --config site/site.config.toml --output dist

# Include draft posts
kiln build --config site/site.config.toml --drafts

# Build with profiling
kiln build --config site/site.config.toml --profile

# Validate config and content without writing output
kiln check --config site/site.config.toml

# Start dev server with auto-rebuild
kiln serve --config site/site.config.toml --port 4173
```

## Project Structure

```
site/
  site.config.toml   # Site configuration
  content/
    posts/           # Blog posts (Markdown + frontmatter)
    pages/           # Static pages
  templates/         # Tera HTML templates (optional, defaults built-in)
  public/            # Static assets (copied as-is)
  styles.css         # Site stylesheet
```

## Configuration

Minimal `site.config.toml`:

```toml
[site]
title = "My Site"
base_url = "https://example.com"

[author]
name = "Author"
email = "author@example.com"
```

With collections and taxonomies:

```toml
[site]
title = "My Site"
base_url = "https://example.com"
language = "en"

paginate_by = 10
paginate_path = "page"

[[collections]]
name = "posts"
directory = "posts"
route = "/posts/{slug}/"
template = "post.html"
date_ordered = true
feed = true

[[taxonomies]]
name = "tags"
slug = "tags"

[[menus.main]]
name = "Home"
url = "/"
weight = 1
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `kiln build` | Build the static site to output directory |
| `kiln check` | Validate config and content without building |
| `kiln serve` | Start dev server with auto-rebuild on file changes |

### Build Flags

| Flag | Description |
|------|-------------|
| `--config <path>` | Path to site config (default: `site/site.config.toml`) |
| `--output <dir>` | Output directory (default: `dist`) |
| `--drafts` | Include draft posts in the build |
| `--profile` | Emit detailed build report with cache and render metrics |

## Build from Source

```bash
git clone https://github.com/rhczz/kiln.git
cd kiln
cargo build --release
```

## License

MIT
