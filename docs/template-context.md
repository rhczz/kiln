# kiln 模板上下文参考

模板引擎使用 [Tera](https://tera.netlify.app/)，语法类似 Jinja2/Django。

内置四个默认模板，位于 `src/defaults/`，可被 `templates/` 目录下的同名文件覆盖：
- `layout.html` — 全局布局（doctype、head、header、footer）
- `home.html` — 首页内容
- `post.html` — 文章详情页
- `page.html` — 普通页面

---

## 全局变量

所有模板（layout、home、post、page）都可访问以下变量。

### `site` — 站点信息

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
| `site.subtitle` | string | 站点副标题（可能为空） |
| `site.description` | string | 站点描述 |
| `site.language` | string | 语言代码 |
| `site.base_url` | string | 站点根 URL（无尾部斜杠） |
| `site.stylesheet_href` | string | 带内容 hash 的 CSS 路径，如 `/assets/styles.a1b2c3.css` |
| `site.author.name` | string | 作者姓名 |
| `site.author.email` | string | 作者邮箱 |

### `theme` — 用户自定义配置

配置文件中非标准表的内容，原样透传。结构完全由用户定义。

```toml
# site.config.toml
intro = "Welcome"
email = "hi@example.com"

[[nav]]
label = "Home"
href = "/"

[[footer_links]]
label = "GitHub"
href = "https://github.com/example"

[home_image]
enabled = true
src = "/hero.jpg"
alt = "Hero"
width = 800
height = 400
```

模板中使用：`{{ theme.intro }}`、`{% for item in theme.nav %}`。

### `config` — 向后兼容合并变量

`config` 是 `site` 和 `theme` 的合并体，用于向后兼容。新模板应优先使用 `site` 和 `theme`。

---

## Layout 专用变量

`layout.html` 负责完整的 HTML 骨架。除了全局变量，还接收：

| 变量 | 类型 | 说明 |
|---|---|---|
| `title` | string | 当前页面标题（用于 `<title>` 标签，可能为空） |
| `description` | string | 当前页面描述 |
| `body` | string (safe HTML) | 页面主体内容（由子模板渲染） |
| `path` | string | 当前页面路径（用于 canonical URL） |
| `og_type` | string | OpenGraph 类型，`"website"` 或 `"article"` |

典型用法：
```html
<title>{% if title %}{{ title }} · {{ site.title }}{% else %}{{ site.title }}{% endif %}</title>
<meta property="og:type" content="{{ og_type }}">
<link rel="canonical" href="{{ site.base_url }}/{% if path %}{{ path }}/{% endif %}">
```

---

## Home 模板变量

`home.html` 除了全局变量，还接收：

### `featured_posts` — 推荐文章列表

最多 6 篇标记为 `featured: true` 的文章，按日期降序。

```json
[
  {
    "title": "Featured Post",
    "slug": "featured-post",
    "date": "2026-06-01",
    "iso_date": "2026-06-01",
    "short_date": "2026.06.01",
    "long_date": "June 1, 2026",
    "year": "2026",
    "description": "Post description",
    "url": "/posts/featured-post/"
  }
]
```

### `archive` — 年度归档

所有日期排序文章按年分组，年份降序。

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

典型用法：
```html
{% for year_entry in archive %}
<h3>{{ year_entry.year }}</h3>
<ul>
  {% for post in year_entry.posts %}
  <li><a href="{{ post.url }}">{{ post.title }}</a></li>
  {% endfor %}
</ul>
{% endfor %}
```

---

## Post / Page 模板变量

`post.html` 和 `page.html` 除了全局变量，还接收：

### `page` — 当前页面数据

Post（date_ordered 集合）：

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
  "type": "posts"
}
```

Page（非 date_ordered 集合）：

```json
{
  "title": "About",
  "slug": "about",
  "description": "About page",
  "url": "/about/",
  "body_html": "<p>Rendered HTML content</p>",
  "date": "",
  "iso_date": "",
  "short_date": "",
  "long_date": "",
  "year": "",
  "tags": [],
  "type": "pages"
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `page.title` | string | 页面标题 |
| `page.slug` | string | URL slug |
| `page.description` | string | 页面描述 |
| `page.url` | string | 相对 URL 路径 |
| `page.body_html` | string (safe HTML) | Markdown 渲染后的 HTML 正文 |
| `page.date` | string | 原始日期字符串（`YYYY-MM-DD`），无日期时为空 |
| `page.iso_date` | string | ISO 格式日期（用于 `<time datetime>`），无日期时为空 |
| `page.short_date` | string | 短格式日期（`YYYY.MM.DD`），无日期时为空 |
| `page.long_date` | string | 长格式日期（`June 1, 2026`），无日期时为空 |
| `page.year` | string | 年份字符串，无日期时为空 |
| `page.tags` | string[] | 标签列表 |
| `page.type` | string | 所属集合名称（`"posts"` / `"pages"` / 自定义） |

典型用法：
```html
<article>
  <h1>{{ page.title }}</h1>
  {% if page.iso_date %}
  <time datetime="{{ page.iso_date }}">{{ page.long_date }}</time>
  {% endif %}
  {% if page.tags %}
  <div>{% for tag in page.tags %}<span>{{ tag }}</span>{% endfor %}</div>
  {% endif %}
  <div>{{ page.body_html | safe }}</div>
</article>
```

---

## 构建产物

构建完成后，输出目录包含：

| 文件 | 说明 |
|---|---|
| `index.html` | 首页 |
| `{collection}/{slug}/index.html` | 各内容页 |
| `assets/styles.{hash}.css` | 带内容 hash 的样式表 |
| `rss.xml` | RSS feed |
| `sitemap.xml` | XML sitemap |
| `robots.txt` | robots.txt（指向 sitemap） |
| `_headers` | Netlify 风格缓存头（CSS immutable cache） |
| `public/` 的内容 | 原样复制的静态资源 |
| `404.html` | 来自 public 目录的 404 页面（如果有） |

---

## Markdown 渲染特性

kiln 的 Markdown 渲染支持以下扩展：

- **GFM table** — 表格
- **Strikethrough** — `~~删除线~~`
- **Autolink** — URL 自动链接
- **Task list** — `- [ ] todo` / `- [x] done`
- **Superscript** — `^上标^`
- **Footnotes** — `[^1]` 脚注
- **Description lists** — 描述列表

额外处理：
- 所有 heading 自动添加 `id` 属性和 `#` 锚点链接
- `<table>` 自动包裹 `<div class="table-scroll">`
- Task list checkbox 添加 `.task-list-item-checkbox` class
