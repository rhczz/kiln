# kiln 构建模型

本文档说明 `kiln build`、`kiln check`、`kiln doctor`、`kiln clean` 和 `kiln serve` 的构建语义、缓存边界、落盘状态和增量重建行为。

`kiln` 的构建模型可以概括为：

```text
content + config + templates + public assets + styles.css -> output directory
```

输出目录默认是 `dist/`。除非特别说明，下面的路径都以站点配置中的路径为准。

## 命令语义

### `kiln build`

`kiln build` 是一次性编译命令。

每次执行时，`kiln` 会：

1. 读取 `site.config.toml`
2. 初始化模板引擎和样式资源
3. 在 full build 模式下清空整个输出目录
4. 复制并 fingerprint `public/` 中的静态资源
5. 读取并解析 Markdown 内容
6. 构建站点模型
7. 渲染页面、RSS、sitemap、robots.txt 和主样式资源
8. 写入 build manifest 和 asset manifest

`kiln build` 不复用上一次进程中的 render cache，因为进程在构建结束后退出。输出目录中的 `.kiln/manifest.json` 会被写入，但它不是跨进程 HTML render cache。

使用 `--profile` 时，`build` 会在本次进程内启用 `BuildCache` 来统计 cache hit/miss 和 render 指标。这个 cache 仍然只存在于本次命令进程中，不会保存到磁盘。

### `kiln check`

`kiln check` 是一次临时输出目录中的 dry build。

它会读取配置、加载模板和样式、解析内容，并执行一次 `Full` build，但输出写入系统临时目录，命令结束后临时目录会被删除。它不会写入用户配置的 `dist/`，也不会复用或更新 `dist/.kiln/manifest.json`。

### `kiln doctor`

`kiln doctor` 是项目健康检查命令。

它会检查：

- config 是否存在且可加载
- `base_url` 是否仍是示例地址或使用 `http://`
- content、styles、templates、public 路径状态
- 模板和 stylesheet 是否能加载
- content route 是否冲突
- public asset 是否存在大小写不敏感冲突
- 临时目录 dry build 是否成功

和 `check` 一样，`doctor` 的 dry build 写入系统临时目录，不写入用户配置的 `dist/`。

### `kiln clean`

`kiln clean` 清理输出目录，但默认保留 `.kiln` 状态目录。

- `kiln clean --output dist` 会删除 `dist/` 下除 `.kiln/` 以外的生成文件和目录。
- `kiln clean --output dist --cache` 只删除 `dist/.kiln/`。

`clean` 会拒绝清理 `/`、当前工作目录、用户 home 目录、包含 `..` 的路径，以及非目录类型的输出路径。

### `kiln serve`

`kiln serve` 是长生命周期开发服务器。

启动时，`serve` 会先执行一次 full build，然后开始监听：

- content 目录
- templates 目录
- public 目录
- styles 文件
- config 文件

`serve` 会在同一个进程中持有内存 `BuildCache`。后续文件变化触发 rebuild 时，内容解析结果、页面 HTML render 结果和部分 public asset 状态可以在这个进程内复用。

如果 rebuild 失败，服务器会继续提供最后一次成功构建的输出。

## 构建模式

内部构建有三种模式：

| 模式 | 入口 | 语义 |
|---|---|---|
| `Full` | `kiln build`、`kiln check`、`kiln doctor` dry build、`kiln serve` 初始构建、配置/样式/模板等变化 | 准备输出目录、处理 public assets、重新加载内容、重新计算站点模型、渲染页面，并写入样式、feeds、sitemap、robots、manifest |
| `Content` | `serve` 中只有 content 变化 | 跳过 public asset fingerprint，加载已有 asset manifest，重新加载内容，重新计算站点模型并渲染受 hash 影响的页面 |
| `Public` | `serve` 中 public asset 变化 | 重新 fingerprint public assets，更新 asset manifest，重新加载内容，重新计算站点模型，并用新的 asset hash 参与页面 render key |

