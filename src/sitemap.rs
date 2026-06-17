use crate::config::SiteConfig;
use crate::model::{Page, PageKind};

const MAX_URLS_PER_SITEMAP: usize = 50_000;
const MAX_SITEMAP_BYTES: usize = 45 * 1024 * 1024;

pub fn generate(config: &SiteConfig, pages: &[Page]) -> Vec<(String, String)> {
    let mut entries: Vec<String> = Vec::new();
    entries.push(url_entry(&config.site.base_url, "/"));
    for page in pages {
        if matches!(
            page.kind,
            PageKind::Home | PageKind::Alias | PageKind::NotFound
        ) {
            continue;
        }
        entries.push(url_entry(&config.site.base_url, &page.url));
    }

    if sitemap_fits_in_single_file(&entries) {
        return vec![("sitemap.xml".into(), format_sitemap(&entries))];
    }

    let mut files = Vec::new();
    let mut current_chunk: Vec<String> = Vec::new();
    let mut current_size = sitemap_overhead_bytes();

    for entry in entries {
        let entry_len = entry.len();
        if !current_chunk.is_empty()
            && (current_chunk.len() >= MAX_URLS_PER_SITEMAP
                || current_size + entry_len > MAX_SITEMAP_BYTES)
        {
            files.push(format_chunk(&current_chunk, files.len() + 1));
            current_chunk.clear();
            current_size = sitemap_overhead_bytes();
        }
        current_size += entry_len;
        current_chunk.push(entry);
    }

    if !current_chunk.is_empty() {
        files.push(format_chunk(&current_chunk, files.len() + 1));
    }

    let index = format_sitemap_index(&config.site.base_url, &files);
    let mut outputs = vec![("sitemap.xml".into(), index)];
    outputs.extend(files);
    outputs
}

fn sitemap_fits_in_single_file(entries: &[String]) -> bool {
    entries.len() <= MAX_URLS_PER_SITEMAP
        && sitemap_overhead_bytes() + entries.iter().map(|entry| entry.len()).sum::<usize>()
            <= MAX_SITEMAP_BYTES
}

fn format_chunk(entries: &[String], index: usize) -> (String, String) {
    let filename = format!("sitemap-{}.xml", index);
    (filename, format_sitemap(entries))
}

fn sitemap_overhead_bytes() -> usize {
    128
}

fn format_sitemap(entries: &[String]) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{}
</urlset>"#,
        entries.join("")
    )
}

fn format_sitemap_index(base_url: &str, files: &[(String, String)]) -> String {
    let entries: String = files
        .iter()
        .map(|(name, _)| {
            format!(
                "  <sitemap>\n    <loc>{}/{}</loc>\n  </sitemap>\n",
                escape_xml(base_url),
                escape_xml(name)
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{}
</sitemapindex>"#,
        entries
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
    use crate::model::{Page, PageKind};
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
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
            extra: toml::Value::Table(Default::default()),
        };

        let pages = vec![
            Page {
                kind: PageKind::Single,
                url: "/posts/hello/".into(),
                output_path: PathBuf::from("posts/hello/index.html"),
                template: "post.html".into(),
                title: "Post".into(),
                description: String::new(),
                source_path: Some(PathBuf::from("content/posts/hello.md")),
                content_item: Some(ContentItem {
                    source_path: PathBuf::from("content/posts/hello.md"),
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
                    taxonomy_terms: Default::default(),
                    extra: serde_json::json!({}),
                    aliases: vec![],
                    raw_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 2),
                    headings: vec![],
                    shortcodes: vec![],
                }),
                redirect_to: None,
            },
            Page {
                kind: PageKind::Single,
                url: "/about/".into(),
                output_path: PathBuf::from("about/index.html"),
                template: "page.html".into(),
                title: "About".into(),
                description: String::new(),
                source_path: Some(PathBuf::from("content/pages/about.md")),
                content_item: Some(ContentItem {
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
                    taxonomy_terms: Default::default(),
                    extra: serde_json::json!({}),
                    aliases: vec![],
                    raw_date: None,
                    headings: vec![],
                    shortcodes: vec![],
                }),
                redirect_to: None,
            },
            Page {
                kind: PageKind::NotFound,
                url: "/404.html".into(),
                output_path: PathBuf::from("404.html"),
                template: "404.html".into(),
                title: "404".into(),
                description: String::new(),
                source_path: None,
                content_item: None,
                redirect_to: None,
            },
        ];

        let files = generate(&config, &pages);
        let sitemap = &files[0].1;
        assert!(sitemap.contains("<loc>https://example.com/about/</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/posts/hello/</loc>"));
        assert!(!sitemap.contains("<loc>https://example.com/about</loc>"));
        assert!(!sitemap.contains("<loc>https://example.com/posts/hello</loc>"));
        assert!(!sitemap.contains("404.html"));
    }
}
