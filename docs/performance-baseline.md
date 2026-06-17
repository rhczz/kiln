# Performance Baseline

This document records local benchmark baselines for large generated sites. These numbers are trend
references, not CI thresholds.

## Environment

- Date: 2026-06-17
- Command profile: `cargo test --test bench_gen ... -- --ignored --nocapture`
- Build profile: Rust test profile, unoptimized with debuginfo

## Large Site Fixture

The generated benchmark fixture lives in `tests/bench_gen.rs` and creates Markdown posts with
frontmatter, tags, headings, tables, task lists, and code blocks.

| Fixture | Total build time | Throughput | Main phase timings |
|---------|------------------|------------|--------------------|
| 1000 posts | 1101ms | 908 posts/sec | `load_content_markdown` 208ms, `render_pages` 492ms, `generate_feeds_sitemap` 386ms |
| 5000 posts | 10370ms | 482 posts/sec | `load_content_markdown` 1063ms, `render_pages` 2013ms, `generate_feeds_sitemap` 7270ms |

## Reproduce

```bash
cargo test --test bench_gen bench_1000_posts -- --ignored --nocapture
cargo test --test bench_gen bench_5000_posts -- --ignored --nocapture
```

For automation-friendly profiling of a real site, use:

```bash
kiln build --config site.config.toml --output dist --profile-json
```