`Full` 在普通 `build`、`check`、`doctor` 和 `serve` 初始构建中会删除目标输出目录后重建。`serve` 中触发的 `Full` rebuild 也使用同一套 full build 路径，但仍可利用当前进程内没有被清掉的 render cache；具体是否命中取决于 rebuild 类型和 render key 是否一致。

主 `styles.css` 不属于 `public/` asset manifest。它会被单独读入、按内容 hash 写到 `assets/styles.<hash>.css`，并通过模板上下文中的 `site.stylesheet_href` 暴露。`Full` 和 `Public` 模式都会写主样式资源；`_headers` 中的 immutable CSS cache 规则只在 `Full` 模式写入。

## Cache 和 Manifest

### 内存 cache

`BuildCache` 只存在于当前进程内，主要用于 `serve`。

它缓存：

- Markdown 解析结果，key 为内容源文件路径和源文件内容 hash
- 单页 HTML render 结果，key 为内容源文件路径和 render hash
- 首页、section、taxonomy、term、pagination、404 等非单篇页面的 HTML render 结果，key 为逻辑 URL 和 render hash
- public asset 的 hash 和输出路径集合；当前主要用于状态记录和 profile 统计
- 上一次页面输出路径集合，用于删除本轮不再生成的页面输出

页面 render hash 由四类输入共同组成：

```text
content hash + template deps hash + config hash + asset manifest hash
```

其中：

- content hash 来自 Markdown 原文；非单篇页面会根据其依赖的内容集合和页面 URL 生成逻辑 hash
- template deps hash 来自页面模板、`layout.html`、include/import/extends 依赖和 shortcode 模板
- config hash 来自核心站点、collection、taxonomy 和 pagination 配置字段
- asset manifest hash 来自 `asset_manifest.json` 中稳定排序后的映射内容

如果 cache 中已有条目但 render hash 不一致，该条目会被视为 stale miss，而不是 hit。

### Build manifest

`dist/.kiln/manifest.json` 是 build manifest。

它记录：

- source 路径
- source 产生的 output 路径
- content hash
- template dependency 列表
- template dependency hash
- config hash

它的职责是记录“哪些输入生成了哪些输出”和“页面依赖哪些模板”。当前实现中，它主要服务于模板依赖判断、输出可观测性和后续清理/增量能力的基础数据。

它不保存渲染后的 HTML，不等价于跨进程 render cache。重新运行 `kiln build` 时，HTML 页面仍会重新生成。

### Asset manifest

`dist/asset_manifest.json` 是 asset manifest。

它记录：

```json
{
  "mappings": {
    "css/site.css": "css/site.<hash>.css",
    "images/logo.png": "images/logo.<hash>.png",
    "data.json": "data.json"
  }
}
```

它的职责是：

- 给模板中的 `asset_url(...)` 提供原始路径到输出路径的映射
- 给 CSS `url(...)` rewrite 提供 fingerprint 后的目标路径
- 生成 asset manifest hash，使 public asset 变化可以影响页面 render key
- 在后续构建中识别并清理 stale asset 输出
- 按 key 排序写出 JSON，减少无意义 diff

asset manifest 也不是 render cache。

## 文件变化行为

`serve` 中的文件变化会先合并 debounce，再分类为 content、public 或 full rebuild。

| 变化类型 | Rebuild 行为 | Cache 行为 |
|---|---|---|
| 只改 content | `Content` rebuild | 复用 public asset manifest；内容 hash 改变的页面 miss，未受影响且 render key 相同的页面 hit |
| 只改 public asset | `Public` rebuild | 重新生成 asset manifest，并重新写主样式资源；asset manifest hash 改变后，页面 render key 会变化 |
| content 和 public 在同一次 watcher 事件中变化 | `Full` rebuild | 使用 full build 流程，避免 content 和 asset 状态分叉 |
| content 和 public 在 debounce 窗口内分多次事件到达 | `Public` rebuild | `Public` 模式仍会重新加载内容并渲染页面，所以可以吸收该窗口内的 content 变化 |
| 改 template | `Full` rebuild，带 changed template 列表 | 重新加载模板；依赖变更模板的单篇页面 render cache 被选择性清理；generic pages 保守清理 |
| 改 config | `Full` rebuild | 清空 render cache，因为配置可能改变路由、集合、taxonomy、分页或模板上下文 |
| 改 styles 文件 | `Full` rebuild | 清空 render cache，并重新写入主样式资源 |
| 未识别路径变化 | `Full` rebuild | 保守处理 |

