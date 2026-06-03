use crate::config::SiteConfig;
use crate::content::ContentItem;

pub fn generate(config: &SiteConfig, items: &[ContentItem]) -> String {
    let mut urls = String::new();

    // Homepage
    urls.push_str(&url_entry(&config.site.base_url, ""));

    // All items
    for item in items {
        urls.push_str(&url_entry(&config.site.base_url, &item.url));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{}
</urlset>"#,
        urls
    )
}

fn url_entry(base_url: &str, path: &str) -> String {
    format!(
        "  <url>\n    <loc>{}{}</loc>\n  </url>\n",
        escape_xml(base_url),
        escape_xml(path)
    )
}

pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::generate;
    use crate::config::{FeedConfig, PathsConfig, SiteConfig, SiteMeta};
    use crate::content::ContentItem;
    use std::path::PathBuf;

    #[test]
    fn emits_canonical_trailing_slash_urls() {
        let config = SiteConfig {
            paths: PathsConfig::default(),
            site: SiteMeta {
                title: "Test".into(),
                subtitle: String::new(),
                description: String::new(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: None,
            feed: FeedConfig::default(),
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
        };

        let items = vec![
            ContentItem {
                source_path: PathBuf::from("content/posts/2026-06-02-hello.md"),
                content_hash: "hash".into(),
                title: "Post".into(),
                slug: "hello".into(),
                description: String::new(),
                body_html: String::new(),
                collection: "posts".into(),
                url: "/posts/hello/".into(),
                date: Some("2026-06-02".into()),
                iso_date: Some("2026-06-02".into()),
                short_date: Some("2026.06.02".into()),
                long_date: Some("June 2, 2026".into()),
                year: Some("2026".into()),
                featured: false,
                draft: false,
                tags: vec![],
                raw_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 2),
            },
            ContentItem {
                source_path: PathBuf::from("content/pages/about.md"),
                content_hash: "hash".into(),
                title: "About".into(),
                slug: "about".into(),
                description: String::new(),
                body_html: String::new(),
                collection: "pages".into(),
                url: "/about/".into(),
                date: None,
                iso_date: None,
                short_date: None,
                long_date: None,
                year: None,
                featured: false,
                draft: false,
                tags: vec![],
                raw_date: None,
            },
        ];

        let sitemap = generate(&config, &items);
        assert!(sitemap.contains("<loc>https://example.com/about/</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/posts/hello/</loc>"));
        assert!(!sitemap.contains("<loc>https://example.com/about</loc>"));
        assert!(!sitemap.contains("<loc>https://example.com/posts/hello</loc>"));
    }
}
