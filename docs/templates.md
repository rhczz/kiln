# Templates

kiln uses [Tera](https://tera.netlify.app/) templates. Built-in templates are used when a site does not provide overrides.

For the complete variable reference, see [template-context.md](template-context.md).

## Template Lookup

Built-in templates:

| Template | Purpose |
|---|---|
| `layout.html` | Full HTML document shell |
| `home.html` | Home page |
| `post.html` | Date-ordered content item |
| `page.html` | Non-date content item |
| `section.html` | Directory section page |
| `taxonomy.html` | Taxonomy index page |
| `term.html` | Taxonomy term page |
| `404.html` | Not found page |

Files in `templates/` with the same names override the built-ins.

Collection-specific templates can be selected in config:

```toml
[[collections]]
name = "docs"
directory = "docs"
route = "/docs/{slug}/"
template = "doc.html"
```

## Layout Pattern

`layout.html` receives the rendered body from the current page template.

```html
<!doctype html>
<html lang="{{ site.language }}">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{% if title %}{{ title }} · {{ site.title }}{% else %}{{ site.title }}{% endif %}</title>
    <link rel="stylesheet" href="{{ site.stylesheet_href }}">
  </head>
  <body>
    <header>
      <a href="/">{{ site.title }}</a>
    </header>
    <main>{{ body | safe }}</main>
  </body>
</html>
```

## Page Template

```html
<article>
  <h1>{{ page.title }}</h1>
  {% if page.iso_date %}
  <time datetime="{{ page.iso_date }}">{{ page.long_date }}</time>
  {% endif %}
  {{ page.body_html | safe }}
</article>
```

## Home Template

```html
<section>
  <h1>{{ site.title }}</h1>
  {% if site.description %}
  <p>{{ site.description }}</p>
  {% endif %}
</section>

{% for year in archive %}
<section>
  <h2>{{ year.year }}</h2>
  {% for post in year.posts %}
  <article>
    <a href="{{ post.url }}">{{ post.title }}</a>
    <time datetime="{{ post.iso_date }}">{{ post.short_date }}</time>
  </article>
  {% endfor %}
</section>
{% endfor %}
```

## Menus

```html
{% if menus.main %}
<nav>
  {% for item in menus.main %}
  <a href="{{ item.url }}">{{ item.name }}</a>
  {% endfor %}
</nav>
{% endif %}
```

Menu items are sorted by `weight`.

## Assets

Use `asset_url` for files in `public/`:

```html
<img src="{{ asset_url(path='images/avatar.svg') }}" alt="Avatar">
```

The output URL points at the fingerprinted asset when the file type is fingerprinted.

## Escaping

Tera escapes variables by default. Render kiln-generated Markdown HTML with `safe`:

```html
{{ page.body_html | safe }}
```

Do not mark untrusted custom frontmatter as `safe`.
