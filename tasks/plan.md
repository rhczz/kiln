# Phase 1 实施计划：工程基础设施

## Context

kiln v2 的目标是在 v1 功能完备基础上提升构建性能、增量构建精度、可观测性和测试覆盖率。Phase 1 是整个 v2 的基础层：建立测试安全网（insta snapshot）、构建分析能力（`--profile`）、和终端诊断系统（彩色输出 + 源码上下文）。后续 Phase 2（依赖追踪/增量构建）和 Phase 3（并行渲染）都依赖 Phase 1 的测试和可观测性基础设施。

## 执行顺序

推荐 **1.3 → 1.2 → 1.1**，原因：
- 1.3 和 1.2 都会修改 `site.rs` 和 `cli.rs`，先完成避免交叉冲突
- 1.1 的 snapshot 应该基于 1.2/1.3 完成后的最终输出状态生成，避免后续需要重新生成 snapshot

---

## Task 1.3：终端诊断增强

### 1.3.1 扩展 Diagnostic 结构体

**文件**: `src/diagnostic.rs`

当前 `Diagnostic` 结构体存在但生产代码未使用（有 `#[allow(dead_code)]`）。需要扩展为完整的诊断发射器。

变更内容：
- 新增 `column: Option<usize>` 字段
- 新增 `source_context: Option<SourceContext>` 字段（出错行 ±2 行源码片段）
- 新增 `template_stack: Vec<TemplateFrame>` 字段（模板调用链）
- 新增 `SourceContext` 结构体：`{ snippet: String, highlight_line: usize }`
- 新增 `TemplateFrame` 结构体：`{ template: String, line: Option<usize> }`
- 新增 `with_column(col)`、`with_source_context()`、`with_template_stack(stack)` builder 方法
- 新增 `read_source_context(source, line, context_lines) -> Option<SourceContext>` 辅助函数

新增 `DiagnosticCollector`：
```rust
pub struct DiagnosticCollector {
    diagnostics: Vec<Diagnostic>,
}
```
- `push(diag)` — 添加诊断
- `errors() / warnings()` — 按级别过滤
- `has_errors()` — 是否有 error
- `summary() -> (usize, usize)` — 返回 (errors, warnings) 计数

新增 `emit_diagnostic(diag)` 函数：
- 检测 `NO_COLOR` 环境变量和 `TERM != "dumb"` 决定是否着色
- 输出格式：`file:line:col: error: message`（兼容 IDE 跳转）
- 着色方案：error 红色 `\x1b[31m`，warning 黄色 `\x1b[33m`，hint 蓝色 `\x1b[34m`
- 位置信息用 `--> file:line:col` 格式
- 源码上下文用 ` > ` 标记高亮行，`^` 指向错误位置

新增 `print_build_summary(collector)` 函数：
- 输出 `Build finished with X error(s), Y warning(s).`

保留现有 `Display` impl 和 `format_diagnostics` 函数不变（向后兼容）。

新增单元测试（≥3）：
- `DiagnosticCollector` 计数正确
- `emit_plain` 格式包含 file:line:col
- `SourceContext` 渲染正确

### 1.3.2 接入构建管线

**文件**: `src/site.rs`、`src/engine.rs`

`src/site.rs`：
- `build_with_artifacts` 内部创建 `DiagnosticCollector`
- 将 `site.rs:194` 的 `eprintln!("Warning: no date-ordered items found...")` 改为 `diagnostics.push(Diagnostic::warning(...))`
- 构建结束前调用 `emit_diagnostic` 遍历输出所有诊断
- 输出 `print_build_summary`

`src/engine.rs`：
- 增强 `render()` 方法的错误处理，捕获模板名称作为 `TemplateFrame`，在错误信息中包含模板栈
- 注意：Tera 的 error 类型不暴露完整模板链 API，只提取顶层模板名即可

**关键约束**：`DiagnosticCollector` 不替代 `anyhow` 错误处理。硬错误（配置解析失败、文件缺失、模板渲染失败）继续用 `anyhow::bail!` / `?` 传播。Collector 只处理软诊断（warning、note）。

**不修改** `build_with_artifacts` 签名——collector 是内部局部变量。

### 验证
- `cargo test` 全部通过（78 个现有测试）
- `cargo clippy -- -D warnings` 通过
- `NO_COLOR=1 cargo run -- build` 输出纯文本诊断

---

## Task 1.2：Build Profile (`--profile`)

### 1.2.1 扩展 BuildTimer

**文件**: `src/timing.rs`

新增 `ProfileData` 结构体：
```rust
pub struct ProfileData {
    cache_hits: usize,
    cache_misses: usize,
    render_count: usize,
    page_timings: Vec<PageTiming>,
}

pub struct PageTiming {
    pub url: String,
    pub template: String,
    pub elapsed_ms: u128,
}
```

`BuildTimer` 新增字段 `profile: Option<ProfileData>`。

新增方法：
- `with_profile() -> Self` — 创建带 profile 的 timer
- `record_cache_hit()` / `record_cache_miss()` — 递增计数器（仅 profile 模式）
- `record_render()` — 递增渲染计数
- `start_page(url, template)` / `end_page()` — 记录单页耗时
- `set_cache_stats(hits, misses)` — 从外部设置缓存统计
- `print_profile_report()` — 输出详细 profile 报告

现有 `new()`、`phase()`、`finish()`、`total_ms()`、`print_report()` 不变。`profile` 字段默认 `None`。

### 1.2.2 BuildCache 添加命中统计

**文件**: `src/cache.rs`

用 `std::cell::Cell<usize>` 添加 `cache_hits` 和 `cache_misses` 字段（保持 `cached_render` 的 `&self` 签名不变）。

