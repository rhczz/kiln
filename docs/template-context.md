# kiln 模板上下文参考

模板引擎使用 [Tera](https://tera.netlify.app/)，语法类似 Jinja2/Django。

内置模板位于 `src/defaults/`，可被 `templates/` 目录下的同名文件覆盖：

| 模板 | 作用 |
|---|---|
| `layout.html` | 全局 HTML 外壳 |
| `home.html` | 首页 |
| `post.html` | 日期排序内容详情页 |
| `page.html` | 非日期内容详情页 |
| `section.html` | 带 `_index.md` 的目录 section 页 |
| `taxonomy.html` | taxonomy 索引页，如 `/tags/` |
| `term.html` | taxonomy term 页，如 `/tags/rust/` |
| `404.html` | 404 页面主体 |

所有页面模板先渲染主体，再交给 `layout.html` 包装。

---

## 全局变量和函数

以下变量会注入到所有模板，包括 `layout.html`、`home.html`、`post.html`、`page.html`、`section.html`、`taxonomy.html`、`term.html` 和 `404.html`。

### `site`

```json
{
  "title": "My Blog",
  "subtitle": "Thoughts",
  "description": "A blog",
  "language": "en",
  "base_url": "https://example.com",
  "stylesheet_href": "/assets/styles.a1b2c3.css",
  "author": {
    "name": "Author Name",
    "email": "author@example.com"
  }
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `site.title` | string | 站点标题 |
| `site.subtitle` | string | 站点副标题，未配置时为空 |
| `site.description` | string | 站点描述 |
| `site.language` | string | HTML `lang` 值 |
| `site.base_url` | string | 站点根 URL，无尾部斜杠 |
| `site.stylesheet_href` | string | 主 stylesheet 的带 hash URL |
| `site.author.name` | string | 作者姓名，未配置时为空 |
| `site.author.email` | string | 作者邮箱，未配置时为空 |

### `theme`

配置文件中未被 kiln 识别的顶层表会透传到 `theme`。结构由站点自己定义。

```toml
[brand]
intro = "Welcome"
email = "hi@example.com"

[[nav]]
label = "Home"
href = "/"
```

模板中访问：

```tera
{{ theme.brand.intro }}
{% for item in theme.nav %}
<a href="{{ item.href }}">{{ item.label }}</a>
{% endfor %}
```

### `menus`

配置中的 `[[menu.<name>]]` 会按 `weight` 排序后注入为 `menus.<name>`。

```json
{
  "main": [
    { "name": "Home", "url": "/", "weight": 1 },
    { "name": "Writing", "url": "/posts/", "weight": 2 }
  ]
}
```

未配置 menu 时，`menus` 不会注入。模板中建议先判断：

```tera
{% if menus.main %}
  {% for item in menus.main %}
  <a href="{{ item.url }}">{{ item.name }}</a>
  {% endfor %}
{% endif %}
```

### `config`

`config` 是 `site`、`theme` 和 `menus` 的兼容合并变量。新模板应优先使用明确的 `site`、`theme` 和 `menus`。

### `asset_manifest`

`asset_manifest` 是 `public/` 原始路径到输出路径的映射。

```json
{
  "images/logo.svg": "images/logo.abc123def456.svg",
  "downloads/file.pdf": "downloads/file.pdf"
}
```

通常应使用 `asset_url(...)`，只有需要直接检查 manifest 时才访问 `asset_manifest`。

### `asset_url(...)`

`asset_url` 是 Tera 函数，用于解析 `public/` asset 的最终输出路径。

```tera
{{ asset_url("images/logo.svg") }}
{{ asset_url(path="images/logo.svg") }}
```

路径会去掉开头的 `/` 或 `./` 后匹配 `asset_manifest`。如果找不到映射，返回原始输入路径。

---

## Layout 变量

`layout.html` 除了全局变量，还接收：

| 变量 | 类型 | 说明 |
|---|---|---|
| `title` | string | 当前页面标题，首页可能为空 |
| `description` | string | 当前页面描述 |
| `body` | string | 子模板渲染后的 HTML 主体 |
| `path` | string | 当前页面路径，用于 canonical 等 URL 拼接 |
| `og_type` | string | `"website"` 或 `"article"` |

典型用法：

```html
<title>{% if title %}{{ title }} · {{ site.title }}{% else %}{{ site.title }}{% endif %}</title>
<meta property="og:type" content="{{ og_type }}">
<main>{{ body | safe }}</main>
```

---

## 列表项基础字段

首页归档、featured posts、section pages、term pages 中的列表项使用同一组基础字段：

| 字段 | 类型 | 说明 |
|---|---|---|
| `title` | string | 页面标题 |
| `slug` | string | URL slug |
| `date` | string | 原始日期，非日期内容为空 |
| `iso_date` | string | ISO 日期，非日期内容为空 |
| `short_date` | string | 短日期，如 `2026.06.01` |
| `long_date` | string | 长日期，如 `June 1, 2026` |
| `year` | string | 年份，非日期内容为空 |
| `description` | string | 页面描述 |
| `url` | string | 站点相对 URL |

---

## `paginator`

当 home、section 或 term 页需要分页时，模板会收到 `paginator`。

```json
{
  "current_index": 2,
  "total_pages": 3,
  "total_items": 25,
  "per_page": 10,
  "first_url": "/posts/",
  "current_url": "/posts/page/2/",
  "prev_url": "/posts/",
  "next_url": "/posts/page/3/",
  "pages": [
    { "index": 1, "url": "/posts/", "is_current": false },
    { "index": 2, "url": "/posts/page/2/", "is_current": true }
  ]
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `paginator.current_index` | number | 当前页码，从 1 开始 |
| `paginator.total_pages` | number | 总页数 |
| `paginator.total_items` | number | 参与分页的总条目数 |
| `paginator.per_page` | number | 每页条目数 |
| `paginator.first_url` | string | 第一页 URL |
| `paginator.current_url` | string | 当前页 URL |
| `paginator.prev_url` | string/null | 上一页 URL |
| `paginator.next_url` | string/null | 下一页 URL |
| `paginator.pages` | object[] | 所有页码链接 |

典型用法：

```tera
{% if paginator and paginator.total_pages > 1 %}
<nav aria-label="Pagination">
  {% if paginator.prev_url %}<a href="{{ paginator.prev_url }}">Previous</a>{% endif %}
  <span>Page {{ paginator.current_index }} of {{ paginator.total_pages }}</span>
  {% if paginator.next_url %}<a href="{{ paginator.next_url }}">Next</a>{% endif %}
</nav>
{% endif %}
```

---

## Home 模板变量

`home.html` 除了全局变量，还接收：

### `featured_posts`

最多 6 篇 `featured: true` 的内容，按站点模型顺序提供基础字段。

### `archive`

日期排序内容按年份分组：

```json
[
  {
    "year": "2026",
    "posts": [
      {
        "title": "Post Title",
        "slug": "post-title",
        "date": "2026-06-01",
        "iso_date": "2026-06-01",
        "short_date": "2026.06.01",
        "long_date": "June 1, 2026",
        "year": "2026",
        "description": "Description",
        "url": "/posts/post-title/"
      }
    ]
  }
]
```

如果启用分页，`archive` 只包含当前页 slice。

---

## Post / Page 模板变量

`post.html`、`page.html` 和自定义 collection template 接收 `page`。

```json
{
  "title": "My Post",
  "slug": "my-post",
  "description": "Post description",
  "url": "/posts/my-post/",
  "body_html": "<p>Rendered HTML content</p>",
  "date": "2026-06-01",
  "iso_date": "2026-06-01",
  "short_date": "2026.06.01",
  "long_date": "June 1, 2026",
  "year": "2026",
  "tags": ["rust", "ssg"],
  "taxonomies": {
    "tags": ["rust", "ssg"],
    "categories": ["notes"]
  },
  "aliases": ["/old-post/"],
  "extra": {
    "cover": "/images/post.jpg"
  },
  "headings": [
    { "level": 2, "id": "install", "text": "Install" }
  ],
  "toc": [
    { "level": 2, "id": "install", "text": "Install" }
  ],
  "type": "posts"
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `page.title` | string | 页面标题 |
| `page.slug` | string | URL slug |
| `page.description` | string | 页面描述 |
| `page.url` | string | 站点相对 URL |
| `page.body_html` | string | Markdown 渲染后的 HTML 正文 |
| `page.date` | string | 原始日期，非日期内容为空 |
| `page.iso_date` | string | ISO 日期，非日期内容为空 |
| `page.short_date` | string | 短日期，非日期内容为空 |
| `page.long_date` | string | 长日期，非日期内容为空 |
| `page.year` | string | 年份，非日期内容为空 |
| `page.tags` | string[] | `tags` taxonomy 的便捷字段 |
| `page.taxonomies` | object | 所有 taxonomy 字段到 term 列表的映射 |
| `page.aliases` | string[] | 规范化后的 alias URL |
| `page.extra` | object | 未识别 frontmatter 字段 |
| `page.headings` | object[] | Markdown 标题列表 |
| `page.toc` | object[] | `page.headings` 的别名 |
| `page.type` | string | 所属 collection 名称 |

`page.body_html` 应使用 `safe` 渲染：

```tera
{{ page.body_html | safe }}
```

---

## Section 模板变量

`section.html` 接收 `section`。section 来自 collection 目录中的 `_index.md`。

```json
{
  "title": "Documentation",
  "slug": "docs",
  "url": "/docs/",
  "pages": [],
  "breadcrumb": [
    { "title": "Documentation", "url": "/docs/" }
  ],
  "parent": null,
  "children": [
    { "title": "Reference", "slug": "docs/reference", "url": "/docs/reference/" }
  ]
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `section.title` | string | `_index.md` 标题 |
| `section.slug` | string | section slug |
| `section.url` | string | section URL |
| `section.pages` | object[] | 当前 section 下的内容列表，启用分页时为当前页 slice |
| `section.breadcrumb` | object[] | 面包屑项 |
| `section.parent` | object/null | 父 section |
| `section.children` | object[] | 子 section，按 `weight`、标题、URL 排序 |

---

## Taxonomy 模板变量

`taxonomy.html` 接收 `taxonomy`。

```json
{
  "name": "tags",
  "slug": "tags",
  "terms": [
    { "name": "Rust", "slug": "rust", "url": "/tags/rust/" }
  ]
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `taxonomy.name` | string | taxonomy 名称，对应 frontmatter 字段 |
| `taxonomy.slug` | string | taxonomy URL segment |
| `taxonomy.terms` | object[] | term 列表 |

---

## Term 模板变量

`term.html` 或自定义 term template 接收 `term`。

```json
{
  "name": "Rust",
  "slug": "rust",
  "url": "/tags/rust/",
  "taxonomy": "tags",
  "pages": []
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `term.name` | string | term 原始名称 |
| `term.slug` | string | term slug |
| `term.url` | string | term URL |
| `term.taxonomy` | string | 所属 taxonomy 名称 |
| `term.pages` | object[] | 当前 term 下的内容列表，启用分页时为当前页 slice |

---

## 404 模板变量

`404.html` 只接收全局变量。渲染后会被 `layout.html` 包装，layout 中的 `title` 为 `Page Not Found`，`description` 为空，`og_type` 为 `website`。

---

## 构建产物

构建完成后，输出目录通常包含：

| 文件 | 说明 |
|---|---|
| `index.html` | 首页 |
| `{collection}/{slug}/index.html` | 内容页 |
| section 和 taxonomy 输出 | 如 `/docs/`、`/tags/`、`/tags/rust/` |
| `assets/styles.{hash}.css` | 主 stylesheet |
| `asset_manifest.json` | public asset 映射 |
| `rss.xml` | RSS feed |
| `sitemap.xml` | XML sitemap |
| `robots.txt` | robots.txt |
| `_headers` | Netlify 风格缓存头 |
| `404.html` | 404 页面 |

---

## Markdown 渲染特性

kiln 的 Markdown 渲染支持：

- GFM table
- strikethrough
- autolink
- task list
- superscript
- footnotes
- description lists

额外处理：

- 所有 heading 自动添加 `id` 属性和 `#` 锚点链接
- `<table>` 自动包裹 `<div class="table-scroll">`
- task list checkbox 添加 `.task-list-item-checkbox` class
