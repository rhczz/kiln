use crate::config::SiteConfig;
use crate::content::ContentItem;
use crate::sitemap::escape_xml;

pub fn generate(config: &SiteConfig, items: &[&ContentItem]) -> String {
    let latest = &items[..config.feed.item_count.min(items.len())];

    let mut rss_items = String::new();
    for item in latest {
        let link = format!("{}{}", config.site.base_url, item.url);
        let safe_desc = item.description.replace("]]>", "]]&gt;");
        let date_str = item.date.as_deref().unwrap_or("");
        rss_items.push_str(&format!(
            r#"    <item>
      <title>{}</title>
      <link>{}</link>
      <guid isPermaLink="true">{}</guid>
      <description><![CDATA[{}]]></description>
      <pubDate>{}</pubDate>
    </item>
"#,
            escape_xml(&item.title),
            escape_xml(&link),
            escape_xml(&link),
            safe_desc,
            rfc2822_date(date_str),
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>{title}</title>
    <link>{base_url}</link>
    <description>{description}</description>
    <language>{lang}</language>
    <atom:link href="{base_url}/rss.xml" rel="self" type="application/rss+xml"/>
{items}
  </channel>
</rss>"#,
        title = escape_xml(&config.site.title),
        base_url = escape_xml(&config.site.base_url),
        description = escape_xml(&config.site.description),
        lang = escape_xml(&config.site.language),
        items = rss_items,
    )
}

fn rfc2822_date(date_str: &str) -> String {
    if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        let dt = d.and_hms_opt(0, 0, 0).unwrap_or_default();
        let dt_utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc);
        dt_utc.format("%a, %d %b %Y %H:%M:%S +0000").to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{generate, rfc2822_date};
    use crate::config::{FeedConfig, PathsConfig, SiteConfig, SiteMeta};
    use crate::content::ContentItem;
    use crate::sitemap::escape_xml;
    use std::path::PathBuf;

    fn test_config() -> SiteConfig {
        SiteConfig {
            paths: PathsConfig::default(),
            site: SiteMeta {
                title: "Test".into(),
                subtitle: String::new(),
                description: "A test feed".into(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: None,
            feed: FeedConfig { item_count: 20 },
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
        }
    }

    fn test_item(title: &str, slug: &str, date: &str, description: &str) -> ContentItem {
        ContentItem {
            source_path: PathBuf::from(format!("content/posts/{}.md", slug)),
            content_hash: "hash".into(),
            title: title.into(),
            slug: slug.into(),
            description: description.into(),
            body_html: String::new(),
            collection: "posts".into(),
            url: format!("/posts/{}/", slug),
            date: Some(date.into()),
            iso_date: Some(date.into()),
            short_date: Some(date.into()),
            long_date: Some(date.into()),
            year: Some(date[..4].into()),
            featured: false,
            draft: false,
            tags: vec![],
            raw_date: chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok(),
        }
    }

    #[test]
    fn escapes_xml_special_chars() {
        assert_eq!(escape_xml("A&B"), "A&amp;B");
        assert_eq!(escape_xml("<b>"), "&lt;b&gt;");
        assert_eq!(escape_xml("\"x\""), "&quot;x&quot;");
        assert_eq!(escape_xml("it's"), "it&apos;s");
    }

    #[test]
    fn formats_rfc2822_date() {
        let result = rfc2822_date("2026-06-02");
        assert!(result.contains("02 Jun 2026"));
        assert!(result.ends_with("+0000"));
    }

    #[test]
    fn rfc2822_date_returns_empty_on_invalid() {
        assert_eq!(rfc2822_date("not-a-date"), "");
    }

    #[test]
    fn generates_valid_rss_with_posts() {
        let config = test_config();
        let items = vec![test_item(
            "A & B",
            "a-and-b",
            "2026-06-02",
            "Description with ]]> content",
        )];
        let refs: Vec<&ContentItem> = items.iter().collect();
        let rss = generate(&config, &refs);

        assert!(rss.contains("<?xml version=\"1.0\""));
        assert!(rss.contains("<title>A &amp; B</title>"));
        assert!(rss.contains("<link>https://example.com/posts/a-and-b/</link>"));
        assert!(
            rss.contains("<description><![CDATA[Description with ]]&gt; content]]></description>")
        );
        assert!(rss.contains("<pubDate>Tue, 02 Jun 2026"));
        assert!(rss.contains("<language>en</language>"));
    }

    #[test]
    fn generates_empty_feed_when_no_posts() {
        let config = test_config();
        let rss = generate(&config, &[]);
        assert!(rss.contains("<title>Test</title>"));
        assert!(!rss.contains("<item>"));
    }

    #[test]
    fn respects_feed_item_count() {
        let mut config = test_config();
        config.feed.item_count = 1;
        let items = vec![
            test_item("First", "first", "2026-06-02", ""),
            test_item("Second", "second", "2026-05-01", ""),
        ];
        let refs: Vec<&ContentItem> = items.iter().collect();
        let rss = generate(&config, &refs);
        assert!(rss.contains("<title>First</title>"));
        assert!(!rss.contains("<title>Second</title>"));
    }
}