在以下位置计数：
- `parse_content_item`：缓存命中（hash 匹配返回 cached）→ hit，否则 → miss
- `cached_render`：返回 `Some` → hit，返回 `None` → miss

新增方法：
- `cache_stats(&self) -> (usize, usize)` — 返回 (hits, misses)

### 1.2.3 CLI 添加 `--profile` flag

**文件**: `src/cli.rs`

`Build` variant 新增：
```rust
#[arg(long)]
profile: bool,
```

`build` 函数签名变更：
```rust
pub fn build(config: &SiteConfig, output_dir: &Path, include_drafts: bool, profile: bool) -> anyhow::Result<()>
```

`build_with_artifacts` 签名变更（新增 `profile: bool` 参数）：
```rust
pub fn build_with_artifacts(
    config: &SiteConfig, output_dir: &Path, include_drafts: bool,
    mode: BuildMode, cache: Option<&mut BuildCache>,
    artifacts: &BuildArtifacts, emit_report: bool, profile: bool,
) -> anyhow::Result<()>
```

所有调用点更新：
- `cli.rs:55` — `crate::site::build(&site_config, &output, drafts, profile)`
- `cli.rs:63` — Check 命令传 `false`
- `site.rs:14` — `build` 传 `profile` 给 `build_with_artifacts`
- `site.rs:30` — `build_public_incremental` 传 `false`
- `serve.rs` — 所有调用传 `false`
- `tests/fixture_tests.rs` — 所有 `build` 调用添加 `false`（约 20 处）
- `tests/bench_gen.rs` — 添加 `false`

### 1.2.4 采集 profile 指标

**文件**: `src/site.rs`

- 根据 `profile` 参数选择 `BuildTimer::with_profile()` 或 `BuildTimer::new()`
- `render_model_pages` 中用 `timer.start_page()` / `timer.end_page()` 包裹每个页面渲染
- 构建结束后从 `cache.cache_stats()` 提取命中统计
- `profile` 模式下调用 `timer.print_profile_report()` 输出增强报告

### 验证
- `cargo test` 全部通过
- `cargo clippy -- -D warnings` 通过
- `cargo run -- build` — 输出与现在一致（无 profile）
- `cargo run -- build --profile` — 输出增强的 profile 报告

---

## Task 1.1：Golden Output Tests (insta)

### 1.1.1 添加 insta 依赖

**文件**: `Cargo.toml`

新增：
```toml
[dev-dependencies]
insta = "1"
```

### 1.1.2 创建 snapshot 测试

**文件**: `tests/snapshot_tests.rs`（新建）

复用 `fixture_tests.rs` 中的 `FixtureBuilder` 模式（复制约 60 行，不引入共享模块以保持自包含）。

测试列表（≥6 个 snapshot）：
1. `snapshot_homepage_html` — 首页 HTML
2. `snapshot_post_page_html` — 文章页 HTML
3. `snapshot_rss_feed` — RSS feed XML
4. `snapshot_sitemap_xml` — Sitemap XML
5. `snapshot_robots_txt` — robots.txt
6. `snapshot_build_manifest` — .kiln/manifest.json

共享 fixture 函数 `single_post_fixture()` 创建包含一篇带标签、日期、描述的文章的最小站点。

snapshot 文件存储在 `tests/snapshots/snapshot_tests/` 目录。

### 验证
- `cargo insta test` — 6 个 snapshot 全部生成
- `cargo insta review` — 审核并接受所有 snapshot
- `cargo test` — 78 现有 + 6 新测试全部通过
- `cargo clippy -- -D warnings` 通过

---

## 整体验证

Phase 1 完成后：
1. `cargo test` — 目标 93+ 测试全部通过
2. `cargo clippy -- -D warnings` — 零警告
3. `cargo fmt --check` — 格式正确
4. `cargo insta test && cargo insta review` — 所有 snapshot 匹配
5. `cargo run -- build` — 输出与 v1 一致（无回归）
6. `cargo run -- build --profile` — 输出增强的 profile 报告
7. `NO_COLOR=1 cargo run -- build` — 纯文本诊断（无 ANSI 码）

---

## 关键文件清单

| 文件 | Task | 变更类型 |
|---|---|---|
| `src/diagnostic.rs` | 1.3 | 大幅扩展 |
| `src/timing.rs` | 1.2 | 扩展 |
| `src/cache.rs` | 1.2 | 小改（加计数器） |
| `src/cli.rs` | 1.2 | 加 `--profile` flag |
| `src/site.rs` | 1.2, 1.3 | 传 profile flag + DiagnosticCollector |
| `src/engine.rs` | 1.3 | 增强模板错误处理 |
| `src/lib.rs` | 1.3 | 导出新类型 |
| `Cargo.toml` | 1.1 | 添加 insta dev-dependency |
| `tests/snapshot_tests.rs` | 1.1 | 新建 |
| `tests/fixture_tests.rs` | 1.2 | 更新 build 调用签名 |
| `tests/bench_gen.rs` | 1.2 | 更新 build 调用签名 |

## 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| `build` 签名变更影响 20+ 测试调用点 | 机械性修改，编译器会报错 | 逐个修复，cargo check 验证 |
| ANSI 码在 Windows 终端不兼容 | Windows 用户看到乱码 | 检测 `NO_COLOR` 和 `TERM`，默认安全 |
| snapshot 包含 CSS hash，样式内容变化会导致 snapshot 失效 | 预期行为，`cargo insta review` 处理 | fixture 使用固定的 CSS 内容 |
| Tera 不暴露完整模板栈 | 模板错误栈只能显示顶层 | 只提取可用信息，不过度包装 |
