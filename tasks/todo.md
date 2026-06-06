# Phase 1 TODO

## Task 1.3：终端诊断增强

- [x] 1.3.1 扩展 `src/diagnostic.rs`：新增 column/source_context/template_stack 字段，DiagnosticCollector，emit_diagnostic，print_build_summary，9 单元测试
- [x] 1.3.2 接入构建管线 `src/site.rs` + `src/engine.rs`：替换 eprintln warning，构建结束输出诊断摘要，增强模板错误信息

## Task 1.2：Build Profile

- [x] 1.2.1 扩展 `src/timing.rs`：ProfileData + PageTiming，with_profile/record_cache_hit/record_render/start_page/end_page/print_profile_report
- [x] 1.2.2 `src/cache.rs` 添加 cache_stats：Cell<usize> 计数器，cache_stats() 方法
- [x] 1.2.3 `src/cli.rs` 添加 `--profile` flag：build + build_with_artifacts 签名变更，更新所有调用点（cli.rs, site.rs, serve.rs, tests/）
- [x] 1.2.4 `src/site.rs` 采集 profile 指标：timer 模式选择，per-page 计时，cache 统计提取

## Task 1.1：Golden Output Tests

- [x] 1.1.1 `Cargo.toml` 添加 insta dev-dependency
- [x] 1.1.2 创建 `tests/snapshot_tests.rs`：FixtureBuilder + 6 个 snapshot 测试（homepage, post, rss, sitemap, robots, manifest）

## 检查点

- [x] cargo test 全部通过（97 测试：71 unit + 20 fixture + 6 snapshot）
- [x] cargo clippy -- -D warnings 零警告
- [x] cargo fmt --check 格式正确
- [x] cargo insta test && cargo insta review 通过
- [ ] cargo run -- build 输出无回归（需实际站点验证）
- [ ] cargo run -- build --profile 输出增强报告（需实际站点验证）
- [ ] NO_COLOR=1 cargo run -- build 纯文本输出（需实际站点验证）
