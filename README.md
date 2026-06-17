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

# Validate without writing dist/
kiln check --config site.config.toml

# Inspect common project issues
kiln doctor --config site.config.toml

# Start the development server
kiln serve --config site.config.toml --port 4173
```

## What kiln Provides

- Markdown to HTML with TOML/YAML frontmatter
- Tera templates with built-in defaults
- Collections, taxonomies, sections, and pagination
- RSS, sitemap, robots.txt, and 404 output
- Draft filtering
- Static asset fingerprinting and CSS `url(...)` rewriting
- `asset_url(...)` for templates
- Incremental rebuilds in `kiln serve`
- Build manifests, asset manifests, and profiling output
- `init`, `check`, `doctor`, `clean`, and `serve` CLI workflows

## Documentation

| Topic | Link |
|---|---|
| Start a new site | [docs/getting-started.md](docs/getting-started.md) |
| Configure kiln | [docs/configuration.md](docs/configuration.md) |
| Full config schema | [docs/config-schema.md](docs/config-schema.md) |
| Write content | [docs/content-model.md](docs/content-model.md) |
| Build templates | [docs/templates.md](docs/templates.md) |
| Template context reference | [docs/template-context.md](docs/template-context.md) |
| Use assets | [docs/assets.md](docs/assets.md) |
| Understand builds and cache behavior | [docs/build-model.md](docs/build-model.md) |
| Deploy output | [docs/deploy.md](docs/deploy.md) |

## Examples

The examples are complete kiln sites and are built by the test suite.

```bash
cargo run -- build --config examples/blog-basic/site.config.toml --output /tmp/kiln-blog
cargo run -- build --config examples/docs-site/site.config.toml --output /tmp/kiln-docs
cargo run -- build --config examples/portfolio/site.config.toml --output /tmp/kiln-portfolio
```

| Example | Use case |
|---|---|
| [examples/blog-basic](examples/blog-basic) | Personal blog with posts, pages, tags, and assets |
| [examples/docs-site](examples/docs-site) | Small documentation site with a custom docs collection |
| [examples/portfolio](examples/portfolio) | Portfolio with project metadata exposed through templates |

## Build from Source

```bash
git clone https://github.com/rhczz/kiln.git
cd kiln
cargo build --release
```

## License

MIT
