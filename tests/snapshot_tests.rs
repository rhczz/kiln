use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kiln::{build, AuthorConfig, FeedConfig, PathsConfig, SiteConfig, SiteMeta};

struct FixtureBuilder {
    root: PathBuf,
}

impl Drop for FixtureBuilder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl FixtureBuilder {
    fn new(prefix: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "kiln-snap-{}-{}-{}",
            prefix,
            std::process::id(),
            now
        ));
        fs::create_dir_all(root.join("content/posts")).unwrap();
        fs::create_dir_all(root.join("content/pages")).unwrap();
        Self { root }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn config(&self) -> SiteConfig {
        SiteConfig {
            paths: PathsConfig {
                content: self.root.join("content").to_string_lossy().to_string(),
                templates: self.root.join("templates").to_string_lossy().to_string(),
                public: self.root.join("public").to_string_lossy().to_string(),
                styles: self.root.join("styles.css").to_string_lossy().to_string(),
            },
            site: SiteMeta {
                title: "Snapshot Site".into(),
                subtitle: "Snapshot subtitle".into(),
                description: "A snapshot test site".into(),
                language: "en".into(),
                base_url: "https://snapshot.test".into(),
            },
            author: Some(AuthorConfig {
                name: "Snap Author".into(),
                email: "snap@test.test".into(),
            }),
            feed: FeedConfig { item_count: 20 },
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
        }
    }

    fn write_styles(&self, css: &str) {
        fs::write(self.root.join("styles.css"), css).unwrap();
    }

    fn write_post(&self, filename: &str, frontmatter: &str, body: &str) {
        let path = self.root.join("content/posts").join(filename);
        fs::write(path, format!("---\n{}\n---\n{}", frontmatter, body)).unwrap();
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {:?}: {}", path, e))
}

fn single_post_fixture() -> (FixtureBuilder, PathBuf) {
    let f = FixtureBuilder::new("single");
    f.write_styles("body { margin: 0; }");
    f.write_post(
        "2026-06-01-hello.md",
        r#"title: "Hello World"
date: "2026-06-01"
description: "First snapshot test"
featured: true
tags: ["test", "snapshot"]
"#,
        "## Intro\n\nParagraph with **bold**.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n",
    );
    let output = f.root().join("dist");
    build(&f.config(), &output, false, false).unwrap();
    (f, output)
}

#[test]
fn snapshot_homepage_html() {
    let (_f, output) = single_post_fixture();
    let html = read(&output.join("index.html"));
    insta::assert_snapshot!("homepage_html", html);
}

#[test]
fn snapshot_post_page_html() {
    let (_f, output) = single_post_fixture();
    let html = read(&output.join("posts/hello/index.html"));
    insta::assert_snapshot!("post_page_html", html);
}

#[test]
fn snapshot_rss_feed() {
    let (_f, output) = single_post_fixture();
    let xml = read(&output.join("rss.xml"));
    insta::assert_snapshot!("rss_feed", xml);
}

#[test]
fn snapshot_sitemap_xml() {
    let (_f, output) = single_post_fixture();
    let xml = read(&output.join("sitemap.xml"));
    insta::assert_snapshot!("sitemap_xml", xml);
}

#[test]
fn snapshot_robots_txt() {
    let (_f, output) = single_post_fixture();
    let txt = read(&output.join("robots.txt"));
    insta::assert_snapshot!("robots_txt", txt);
}

#[test]
fn snapshot_build_manifest() {
    let (f, output) = single_post_fixture();
    let json = read(&output.join(".kiln/manifest.json"));
    let root_str = f.root().to_string_lossy().to_string();
    let normalized = json.replace(&root_str, "<ROOT>");

    let mut manifest: serde_json::Value = serde_json::from_str(&normalized).unwrap();
    if let Some(entries) = manifest.get_mut("entries").and_then(|e| e.as_array_mut()) {
        entries.sort_by_key(|e| e["source"].as_str().unwrap_or("").to_string());
    }
    let sorted = serde_json::to_string_pretty(&manifest).unwrap();

    insta::assert_snapshot!("build_manifest", sorted);
}
