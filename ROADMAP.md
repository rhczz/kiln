# kiln 项目路线图

> kiln 是一个用 Rust 编写的静态站点编译器。它的定位是：**稳定、可预测、工程化的静态站点编译器**。
>
> 不做框架运行时，不做插件平台，不做 JS runtime，不做 CMS，不做动态服务端。
> 让一个 Markdown 编译器保持为一个 Markdown 编译器。

## 核心管线

```
Markdown/content
        ↓
站点模型构建
        ↓
页面计划生成
        ↓
模板渲染
        ↓
静态产物输出
```

---

## 一、能力边界

### 支持的核心能力

| 模块 | 边界 |
|---|---|
| 内容模型 | Markdown + frontmatter + collection + section |
| 页面类型 | 单页、列表页、section 页、taxonomy 页、分页页、404、首页 |
| 模板系统 | layout、page template、partials、shortcodes |
| 站点上下文 | site、page、section、taxonomy、paginator |
| 静态资源 | copy、hash、CSS/JS 原样处理、可选压缩 |
| SEO | sitemap、RSS、robots、canonical、OpenGraph |
| 构建系统 | full build、增量 build、cache、manifest |
| Dev server | watch、debounce、增量重建、错误保留 |
| 可观测性 | timing、profile、错误定位、verbose log |
| 工程质量 | golden tests、benchmark、fixture、兼容性测试 |

### 明确不支持

| 不做 | 理由 |
|---|---|
| 插件系统 | 复杂度巨大，早期收益低 |
| JS/TS runtime | 会把 SSG 拖成前端框架 |
| 服务端渲染 | 和静态编译器定位冲突 |
| CMS 后台 | 不是这个产品的战场 |
| 数据库内容源 | 复杂度会爆炸 |
| 远程构建缓存 | 后期再说 |
| 分布式构建 | 20 万页以后才有意义 |
| 主题市场 | 生态问题，不是核心编译器问题 |
| Hydration / island | 这是另一个产品 |

---

## 二、功能缺口与数据模型

### 1. 站点级内容模型

当前实现以 `Vec<ContentItem>` 为主，缺少结构化的站点模型。需要：

```rust
SiteModel {
    pages,       // 所有页面
    sections,    // section 树
    collections, // 命名集合
    taxonomies,  // 分类体系
    menus,       // 导航菜单
    assets,      // 静态资源
}
```

**核心类型缺口**：`Site`、`Page`、`Section`、`Collection`、`Taxonomy`、`Term`、`Menu`、`Asset`、`BuildManifest`

### 2. 页面类型系统

当前主要是内容详情页。必须补齐的页面种类：

`home` | `single` | `list` | `section` | `taxonomy` | `term` | `pagination` | `404` | `rss` | `sitemap` | `robots`

**缺口**：`PageKind` 枚举、`RenderPlan`、`OutputPath` 规则、`TemplateResolution` 查找优先级

### 3. Section 层级

复杂文档站必须有嵌套 section：

```
/docs/
/docs/k8s/
/docs/k8s/network/
```

**缺口**：内容树、父子 section 关系、weight 排序、breadcrumb、section index page、sibling prev/next

### 4. Taxonomy

需要稳定的 taxonomy graph，支持用户配置 taxonomy 名称（tags、categories、series、authors 等）。

**缺口**：taxonomy 配置、term 聚合、term 页面、taxonomy index 页、term RSS（可选）、URL 规则

### 5. Pagination

核心能力。列表页到 1 万篇没有分页不可用。

**缺口**：paginator model、page size 配置、page number URL 生成、首页分页、section 分页、taxonomy term 分页

### 6. 模板基础设施

当前有 Tera，需要加固工程约定：

**缺口**：partial 约定、shortcode 约定、模板查找优先级、默认模板 fallback、模板错误栈、template context 文档化

### 7. Shortcode

Markdown 里必须能写 `{{< note >}}`、`{{< figure src="..." >}}`、`{{< youtube id="..." >}}`，否则复杂内容会把 HTML 塞进 Markdown。

**缺口**：block shortcode、inline shortcode、参数解析、shortcode 模板、错误定位

### 8. TOC / Heading Index

文档主题必需。当前只给 heading 加 anchor，缺少结构化 TOC 数据。

**缺口**：heading list（level / id / text）、TOC context、per-page TOC、sidebar 可用结构

### 9. Menu

**缺口**：menu 配置、active state、nested menu、weight 排序、external/internal link 区分

### 10. Asset 基础能力

不做 webpack，但基础必须有。

**缺口**：asset hash manifest、`asset_url(path)` helper、CSS/JS fingerprint、图片 copy/hash、stale asset prune

暂不做：SCSS 编译、TS bundling、image transform

### 11. Build Manifest

工程化核心。需要追踪：哪个 source 生成哪个 output、output hash、依赖哪些模板和内容。

