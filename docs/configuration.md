# Configuration

kiln reads `site.config.toml`. Paths are resolved relative to the config file.

For the exhaustive field reference, see [config-schema.md](config-schema.md).

## Minimal Config

```toml
[site]
title = "My Site"
description = "Notes and writing"
base_url = "https://example.com"

[author]
name = "Author Name"
email = "author@example.com"
```

With this config, kiln uses default paths:

```toml
[paths]
content = "content"
templates = "templates"
public = "public"
styles = "styles.css"
```

It also uses the default collections:

```toml
[[collections]]
name = "posts"
directory = "posts"
route = "/posts/{slug}/"
template = "post.html"
date_ordered = true
feed = true

[[collections]]
name = "pages"
directory = "pages"
route = "/{slug}/"
template = "page.html"
date_ordered = false
feed = false
```

## Site Metadata

```toml
[site]
title = "Field Notes"
subtitle = "Small dispatches"
description = "Writing about systems and craft"
language = "en"
base_url = "https://notes.example.com"
```

`base_url` is used for RSS, sitemap, robots.txt, and canonical URLs. Use the final public URL before publishing.

## Collections

Collections map content directories to URL routes and templates.

```toml
[[collections]]
name = "docs"
directory = "docs"
route = "/docs/{slug}/"
template = "doc.html"
date_ordered = false
feed = false
```

Rules:

- `name` is exposed as `page.type`.
- `directory` is relative to `content`.
- `route` must start and end with `/` and include `{slug}`.
- `date_ordered = true` requires a `date` field in every item.
- `feed = true` includes items in `rss.xml`.

If you define any collection, kiln replaces the defaults. Add `posts` and `pages` explicitly if you still need them.

## Taxonomies

Taxonomies group pages by frontmatter fields.

```toml
[[taxonomies]]
name = "tags"
slug = "tags"
template = "term.html"
```

Content:

```yaml
---
title: "Rust Notes"
date: "2026-06-01"
tags: ["rust", "ssg"]
---
```

Outputs:

```text
/tags/
/tags/rust/
/tags/ssg/
```

## Pagination

```toml
paginate_by = 10
paginate_path = "page"
```

This paginates home, section, and taxonomy term lists. Page 1 stays at the base URL. Later pages use paths such as `/page/2/` or `/posts/page/2/`.

Set `paginate_by = 0` to disable pagination.

## Menus

Menus are named lists of links.

```toml
[[menu.main]]
name = "Home"
url = "/"
weight = 1

[[menu.main]]
name = "Writing"
url = "/posts/"
weight = 2
```

Templates can read `menus.main`.

## Template Data

Templates always receive `site`, `menus`, page-specific variables, and generated asset data. See [template-context.md](template-context.md) for the complete reference.

When adding root-level fields such as `paginate_by`, place them outside tables like `[paths]` or `[site]`. In TOML, keys after `[paths]` still belong to `[paths]` until a new table starts.

## Validation

kiln fails fast for unsafe or ambiguous configuration:

- duplicate collection names or directories
- duplicate taxonomy names or slugs
- routes without `{slug}`
- paths containing `..`
- taxonomy slugs that conflict with collection route namespaces
- final output path conflicts between content, generated pages, and aliases
