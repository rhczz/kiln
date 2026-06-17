# kiln 配置参考

配置文件为 `site.config.toml`，使用 TOML 格式。

## `[site]` — 必填

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `title` | string | 是 | — | 站点标题，显示在页面 title、RSS、模板中 |
| `subtitle` | string | 否 | `""` | 站点副标题 |
| `description` | string | 否 | `""` | 站点描述，用于 meta description 和 RSS |
| `language` | string | 否 | `"en"` | HTML lang 属性值 |
| `base_url` | string | 是 | — | 站点根 URL，必须以 `http://` 或 `https://` 开头，尾部斜杠会自动去除 |

```toml
[site]
title = "My Blog"
subtitle = "Thoughts and notes"
description = "A personal blog about technology"
language = "zh-CN"
base_url = "https://example.com"
```

## `[author]` — 可选

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `name` | string | 是 | — | 作者姓名，显示在页脚和 RSS |
| `email` | string | 否 | `""` | 作者邮箱 |

```toml
[author]
name = "Zhang San"
email = "zhang@example.com"
```

## `[feed]` — 可选

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `item_count` | integer | 否 | `20` | RSS feed 中包含的最大条目数 |

```toml
[feed]
item_count = 30
```

## `[paths]` — 可选

路径相对于 `site.config.toml` 所在目录。

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `content` | string | 否 | `"content"` | Markdown 内容目录 |
| `templates` | string | 否 | `"templates"` | Tera 模板目录 |
| `public` | string | 否 | `"public"` | 静态资源目录，原样复制到输出 |
| `styles` | string | 否 | `"styles.css"` | 主样式表文件路径 |

```toml
[paths]
content = "content"
templates = "templates"
public = "static"
styles = "assets/main.css"
```

**约束**：
- `styles` 必须是 site-relative 路径（不能绝对、不能含 `..`）
- `styles` 必须指向一个存在的 CSS 文件
- `content` 解析后必须指向一个存在的目录

## `[[collections]]` — 可选

默认提供两个集合：`posts`（日期排序，feed 开启）和 `pages`（无日期，无 feed）。如果用户指定了任何 collection，默认值会被完全替换。

默认集合等价于：

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

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `name` | string | 是 | — | 集合名称，用于模板选择和 URL 生成 |
| `directory` | string | 是 | — | content 目录下的子目录名 |
| `route` | string | 是 | — | URL 路由模板，必须以 `/` 开头和结尾，包含 `{slug}` |
| `template` | string | 否 | `"{name}.html"` | 渲染模板文件名 |
| `date_ordered` | boolean | 否 | `false` | 是否按日期排序，要求 frontmatter 有 `date` 字段 |
| `feed` | boolean | 否 | `false` | 该集合的内容是否出现在 RSS feed 中 |

```toml
[[collections]]
name = "posts"
directory = "posts"
route = "/posts/{slug}/"
date_ordered = true
feed = true

[[collections]]
name = "pages"
directory = "pages"
route = "/{slug}/"
```

**约束**：
- `name` 不能重复
- `name` 必须是单个非空 slug/path segment，不能含 `/`、`\`、`..`，不能以 `.` 开头，不能有首尾空白
- `directory` 不能重复
- `directory` 必须是 content-relative 路径（不能绝对、不能含 `..`）
- `route` 不能含 `..`
- `route` 必须是规范化 URL path：以 `/` 开头和结尾、包含 `{slug}`、不能含 `//` 或 `\`
- `date_ordered = true` 时，frontmatter 中的 `date` 字段必填
- build 会检测最终输出路径冲突；如果两个 collection 生成同一个 URL，会失败并报告两个来源文件

## `[[taxonomies]]` — 可选

默认 taxonomy 为 `tags`，slug 也是 `tags`，term 模板为 `term.html`。如果用户指定了任何 taxonomy，默认值会被完全替换。

```toml
[[taxonomies]]
name = "tags"
slug = "tags"
template = "term.html"

[[taxonomies]]
name = "categories"
slug = "categories"
template = "term.html"
```

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `name` | string | 是 | — | frontmatter 字段名，例如 `tags` 或 `categories` |
| `slug` | string | 否 | `name` | taxonomy index URL path segment，例如 `/tags/` |
| `template` | string | 否 | `"term.html"` | term 页面模板文件名 |

**约束**：
- `name` 不能重复
- `slug` 不能重复
- `name` 和 `slug` 都必须是单个非空 slug/path segment，不能含 `/`、`\`、`..`，不能以 `.` 开头，不能有首尾空白
- `slug` 不能和 collection route 的静态命名空间冲突；例如 taxonomy slug `tags` 不能与 collection route `/tags/{slug}/` 同时存在
- build 会检测 taxonomy index、term 页面、section 页面和 content 页面之间的最终输出路径冲突；例如 `/tags/` 页面和默认 tags taxonomy index 冲突时会失败

## 自定义表（theme 透传）

配置文件中未识别的顶层表会自动透传为 `theme` 模板变量。

```toml
# 这些会变成 theme.brand.intro, theme.brand.email 等
[brand]
intro = "Welcome to my blog"
email = "hi@example.com"

[[nav]]
label = "Home"
href = "/"

[[nav]]
label = "GitHub"
href = "https://github.com/example"

[home_image]
enabled = true
src = "/hero.jpg"
alt = "Hero image"
width = 800
height = 400

[[footer_links]]
label = "GitHub"
href = "https://github.com/example"
```

模板中使用：`{{ theme.brand.intro }}`、`{% for item in theme.nav %}` 等。

## Frontmatter 字段

每个 Markdown 文件的 frontmatter（YAML 格式，`---` 分隔）：

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `title` | string | 是 | — | 页面标题 |
| `date` | string | 条件 | — | 日期，格式 `YYYY-MM-DD`。`date_ordered` 集合中必填 |
| `description` | string | 否 | 自动提取 | 页面描述。留空则从正文前 200 字符自动生成 |
| `slug` | string | 否 | 文件名推导 | 自定义 URL slug。默认从文件名去除日期前缀 |
| `featured` | boolean | 否 | `false` | 是否为推荐内容（首页展示） |
| `draft` | boolean | 否 | `false` | 草稿标记，`--drafts` 参数才能包含 |
| `tags` | string[] | 否 | `[]` | 标签列表 |

```yaml
---
title: "My Post"
date: "2026-06-05"
description: "A brief summary"
slug: "custom-url"
featured: true
draft: false
tags: ["rust", "ssg"]
---
```

## CLI 参数

```
kiln build --config <path> --output <path> --drafts
kiln serve --config <path> --output <path> --port <port>
```

| 参数 | 默认值 | 说明 |
|---|---|---|
| `--config` | `site/site.config.toml` | 配置文件路径 |
| `--output` | `dist` | 输出目录 |
| `--drafts` | off | 包含草稿内容 |
| `--port` | `4173` | 开发服务器端口（仅 serve） |