```json
{
  "source": "content/posts/a.md",
  "outputs": ["dist/posts/a/index.html"],
  "content_hash": "...",
  "template_hash": "...",
  "dependencies": []
}
```

没有 manifest，增量构建和清理迟早不可靠。

### 12. 增量构建

最低要求的重建规则：

| 变更 | 应重建 |
|---|---|
| 单篇文章 | 当前 page + 相关 list/taxonomy/rss/sitemap |
| layout | 所有 HTML |
| partial | 依赖它的页面，做不到则所有 HTML |
| config | 全量 |
| public asset | 对应 asset |
| style | 引用它的页面或 asset manifest |

**缺口**：content hash、template hash、config hash、dependency invalidation、stale output prune、manifest cache

### 13. 大规模 Sitemap

必须分片。

**缺口**：sitemap index、sitemap chunk、最大 URL 数限制、最大文件大小限制、只包含 canonical URL

### 14. RSS / Feed 模型

**缺口**：feed config per collection、section feed、taxonomy feed（可选）、feed item limit、updated date、escaped content policy

### 15. 错误诊断

产品级错误信息要求：

```
content/posts/a.md:12: frontmatter field `date` invalid
templates/post.html:8: variable `page.foo` not found
```

**缺口**：文件路径 + 行号 + 错误类型、模板栈、建议修复、`kiln check` 命令

### 16. Build Timing / Profile

**缺口**：阶段耗时、文件/页面/输出数量、cache hit/miss、peak memory（可选）、`--profile` 参数

### 17. 测试体系

**缺口**：fixture site tests、golden output tests、large site benchmark、template resolution tests、taxonomy/pagination tests、Windows path tests、snapshot diff

---

## 三、版本路线

### v0 — 当前能力整理

**目标：不扩张，先稳。**

- [ ] 梳理 build pipeline，明确各阶段职责
- [ ] 加 build timing 输出
- [ ] 加 fixture tests（最小站点、多内容站点）
- [ ] 加 benchmark generator
- [ ] 明确 config schema，文档化所有配置项
- [ ] 文档化模板 context（site / page / collection 各字段）

### v1 — 最小产品级 SSG

**这是核心产品边界。必须包含：**

- [ ] `SiteModel` 结构化站点模型
- [ ] `PageKind` + `RenderPlan` 页面类型系统
- [ ] Section tree（父子关系、breadcrumb、prev/next）
- [ ] Taxonomy graph（配置化 taxonomy、term 聚合、term 页面）
- [ ] Pagination（paginator model、分页 URL、多种列表分页）
- [ ] Template lookup order + partials 约定
- [ ] Shortcodes（block / inline / 参数解析 / 模板）
- [ ] TOC / heading index
- [ ] Menu model
- [ ] Sitemap split（分片、限制）
- [ ] Build manifest
- [ ] `kiln check` 命令

### v2 — 工程化增强

- [ ] 并行 render
- [ ] 稳定增量构建（content/template/config hash + dependency invalidation）
- [ ] Asset fingerprint + manifest
- [ ] Template dependency tracking
- [ ] Dev server 错误 overlay 或清晰错误页
- [ ] Build profile（`--profile`）
- [ ] Golden output tests + snapshot diff
- [ ] Large site benchmark

### v3 — 生态增强（仍不做运行时）

- [ ] Theme directory 约定
- [ ] Theme override 机制
- [ ] Starter templates
- [ ] 更丰富的 default theme
- [ ] 更多内置 shortcodes
- [ ] 更完善的文档

---

## 四、不要做的"伪核心能力"

以下功能很诱人，但现在别碰。它们会把项目拖进复杂度泥潭：

- plugin system / WASM plugins
- JS runtime / React/Vue components / MDX
- remote content source / database source
- admin UI / visual editor
- distributed build
- AI content generation

**原则**：不要让一个 Markdown 编译器长成小型联合国，最后谁都管不了。

---

## 五、缺口优先级排序

```
P0 — v1 阻塞项，无则不可用：
  1. SiteModel
  2. PageKind / RenderPlan
  3. Section tree
  4. Taxonomy graph
  5. Pagination
  6. Template lookup / partials

P1 — v1 必须有，影响内容质量和站点完整性：
  7. Shortcodes
  8. TOC / heading index
  9. Menu model
  10. Build manifest
  11. Error diagnostics + kiln check

P2 — v1 加分项或 v2 前置：
  12. Asset manifest / fingerprint
  13. Incremental invalidation
  14. Sitemap split
  15. Feed model 增强
  16. Build timing / profile

P3 — 工程质量，贯穿全程：
  17. Fixture / golden tests
  18. Benchmark generator
  19. Parallel rendering
  20. Theme override boundary
```

---

*比玩具强很多，但不把自己做成操作系统。这才是一个长期可维护的静态站点编译器该有的样子。*
