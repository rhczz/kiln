# Deploy

kiln produces static files. Any static host can serve the output directory.

## Build for Production

Set the final public URL:

```toml
[site]
base_url = "https://example.com"
```

Build:

```bash
kiln build --config site.config.toml --output dist
```

Validate:

```bash
kiln check --config site.config.toml
kiln doctor --config site.config.toml
```

Deploy the contents of `dist/`.

## GitHub Pages

Build to a directory that your publish workflow uploads:

```bash
kiln build --config site.config.toml --output dist
```

Upload `dist/` with your existing Pages workflow. For project pages, set `site.base_url` to the final project URL.

## Netlify

Use:

```text
Build command: kiln build --config site.config.toml --output dist
Publish directory: dist
```

kiln writes a Netlify-style `_headers` file for immutable hashed stylesheet caching.

## Cloudflare Pages

Use:

```text
Build command: kiln build --config site.config.toml --output dist
Output directory: dist
```

## Any Static Server

The output has no runtime dependency on kiln:

```text
dist/
  index.html
  rss.xml
  sitemap.xml
  robots.txt
  assets/
  .kiln/
```

`.kiln/` is build metadata that may contain local filesystem paths. Exclude `dist/.kiln/` when deploying.

## Pre-publish Checklist

- `site.base_url` matches the final URL.
- `kiln check` passes.
- `kiln doctor` has no errors.
- Drafts are intentionally included or excluded.
- Generated `rss.xml`, `sitemap.xml`, and `robots.txt` point at the correct URL.
