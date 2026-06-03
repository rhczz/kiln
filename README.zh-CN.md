[English](README.md)

# kiln

轻量级静态站点编译器，将 Markdown 编译为 HTML。

基于 Rust 构建，零运行时依赖，单文件部署。

## 功能特性

- Markdown 转 HTML，支持代码语法高亮 (comrak)
- TOML frontmatter 支持文章与页面元数据
- Tera 模板引擎
- 自定义路由与模板的 Collection 机制
- RSS 订阅源与 Sitemap 自动生成
- 内置开发服务器，文件变更自动重新构建
- 内容哈希实现缓存失效
- 草稿支持

## 快速开始

```bash
# 构建站点
kiln build --config site/site.config.toml --output dist

# 启动开发服务器（自动重新构建）
kiln serve --config site/site.config.toml --port 4173
```

## 项目结构

```
site/
  site.config.toml   # 站点配置
  content/
    posts/           # 博客文章（Markdown + frontmatter）
    pages/           # 静态页面
  templates/         # Tera HTML 模板
  public/            # 静态资源（原样复制）
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

## 从源码构建

```bash
git clone https://github.com/rhczz/kiln.git
cd kiln
cargo build --release
```

## 许可证

MIT
