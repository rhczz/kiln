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

## 二、真实能力状态

ROADMAP 以当前代码和文档为准，不再把已经落地的能力继续列成纯缺口。

### 已实现或已有初版

| 能力 | 当前状态 |
|---|---|
| `SiteModel` | 已有结构化站点模型，包含页面、section、collection、taxonomy、menu 等渲染输入 |
| `PageKind` | 已覆盖 single、alias、home、section、taxonomy index、term、pagination、404 等页面类型 |
| Section page | 已支持 collection/section index 页面、排序和上下文输出 |
| Taxonomy index / term page | 已支持配置化 taxonomy、term 聚合、taxonomy index 与 term 页面 |
| Pagination | 已支持首页、section/list、term 等列表分页模型 |
| TOC / heading index | Markdown 渲染产出 heading list，模板上下文暴露 `page.headings` / `page.toc` |
| Menu config / menu context | 配置可定义 menu，模板上下文可读取站点 menu |
| Template lookup order | 已有默认模板、用户模板覆盖、collection/taxonomy 相关模板查找 |
| Shortcodes | 已支持 inline/block shortcode、参数解析和 shortcode template 初版 |
| Sitemap split | 已支持 sitemap index、分片、URL 数量和文件大小限制 |
| `BuildManifest` | 已记录 source、outputs、hash、template dependencies 等构建产物信息 |
| Template dependency tracking | 已有 `{% extends %}`、`include`、`import`、`from` 的静态依赖追踪初版 |
| Asset fingerprint + manifest | 已支持 public asset 指纹化、manifest、CSS `url(...)` 重写和 stale asset prune |
| `asset_url()` | 已作为 Tera 函数暴露给模板 |
| CSS `url(...)` suffix preservation | 已保留 query string 和 fragment，并有回归测试 |
| `kiln check` | 已能在临时输出目录中验证配置和内容，不写用户 `dist/` |
| `kiln doctor` | 已检查项目结构、配置、内容、模板、路由和资源，并输出修复建议 |
| `--profile` / `--profile-json` | 已支持人类可读和机器可读构建 profile |
| Parallel rendering | 已支持并行页面渲染，并在 profile 中暴露相关统计 |
| Large site benchmark | 已有 benchmark generator 和性能基线文档 |
| 测试体系 | 已有 CLI、fixture、example、snapshot、unit tests 和 clean/incremental parity 覆盖 |

### 已实现但还不够发布级

这些不是“从零实现”的功能，而是下一阶段必须硬化的工程边界。

| 能力 | 需要硬化到什么程度 |
|---|---|
| build/output 原子性 | 构建失败后上一版输出 byte-for-byte 保持可用；所有 destructive output 操作走统一安全校验 |
| serve 失败态 | 任意 rebuild 失败后继续 serve 上一版成功输出，修复错误后自动恢复新版本 |
| dependency invalidation | config、template、content、asset、shortcode、feed、sitemap、taxonomy/list 派生页都进入可信依赖图 |
| template dependency graph | 动态 include/import 有明确策略；无法静态判断时保守全量 invalidation 或显式报错 |
| shortcode 错误定位 | malformed、missing close、unknown template、invalid params、render error 都带文件和 span |
| feed model | 支持 per collection/section/taxonomy feed、updated date、author/category/canonical、内容策略和 limit |
| `check` / `doctor` / `profile` 发布门禁 | `check` 可作为 CI gate；`profile-json` schema 文档化并有 golden/snapshot 保护 |

### 后置生态增强

这些不是当前瓶颈，后续只在可靠性和依赖图收口后再考虑。

- Theme directory 约定
- Theme override 机制
- Starter templates
- 更丰富的 default theme
- 更多内置 shortcodes
- 更完整的教程和迁移文档

---

## 三、版本路线

### v0 — 当前能力整理（已完成）

**目标：不扩张，先稳。**

- [x] 梳理 build pipeline，明确各阶段职责
- [x] 加 build timing 输出
- [x] 加 fixture tests（最小站点、多内容站点）
- [x] 加 benchmark generator
- [x] 明确 config schema，文档化所有配置项
- [x] 文档化模板 context（site / page / collection 各字段）

### v1 — 可靠、可发布、可诊断

**目标：构建结果可信，失败不破坏上一版，错误能定位，命令能进 CI。**

- [ ] 统一 output safety：build、serve、clean、asset prune、manifest save、generated files 都经过同一套路径保护
  - 验收：拒绝 root/cwd/home/config/content/templates/public/styles 重叠路径、symlink 指向受保护路径、受保护路径父子目录
- [ ] Transactional build output：构建写入同 filesystem staging，成功后 swap 到 live output
  - 验收：构建失败后原 `dist` byte-for-byte 保持可用；staging/backup 只能删除带 kiln marker 的目录
- [ ] Dev server 失败态：记录 last successful build、last error、rebuilding、stale-but-serving
  - 验收：模板/config/shortcode/frontmatter 错误后 HTTP 仍返回上一版成功输出，修复后自动恢复
- [ ] Shortcode diagnostics：parser 返回 source span，所有 shortcode 错误带文件、行列和原因
  - 验收：malformed、missing close、unknown template、invalid params、nested/same-name block 策略都有测试
- [ ] `check` / `doctor` 发布门禁
  - 验收：`kiln check` 不写用户输出目录，覆盖 config/content/routes/templates/shortcodes/aliases/taxonomy/feed/sitemap 的可定位错误
- [ ] Profile JSON schema 固化
  - 验收：schema 文档化，profile JSON golden/snapshot 测试保护字段兼容性
- [ ] 核心回归测试矩阵
  - 验收：output safety、serve failure、shortcode diagnostics、profile JSON、clean/incremental parity 都有 fixture 或 golden tests

### v2 — 稳定增量构建和依赖图

**目标：所有影响输出的输入都能解释为什么重建，增量结果与 clean build 一致。**

- [ ] render key 覆盖 content、template、config、asset manifest、style、shortcode template
  - 验收：修改 author/menu/theme/style/feed/taxonomy/pagination 等配置不需要手动 clean 也能得到正确页面
- [ ] template dependency graph 收紧
  - 验收：extends/include/import/from/default template/user override/shortcode template 变化能重建受影响页面；动态依赖有保守策略
- [ ] 派生页面依赖进入 manifest
  - 验收：首页、section、taxonomy、term、pagination、rss、sitemap、robots 能解释触发重建的输入
- [ ] public asset 变更影响引用它的 HTML 或触发相关页面重渲染
  - 验收：引用 asset 的模板输出与 asset manifest 保持一致
- [ ] Feed model 增强
  - 验收：支持 per collection feed、可选 section/term feed、updated date、author/category/canonical、内容策略和 limit

### v3 — 主题和生态增强（仍不做运行时）

**目标：在核心可靠性稳定后改善复用体验，不把 kiln 拖成前端框架。**

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
P0 — 发布级底线：
  1. Output safety
  2. Transactional build output
  3. Serve failure state
  4. Cache/output transaction consistency

P1 — 诊断和门禁：
  5. Shortcode diagnostics
  6. kiln check CI gate
  7. doctor actionable hints
  8. profile-json schema stability

P2 — 增量构建可信度：
  9. Config/template/style/asset/shortcode render key coverage
  10. Template dependency graph hardening
  11. Derived page dependency tracking
  12. Manifest explanation for rebuild decisions

P3 — 模型增强和生态：
  13. Feed model 增强
  14. Theme directory / override boundary
  15. Starter templates
  16. More default theme / shortcode docs
```

---

*比玩具强很多，但不把自己做成操作系统。这才是一个长期可维护的静态站点编译器该有的样子。*
