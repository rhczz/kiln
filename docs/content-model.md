# Content Model

kiln content is Markdown with YAML or TOML frontmatter. Files live under the configured `content` directory and are grouped by collection.

## Basic Post

```markdown
---
title: "Hello kiln"
date: "2026-06-01"
description: "A first post"
tags: ["intro"]
featured: true
---

# Hello kiln

Write Markdown here.
```

For the default `posts` collection, this writes:

```text
/posts/hello-kiln/
```

## Basic Page

```markdown
---
title: "About"
slug: "about"
---

This is a standalone page.
```

For the default `pages` collection, this writes:

```text
/about/
```

## Frontmatter Fields

| Field | Type | Meaning |
|---|---|---|
| `title` | string | Required page title |
| `date` | string | Required for date-ordered collections, format `YYYY-MM-DD` |
| `description` | string | Optional summary; generated from body when omitted |
| `slug` | string | Optional URL slug; defaults to the filename without a leading date |
| `featured` | boolean | Marks posts for `featured_posts` on the home page |
| `draft` | boolean | Excluded unless `--drafts` is passed |
| `tags` | array | Default taxonomy terms |
| `aliases` | array | Old URLs that should redirect to this page |

Unknown fields are exposed through `page.extra`.

```yaml
---
title: "Launch Notes"
cover: "/images/launch.jpg"
cta:
  label: "Read more"
  href: "/posts/"
---
```

Template:

```tera
{% if page.extra.cover %}
<img src="{{ page.extra.cover }}" alt="">
{% endif %}
```

## Drafts

```yaml
---
title: "Private Draft"
date: "2026-06-01"
draft: true
---
```

Drafts are skipped by default:

```bash
kiln build --config site.config.toml --output dist
```

Include them explicitly:

```bash
kiln build --config site.config.toml --output dist --drafts
```

## Aliases

Aliases generate static redirect HTML files.

```yaml
---
title: "New Guide"
aliases:
  - /old-guide/
  - legacy/guide
---
```

Both `/old-guide/` and `/legacy/guide/` redirect to the page's current URL.

Aliases cannot escape the output directory and cannot collide with real pages, generated taxonomy pages, generated section pages, or other aliases.

## Sections

A directory can define a section page with `_index.md`.

```text
content/docs/
  _index.md
  install.md
  deploy.md
```

`content/docs/_index.md`:

```yaml
---
title: "Documentation"
weight: 1
---
```

The section page receives `section.pages`, `section.children`, and breadcrumb data. See [template-context.md](template-context.md).

## Headings and TOC

Markdown headings receive stable `id` attributes. Templates can render the generated heading list:

```tera
{% if page.toc %}
<nav aria-label="Table of contents">
  {% for heading in page.toc %}
  <a href="#{{ heading.id }}">{{ heading.text }}</a>
  {% endfor %}
</nav>
{% endif %}
```

## Markdown Features

kiln uses comrak and supports common writing features:

- tables
- task lists
- strikethrough
- autolinks
- footnotes
- description lists
- syntax highlighting
