# kiln v2 — 工程化增强 SPEC

> 定位：在 v1 功能完备的基础上，提升构建性能、增量构建精度、可观测性和测试覆盖率。
> 不扩张功能边界，不做运行时、不做插件、不做生态。

---

## 目标

1. **并行渲染**：用 tokio 并行渲染页面，显著缩短大型站点构建时间
2. **稳定增量构建**：基于 content/template/config hash + 依赖图的精确重建
3. **Asset fingerprint + manifest**：静态资源哈希化和引用管理
4. **Template 依赖追踪**：追踪每个页面依赖哪些模板，模板变更时精确重建
5. **终端诊断增强**：彩色输出、源码上下文片段、修复建议
6. **Build profile**：`--profile` 参数输出详细构建分析
7. **Golden output tests**：用 insta crate 做 snapshot 测试

---

## 阶段划分

### Phase 1 — 工程基础设施

**目标：建立测试和可观测性基础，为后续阶段提供质量保障。**

#### 1.1 Golden output tests (insta)

- 引入 `insta` 依赖
- 对现有 fixture 测试的关键输出做 snapshot：
  - 渲染后的 HTML 页面
  - sitemap.xml
  - RSS feed
  - manifest.json
- 测试文件放在 `tests/snapshot_tests.rs`
- `cargo insta test` + `cargo insta review` 工作流

**验收标准**：
- [ ] 至少 5 个核心输出的 snapshot 测试
- [ ] snapshot 文件存储在 `snapshots/` 目录
- [ ] `cargo test` 包含 snapshot 测试且通过
- [ ] 现有 78 个测试全部通过，无回归

**涉及文件**：
- `Cargo.toml` — 添加 insta dev-dependency
- `tests/snapshot_tests.rs` — 新建

#### 1.2 Build profile (`--profile`)

- `CLI` 添加 `--profile` flag（仅在 `build` 子命令下）
- 开启后输出详细构建分析：
  - 各阶段耗时明细（已有 `BuildTimer`，需增强）
  - 内容统计：文件数、页面数、输出文件数
  - 缓存统计：cache hit / miss 数量
  - 模板统计：渲染次数、平均耗时
  - 内存峰值（可选，用 `std::alloc` 统计）

**验收标准**：
- [ ] `kiln build --profile` 输出详细的阶段耗时和缓存统计
- [ ] 不加 `--profile` 时行为与现在完全一致
- [ ] 现有测试通过

**涉及文件**：
- `src/cli.rs` — 添加 `--profile` flag
- `src/timing.rs` — 增强 `BuildTimer`，记录更多指标
- `src/site.rs` — 传递 profile 模式，收集统计

#### 1.3 终端诊断增强

- 彩色输出（error 红色、warning 黄色、hint 蓝色）
- 错误信息包含源码上下文片段（出错行 ±2 行）
- 错误位置用 `file:line:col` 格式输出，兼容 IDE 跳转
- 模板错误显示模板栈（哪个 partial 调用了出错的模板）
- 构建失败时输出摘要：X errors, Y warnings

**验收标准**：
- [ ] `Diagnostic` 输出包含 `file:line:col` 格式
- [ ] error/warning 有颜色区分（终端支持时）
- [ ] 错误信息显示源码上下文片段
- [ ] 构建结束输出 error/warning 计数摘要
- [ ] 现有测试通过

**涉及文件**：
- `src/diagnostic.rs` — 增强输出格式，添加颜色和源码上下文
- `src/site.rs` — 构建结束时收集并输出诊断摘要
- `src/engine.rs` — 模板错误收集模板栈信息

---

### Phase 2 — 依赖追踪与增量构建

**目标：建立精确的依赖图，实现模板/资源级别的增量重建。**

#### 2.1 Template 依赖追踪

- 在模板渲染时记录每个页面使用的模板链：
  - page template → layout → partials
- 扩展 `ManifestEntry` 添加 `template_deps: Vec<PathBuf>`
- 渲染后自动记录依赖关系到 manifest
- 提供 `template_dep_graph()` 查询：哪些页面依赖某个模板

**验收标准**：
- [ ] manifest 中每个 entry 记录 template_deps
- [ ] 模板变更时能查询受影响的页面列表
- [ ] 测试覆盖：模板变更 → 正确识别受影响页面

**涉及文件**：
- `src/manifest.rs` — `ManifestEntry` 添加 template_deps 字段
- `src/engine.rs` — 渲染时记录模板依赖链
- `src/site.rs` — 将依赖信息写入 manifest

#### 2.2 Asset fingerprint + manifest

- 对 `public/` 目录下的资源文件计算内容哈希
- 哈希写入文件名（如 `style.a1b2c3.css`）
- 生成 `asset_manifest.json`：原始路径 → 哈希后路径的映射
- 提供 Tera 全局函数 `asset_url(path)` 返回哈希后路径
- 清理旧版本的哈希文件