模板变更时，`serve` 会读取上一轮 build manifest 中记录的 template deps。单篇页面如果依赖被修改的模板，会被从 render cache 中移除。首页、section、taxonomy、term、pagination、404 等 generic render cache 当前没有精细 dependency map，因此模板变化时会保守清空 generic render cache。

## 输出清理

### Full build 输出目录

普通 `kiln build` 的 full build 会删除整个输出目录，然后重新生成所有输出。

这意味着：

- 旧页面输出不会残留
- 旧 fingerprint asset 不会残留
- 用户手动放在输出目录里的文件也会被删除

输出目录应视为构建产物目录，不应作为源码目录使用。

`kiln check` 和 `kiln doctor` 也运行 full build 路径，但目标是临时目录，因此不会清理配置中的输出目录。

`kiln clean --output dist` 与 full build 不同：它只删除 `dist/` 下除 `.kiln/` 以外的内容。需要删除 `.kiln` 状态时使用 `kiln clean --output dist --cache`。

### Serve 中的页面输出

`serve` 的 rebuild 会维护上一轮页面输出集合。

当本轮不再生成某个页面输出时，`kiln` 会删除对应文件，并尝试清理空目录。典型场景包括：

- 删除 Markdown 文件
- 修改 slug 或 route
- 修改 collection 配置导致输出路径变化
- taxonomy、section 或 pagination 页数减少

### Public asset 输出

public asset 有两类清理：

- fingerprint asset：如果文件名看起来像 `name.<12-hex>.ext`，且不在当前 asset manifest 中，会被删除
- 非 fingerprint asset：如果上一轮 asset manifest 记录过某个原始路径，而本轮不再存在，会删除输出目录中的同名文件

`kiln` 只主动清理它能识别为自己生成或曾由 manifest 记录的 public asset 输出。

主 `styles.css` 生成的 `assets/styles.<hash>.css` 不属于 `public/` asset manifest；普通 full build 通过清空输出目录移除旧样式文件，`clean` 也会删除它。

## 稳定性规则

`kiln` 的目标是同样输入产生同样输出。

构建稳定性依赖以下规则：

- 页面 render key 包含 content、template、config 和 asset 四类输入
- asset manifest hash 和 asset manifest JSON 输出都使用按 key 排序后的映射内容
- RSS 条目按日期降序排序，并用 URL 作为同日期时的稳定 tie-breaker
- sitemap、robots.txt 和 build manifest 在每次成功构建后重新写入
- cached HTML 命中时仍会通过 `write_page_if_changed` 写入目标路径，确保输出文件存在

当前测试已经覆盖 parallel render determinism、build manifest snapshot、asset manifest roundtrip、CSS url rewrite、sitemap stale output prune、content incremental update，以及基础博客、多 collection、taxonomy/pagination、public asset、template/shortcode 等 clean build 与 incremental rebuild bit-for-bit 一致的 fixture。后续新增构建能力时，应继续保证 clean build 和增量 rebuild 的输出一致。

## 边界和非目标

当前构建模型刻意保持简单：

- 不提供跨进程 HTML render cache
- 不把 `.kiln/manifest.json` 当作可恢复的构建数据库
- 不做插件级别的外部 build graph
- 不内置前端 bundler
- 不在输出目录之外写构建状态

如果需要确认一次构建完全不依赖旧状态，可以删除输出目录后运行：

```bash
kiln build --config site/site.config.toml --output dist
```

如果需要观察当前进程内 cache 行为，可以运行：

```bash
kiln build --config site/site.config.toml --output dist --profile
```

需要注意的是，`--profile` 展示的是本次命令进程内的 cache 活动，不表示上一次 `kiln build` 的 HTML 被复用了。
