[English](README.md)

# kiln

轻量级静态站点编译器，将 Markdown 编译为 HTML。

基于 Rust 构建，零运行时依赖，单文件部署。

## 功能特性

- Markdown 转 HTML，支持代码语法高亮、表格、任务列表 (comrak)
- TOML frontmatter 支持文章与页面元数据
- Tera 模板引擎，内置默认模板
- 模板依赖追踪，精确的缓存失效
- 自定义路由与模板的 Collection 机制
- 分类法（标签、分类或自定义分组），支持独立模板
- 分页归档与分栏页面
- RSS 订阅源与 Sitemap 自动生成
- 内置开发服务器，文件变更自动重新构建
- 资源指纹化，CSS url() 重写，`asset_url()` Tera 函数
- 并行页面渲染（tokio），加速大型站点构建
- 增量构建，基于内容、模板、资源的感知缓存
- 内容哈希实现缓存失效
- 草稿支持
- 构建分析 (`--profile` / `--profile-json`)，查看缓存命中率、页面渲染耗时、并行渲染统计
- 结构化诊断，彩色终端输出

## 快速开始

```bash
# 创建新站点
kiln init my-site
cd my-site

# 构建站点
kiln build --config site.config.toml --output dist

# 包含草稿文章
kiln build --config site.config.toml --drafts

# 构建并输出性能分析
kiln build --config site.config.toml --profile

# 构建并输出机器可读性能分析
kiln build --config site.config.toml --profile-json

# 仅验证配置和内容，不生成输出
kiln check --config site.config.toml

# 检查项目健康状态，并输出可执行修复建议
kiln doctor --config site.config.toml

# 清理构建产物，默认保留 .kiln 状态
kiln clean --output dist

# 仅清理 .kiln 状态
kiln clean --output dist --cache

# 启动开发服务器（自动重新构建）
kiln serve --config site.config.toml --port 4173
```

## 项目结构

```
site/
  site.config.toml   # 站点配置
  content/
    posts/           # 博客文章（Markdown + frontmatter）
    pages/           # 静态页面
  templates/         # Tera HTML 模板（可选，内置默认模板）
  public/            # 静态资源（以内容哈希文件名复制）
  styles.css         # 站点样式表
```

## 配置

最小 `site.config.toml` 示例：

```toml
[site]
title = "我的站点"
base_url = "https://example.com"

[author]
name = "作者"
email = "author@example.com"
```

包含 Collections 和分类法的完整示例：

```toml
[site]
title = "我的站点"
base_url = "https://example.com"
language = "zh-CN"

paginate_by = 10
paginate_path = "page"

[[collections]]
name = "posts"
directory = "posts"
route = "/posts/{slug}/"
template = "post.html"
date_ordered = true
feed = true

[[taxonomies]]
name = "tags"
slug = "tags"

[[menus.main]]
name = "首页"
url = "/"
weight = 1
```

## CLI 命令

| 命令 | 说明 |
|------|------|
| `kiln init <path>` | 创建可直接用内置模板构建的最小站点 |
| `kiln build` | 构建静态站点到输出目录 |
| `kiln check` | 验证配置和内容，不生成输出 |
| `kiln doctor` | 检查配置、内容、模板、路由与资源，并给出修复建议 |
| `kiln clean` | 清理构建产物或仅清理 `.kiln` 状态 |
| `kiln serve` | 启动开发服务器，文件变更自动重新构建 |

### Build 标志

| 标志 | 说明 |
|------|------|
| `--config <path>` | 站点配置文件路径（默认：`site/site.config.toml`） |
| `--output <dir>` | 输出目录（默认：`dist`） |
| `--drafts` | 构建时包含草稿文章 |
| `--profile` | 输出详细构建报告，包含缓存与渲染指标 |
| `--profile-json` | 输出机器可读的构建性能分析 JSON |

## 从源码构建

```bash
git clone https://github.com/rhczz/kiln.git
cd kiln
cargo build --release
```

## 许可证

MIT
