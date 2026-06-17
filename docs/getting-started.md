# Getting Started

This guide creates a small site, builds it, and starts the local development server.

## Install

Build the binary from source:

```bash
cargo build --release
```

Use the binary directly:

```bash
./target/release/kiln --help
```

During development, run commands through Cargo:

```bash
cargo run -- build --config examples/blog-basic/site.config.toml --output /tmp/kiln-blog
```

## Create a Site

```bash
kiln init my-site
cd my-site
```

`kiln init` writes a minimal project:

```text
site.config.toml
content/
  posts/
    hello.md
templates/
public/
styles.css
```

The generated site uses kiln's built-in templates, so it can build immediately.

## Build

```bash
kiln build --config site.config.toml --output dist
```

The output directory contains static files:

```text
dist/
  index.html
  posts/hello/index.html
  rss.xml
  sitemap.xml
  robots.txt
  asset_manifest.json
  .kiln/manifest.json
```

`dist/` is generated output. Keep source content, templates, and assets outside it.

## Check and Diagnose

Use `check` before publishing:

```bash
kiln check --config site.config.toml
```

Use `doctor` when a project does not behave as expected:

```bash
kiln doctor --config site.config.toml
```

`check` and `doctor` write to temporary output directories, not to your configured `dist/`.

## Serve Locally

```bash
kiln serve --config site.config.toml --output dist --port 4173
```

Open `http://127.0.0.1:4173`.

The dev server watches content, templates, public assets, styles, and config. If a rebuild fails, it keeps serving the last successful output.

## Clean Output

Remove generated output while keeping `.kiln` state:

```bash
kiln clean --output dist
```

Remove only `.kiln` state:

```bash
kiln clean --output dist --cache
```

## Next Steps

- Read [configuration.md](configuration.md) to shape routes and collections.
- Read [content-model.md](content-model.md) for frontmatter fields.
- Read [templates.md](templates.md) before overriding built-in templates.
- Read [assets.md](assets.md) for fingerprinted static files.