**验收标准**：
- [ ] `public/` 下的 CSS/JS/图片文件被 fingerprint
- [ ] `asset_manifest.json` 记录所有映射
- [ ] 模板中 `asset_url("style.css")` 返回哈希后路径
- [ ] 旧哈希文件被自动清理
- [ ] 测试覆盖

**涉及文件**：
- `src/site.rs` — asset fingerprint 逻辑
- `src/engine.rs` — 注册 `asset_url` 全局函数
- 新增 `src/asset.rs` 或在 `site.rs` 中处理

#### 2.3 稳定增量构建

- 基于依赖图的精确重建决策：
  - 内容文件变更 → 重建该页 + 依赖该内容的列表页/taxonomy/rss/sitemap
  - 模板变更 → 重建所有依赖该模板的页面
  - config 变更 → 全量重建
  - public asset 变更 → 重新 fingerprint 对应资源
- 三级哈希校验：content_hash + template_hash + config_hash
- 增量构建时跳过未变更的页面渲染
- stale output 自动清理

**验收标准**：
- [ ] 单篇文章修改 → 只重建相关页面，不重建无关页面
- [ ] partial 变更 → 重建依赖该 partial 的所有页面
- [ ] config 变更 → 全量重建
- [ ] manifest 记录完整的依赖关系
- [ ] 增量构建的输出与全量构建的输出一致
- [ ] 测试覆盖

**涉及文件**：
- `src/site.rs` — 增量构建逻辑重构
- `src/manifest.rs` — 增强依赖记录和查询
- `src/cache.rs` — 缓存策略调整
- `src/serve.rs` — 利用依赖图优化 rebuild mode 分类

---

### Phase 3 — 并行渲染

**目标：用 tokio 并行渲染页面，充分利用多核。**

#### 3.1 tokio 异步渲染

- 引入 `tokio` 依赖
- 将页面渲染阶段从串行 `for page in pages` 改为 `tokio::task::spawn_blocking` 并行
- 每个页面的模板渲染是 CPU 密集型，用 `spawn_blocking` 在 tokio 线程池中执行
- 结果收集后统一写入文件
- 保持渲染顺序确定性（输出文件路径与串行一致）

**验收标准**：
- [ ] 渲染阶段使用 tokio 并行
- [ ] 输出结果与串行渲染完全一致（bit-for-bit）
- [ ] 现有 78 个测试全部通过
- [ ] 大型站点（1000+ 页面）构建时间明显缩短
- [ ] `--profile` 输出包含并行渲染统计

**涉及文件**：
- `Cargo.toml` — 添加 tokio 依赖
- `src/site.rs` — 渲染循环改为并行
- `src/timing.rs` — 记录并行渲染统计

---

## 技术约束

### 新增依赖

| crate | 用途 | 阶段 |
|---|---|---|
| `insta` | snapshot 测试 | Phase 1 |
| `tokio` (rt-multi-thread) | 并行渲染 | Phase 3 |

### 不引入的依赖

- 不引入 rayon（用 tokio 代替）
- 不引入额外的日志框架（直接用 eprintln）
- 不引入 CLI 美化库（直接用 ANSI 转义码）

### 代码风格

- 遵循现有模块化结构，新功能优先放入已有模块
- 新模块需要充分理由（如 `asset.rs` 只在逻辑足够复杂时才拆分）
- 所有 public API 需要 doc comment
- 错误处理统一用 `anyhow`
- 测试与实现同文件（`#[cfg(test)] mod tests`）

---

## 测试策略

### 现有测试（必须保持通过）

- 58 个单元测试
- 20 个 fixture 集成测试
- 4 个 benchmark 测试（ignored）

### 新增测试

| 类型 | 阶段 | 数量 |
|---|---|---|
| snapshot 测试 | Phase 1 | ≥ 5 |
| profile 输出测试 | Phase 1 | ≥ 2 |
| 诊断格式测试 | Phase 1 | ≥ 3 |
| 模板依赖追踪测试 | Phase 2 | ≥ 3 |
| asset fingerprint 测试 | Phase 2 | ≥ 3 |
| 增量构建正确性测试 | Phase 2 | ≥ 5 |
| 并行渲染一致性测试 | Phase 3 | ≥ 3 |

---

## 边界

### 始终遵守

- 所有阶段共享 `feature/v2-content-pipeline` 分支
- 每个阶段完成后确保全量测试通过
- 不修改 v1 的公共 API 签名（可扩展，不改坏）
- clippy + `cargo fmt` 必须通过

### 先问再做

- 新增模块（确认是否值得拆分）
- 修改 `Cargo.toml` 的依赖版本范围
- 修改 `SiteConfig` 的配置结构

### 绝对不做

- 不做插件系统 / WASM
- 不做 JS runtime / MDX
- 不做服务端渲染
- 不做 CMS 后台
- 不做主题市场
- 不引入 async runtime 到非渲染路径
- 不在测试中 mock 文件系统
