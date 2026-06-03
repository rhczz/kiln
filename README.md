[简体中文](README.zh-CN.md)

# kiln

A lean static site compiler that bakes Markdown into HTML.

Built with Rust. Zero runtime dependencies. Single binary.

## Features

- Markdown to HTML with syntax highlighting (comrak)
- TOML frontmatter for posts and pages
- Tera templating engine
- Collections with custom routes and templates
- RSS feed and sitemap generation
- Built-in dev server with live reload on file changes
- Content hashing for cache busting
- Draft support

## Quick Start

```bash
# Build your site
kiln build --config site/site.config.toml --output dist

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
  templates/         # Tera HTML templates
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

## Build from Source

```bash
git clone https://github.com/rhczz/kiln.git
cd kiln
cargo build --release
```

## License

MIT
