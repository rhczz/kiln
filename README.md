[简体中文](README.zh-CN.md)

# kiln

A lean static site compiler that bakes Markdown into HTML.

Built with Rust. Zero runtime dependencies. Single binary.

## Quick Start

```bash
# Create a site
kiln init my-site
cd my-site

# Build static output
kiln build --config site.config.toml --output dist

# Include draft posts
kiln build --config site.config.toml --drafts

# Build with profiling
kiln build --config site.config.toml --profile

# Build with machine-readable profiling
kiln build --config site.config.toml --profile-json

# Validate config and content without writing output
# Validate without writing dist/
kiln check --config site.config.toml

# Inspect common project issues
kiln doctor --config site.config.toml

# Start the development server
kiln serve --config site.config.toml --port 4173
```

## Project Structure

```
site/
  site.config.toml   # Site configuration
  content/
    posts/           # Blog posts (Markdown + frontmatter)
    pages/           # Static pages
  templates/         # Tera HTML templates (optional, defaults built-in)
  public/            # Static assets (copied with content-hash filenames)
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
| `kiln init <path>` | Create a minimal site that builds with the built-in templates |
| `kiln build` | Build the static site to output directory |
| `kiln check` | Validate config and content without building |
| `kiln doctor` | Inspect config, content, templates, routes, and assets with hints |
| `kiln clean` | Remove generated output or only `.kiln` cache state |
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
