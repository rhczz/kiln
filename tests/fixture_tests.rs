use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kiln::{
    build, build_with_artifacts, AuthorConfig, BuildArtifacts, BuildCache, BuildMode,
    CollectionConfig, FeedConfig, PageKind, PathsConfig, SiteConfig, SiteMeta, TaxonomyConfig,
};

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
            "kiln-fixture-{}-{}-{}",
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
                title: "Fixture Site".into(),
                subtitle: "Test subtitle".into(),
                description: "A test fixture site".into(),
                language: "en".into(),
                base_url: "https://fixture.test".into(),
            },
            author: Some(AuthorConfig {
                name: "Fixture Author".into(),
                email: "author@fixture.test".into(),
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

    fn write_template(&self, relative_path: &str, content: &str) {
        let path = self.root.join("templates").join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn write_post(&self, filename: &str, frontmatter: &str, body: &str) {
        let path = self.root.join("content/posts").join(filename);
        fs::write(path, format!("---\n{}\n---\n{}", frontmatter, body)).unwrap();
    }

    fn write_page(&self, filename: &str, frontmatter: &str, body: &str) {
        let path = self.root.join("content/pages").join(filename);
        fs::write(path, format!("---\n{}\n---\n{}", frontmatter, body)).unwrap();
    }

    fn write_public(&self, relative_path: &str, content: &[u8]) {
        let path = self.root.join("public").join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {:?}: {}", path, e))
}

fn file_exists(path: &Path) -> bool {
    path.exists()
}

// --- Minimal site ---

#[test]
fn minimal_site_builds_homepage_and_feeds() {
    let f = FixtureBuilder::new("minimal");
    f.write_styles("body {}");
    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    assert!(file_exists(&output.join("index.html")));
    assert!(file_exists(&output.join("rss.xml")));
    assert!(file_exists(&output.join("sitemap.xml")));
    assert!(file_exists(&output.join("robots.txt")));

    let index = read(&output.join("index.html"));
    assert!(index.contains("Fixture Site"));
    assert!(index.contains(r#"href="/assets/styles."#));
}

// --- Single post ---

#[test]
fn single_post_generates_page_and_feed_entry() {
    let f = FixtureBuilder::new("single-post");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-01-hello.md",
        r#"title: "Hello World"
date: "2026-06-01"
description: "First post"
tags: ["hello", "test"]
"#,
        "This is my first post.",
    );
    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let post = read(&output.join("posts/hello/index.html"));
    assert!(post.contains("Hello World"));
    assert!(post.contains("This is my first post"));
    assert!(post.contains(r#"<time datetime="2026-06-01""#));
    assert!(post.contains("hello"));
    assert!(post.contains("test"));

    let rss = read(&output.join("rss.xml"));
    assert!(rss.contains("Hello World"));
    assert!(rss.contains("/posts/hello/"));

    let sitemap = read(&output.join("sitemap.xml"));
    assert!(sitemap.contains("/posts/hello/"));
}

// --- Multiple posts ---

#[test]
fn multiple_posts_generate_archive_on_home() {
    let f = FixtureBuilder::new("multi-post");
    f.write_styles("body {}");

    for i in 1..=5 {
        let day = format!("{:02}", i);
        f.write_post(
            &format!("2026-06-{}-post-{}.md", day, i),
            &format!(
                r#"title: "Post {}"
date: "2026-06-{}"
description: "Post number {}"
featured: {}
"#,
                i,
                day,
                i,
                i == 1
            ),
            &format!("Content of post {}.", i),
        );
    }

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    for i in 1..=5 {
        let post = read(&output.join(&format!("posts/post-{}/index.html", i)));
        assert!(post.contains(&format!("Post {}", i)));
    }
}

#[test]
fn paginated_home_pages_slice_archive() {
    let f = FixtureBuilder::new("paginate");
    f.write_styles("body {}");

    for i in 1..=3 {
        let day = format!("{:02}", i);
        f.write_post(
            &format!("2026-06-{}-post-{}.md", day, i),
            &format!(
                r#"title: "Post {}"
date: "2026-06-{}"
"#,
                i, day
            ),
            &format!("Content of post {}.", i),
        );
    }

    let mut config = f.config();
    config.paginate_by = 1;
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let index = read(&output.join("index.html"));
    assert!(index.contains("Post 3"));
    assert!(!index.contains("Post 2"));
    assert!(!index.contains("Post 1"));

    let page2 = read(&output.join("page/2/index.html"));
    assert!(page2.contains("Page 2"));
    assert!(page2.contains("Post 2"));
    assert!(!page2.contains("Post 3"));

    let page3 = read(&output.join("page/3/index.html"));
    assert!(page3.contains("Page 3"));
    assert!(page3.contains("Post 1"));
    assert!(!page3.contains("Post 2"));
}

// --- Pages without dates ---

#[test]
fn pages_without_dates_render_correctly() {
    let f = FixtureBuilder::new("pages");
    f.write_styles("body {}");
    f.write_page("about.md", r#"title: "About""#, "This is the about page.");
    f.write_page("contact.md", r#"title: "Contact""#, "Get in touch.");

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let about = read(&output.join("about/index.html"));
    assert!(about.contains("About"));
    assert!(about.contains("This is the about page"));
    assert!(!about.contains(r#"<time"#));

    let contact = read(&output.join("contact/index.html"));
    assert!(contact.contains("Contact"));
}

#[test]
fn custom_taxonomy_uses_configured_term_template() {
    let f = FixtureBuilder::new("taxonomy");
    f.write_styles("body {}");
    f.write_template("category_term.html", "CUSTOM TERM {{ term.name }}");
    f.write_post(
        "2026-06-01-rusty.md",
        r#"title: "Rusty"
date: "2026-06-01"
categories: ["Rust"]
"#,
        "Rust content.",
    );

    let mut config = f.config();
    config.taxonomies = vec![TaxonomyConfig {
        name: "categories".into(),
        slug: "categories".into(),
        template: "category_term.html".into(),
    }];
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    assert!(output.join("categories/index.html").is_file());
    let term = read(&output.join("categories/rust/index.html"));
    assert!(term.contains("CUSTOM TERM Rust"));
}

#[test]
fn custom_collection_route_uses_collection_section_template() {
    let f = FixtureBuilder::new("section-template");
    f.write_styles("body {}");
    f.write_template("posts_section.html", "CUSTOM SECTION {{ section.title }}");

    let docs = f.root().join("content/posts/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(
        docs.join("_index.md"),
        r#"---
title: "Docs"
---
"#,
    )
    .unwrap();
    std::fs::write(
        docs.join("2026-06-01-guide.md"),
        r#"---
title: "Guide"
date: "2026-06-01"
---
Guide content.
"#,
    )
    .unwrap();

    let mut config = f.config();
    config.collections = vec![CollectionConfig {
        name: "posts".into(),
        directory: "posts".into(),
        route: "/blog/{slug}/".into(),
        template: "post.html".into(),
        date_ordered: true,
        feed: true,
    }];
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let section = read(&output.join("blog/docs/index.html"));
    assert!(section.contains("CUSTOM SECTION Docs"));
}

#[test]
fn section_prefix_collision_does_not_include_sibling_content() {
    let f = FixtureBuilder::new("section-collision");
    f.write_styles("body {}");

    let docs = f.root().join("content/posts/docs");
    let docs2 = f.root().join("content/posts/docs2");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::create_dir_all(&docs2).unwrap();
    std::fs::write(
        docs.join("_index.md"),
        r#"---
title: "Docs"
---
"#,
    )
    .unwrap();
    std::fs::write(
        docs2.join("_index.md"),
        r#"---
title: "Docs2"
---
"#,
    )
    .unwrap();
    std::fs::write(
        docs.join("2026-06-01-docs.md"),
        r#"---
title: "Docs Item"
date: "2026-06-01"
---
Docs content.
"#,
    )
    .unwrap();
    std::fs::write(
        docs2.join("2026-06-01-docs2.md"),
        r#"---
title: "Docs2 Item"
date: "2026-06-01"
---
Docs2 content.
"#,
    )
    .unwrap();

    let mut config = f.config();
    config.paginate_by = 2;
    config.collections = vec![CollectionConfig {
        name: "posts".into(),
        directory: "posts".into(),
        route: "/blog/{slug}/".into(),
        template: "post.html".into(),
        date_ordered: true,
        feed: true,
    }];
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let docs_page = read(&output.join("blog/docs/index.html"));
    assert!(docs_page.contains("Docs Item"));
    assert!(!docs_page.contains("Docs2 Item"));
}

#[test]
fn custom_collection_route_drives_section_urls() {
    let f = FixtureBuilder::new("section-route");
    f.write_styles("body {}");

    let docs = f.root().join("content/posts/docs");
    let k8s = docs.join("k8s");
    std::fs::create_dir_all(&k8s).unwrap();
    std::fs::write(
        docs.join("_index.md"),
        r#"---
title: "Docs"
---
"#,
    )
    .unwrap();
    std::fs::write(
        k8s.join("_index.md"),
        r#"---
title: "K8s"
---
"#,
    )
    .unwrap();
    std::fs::write(
        k8s.join("2026-06-01-cluster.md"),
        r#"---
title: "Cluster"
date: "2026-06-01"
---
K8s content.
"#,
    )
    .unwrap();

    let mut config = f.config();
    config.collections = vec![CollectionConfig {
        name: "posts".into(),
        directory: "posts".into(),
        route: "/blog/{slug}/".into(),
        template: "post.html".into(),
        date_ordered: true,
        feed: true,
    }];
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    assert!(output.join("blog/docs/index.html").is_file());
    assert!(output.join("blog/docs/k8s/index.html").is_file());
    assert!(!output.join("posts/docs/index.html").exists());
}

// --- Public assets ---

#[test]
fn public_assets_are_copied_to_output() {
    let f = FixtureBuilder::new("public-assets");
    f.write_styles("body {}");
    f.write_public("images/logo.png", b"PNG_DATA");
    f.write_public("favicon.ico", b"ICO_DATA");
    f.write_public("sub/deep.txt", b"DEEP");

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    assert_eq!(
        fs::read(output.join("images/logo.png")).unwrap(),
        b"PNG_DATA"
    );
    assert_eq!(fs::read(output.join("favicon.ico")).unwrap(), b"ICO_DATA");
    assert_eq!(fs::read(output.join("sub/deep.txt")).unwrap(), b"DEEP");
}

// --- Draft handling ---

#[test]
fn draft_posts_excluded_from_production_build() {
    let f = FixtureBuilder::new("drafts");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-01-published.md",
        r#"title: "Published"
date: "2026-06-01"
"#,
        "Public content.",
    );
    f.write_post(
        "2026-06-02-draft.md",
        r#"title: "Draft"
date: "2026-06-02"
draft: true
"#,
        "Draft content.",
    );

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    assert!(file_exists(&output.join("posts/published/index.html")));
    assert!(!file_exists(&output.join("posts/draft/index.html")));

    let rss = read(&output.join("rss.xml"));
    assert!(rss.contains("Published"));
    assert!(!rss.contains("Draft"));
}

#[test]
fn draft_posts_included_when_requested() {
    let f = FixtureBuilder::new("drafts-include");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-02-draft.md",
        r#"title: "Draft"
date: "2026-06-02"
draft: true
"#,
        "Draft content.",
    );

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, true).unwrap();

    assert!(file_exists(&output.join("posts/draft/index.html")));
}

// --- Feed item count ---

#[test]
fn feed_respects_item_count() {
    let f = FixtureBuilder::new("feed-limit");
    f.write_styles("body {}");

    for i in 1..=10 {
        let day = format!("{:02}", i);
        f.write_post(
            &format!("2026-06-{}-post-{}.md", day, i),
            &format!(
                r#"title: "Post {}"
date: "2026-06-{}"
description: "Post {}"
"#,
                i, day, i
            ),
            &format!("Content {}", i),
        );
    }

    let mut config = f.config();
    config.feed.item_count = 3;
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let rss = read(&output.join("rss.xml"));
    assert!(rss.contains("Post 10"));
    assert!(rss.contains("Post 9"));
    assert!(rss.contains("Post 8"));
    assert!(!rss.contains("Post 7"));
}

// --- Sitemap ---

#[test]
fn sitemap_includes_all_published_urls() {
    let f = FixtureBuilder::new("sitemap");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-01-post.md",
        r#"title: "Post"
date: "2026-06-01"
"#,
        "Content.",
    );
    f.write_page("about.md", r#"title: "About""#, "About page.");

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let sitemap = read(&output.join("sitemap.xml"));
    assert!(sitemap.contains("https://fixture.test/"));
    assert!(sitemap.contains("https://fixture.test/posts/post/"));
    assert!(sitemap.contains("https://fixture.test/about/"));
}

// --- robots.txt ---

#[test]
fn robots_txt_points_to_sitemap() {
    let f = FixtureBuilder::new("robots");
    f.write_styles("body {}");

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let robots = read(&output.join("robots.txt"));
    assert!(robots.contains("User-agent: *"));
    assert!(robots.contains("Allow: /"));
    assert!(robots.contains("Sitemap: https://fixture.test/sitemap.xml"));
}

// --- Custom slug ---

#[test]
fn custom_slug_overrides_filename() {
    let f = FixtureBuilder::new("custom-slug");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-01-original.md",
        r#"title: "Custom Slug Post"
date: "2026-06-01"
slug: "custom-url"
"#,
        "Content.",
    );

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    assert!(file_exists(&output.join("posts/custom-url/index.html")));
    assert!(!file_exists(&output.join("posts/original/index.html")));
}

// --- Featured posts ---

#[test]
fn featured_posts_appear_in_home_context() {
    let f = FixtureBuilder::new("featured");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-01-featured.md",
        r#"title: "Featured"
date: "2026-06-01"
featured: true
"#,
        "Featured content.",
    );
    f.write_post(
        "2026-06-02-normal.md",
        r#"title: "Normal"
date: "2026-06-02"
"#,
        "Normal content.",
    );

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    let index = read(&output.join("index.html"));
    assert!(index.contains("Featured"));
}

// --- Date prefix stripped ---

#[test]
fn date_prefix_stripped_from_slug() {
    let f = FixtureBuilder::new("slug-strip");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-01-my-post.md",
        r#"title: "My Post"
date: "2026-06-01"
"#,
        "Content.",
    );

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false).unwrap();

    assert!(file_exists(&output.join("posts/my-post/index.html")));
}

// --- Incremental build ---

#[test]
fn incremental_build_updates_changed_post() {
    let f = FixtureBuilder::new("incremental");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-01-change.md",
        r#"title: "Original Title"
date: "2026-06-01"
"#,
        "Original content.",
    );

    let config = f.config();
    let output = f.root().join("dist");
    let mut cache = BuildCache::new();

    let artifacts = BuildArtifacts::load(&config).unwrap();
    build_with_artifacts(
        &config,
        &output,
        false,
        BuildMode::Full,
        Some(&mut cache),
        &artifacts,
        true,
    )
    .unwrap();

    let post = read(&output.join("posts/change/index.html"));
    assert!(post.contains("Original Title"));

    // Simulate content change
    f.write_post(
        "2026-06-01-change.md",
        r#"title: "Updated Title"
date: "2026-06-01"
"#,
        "Updated content.",
    );

    build_with_artifacts(
        &config,
        &output,
        false,
        BuildMode::Content,
        Some(&mut cache),
        &artifacts,
        true,
    )
    .unwrap();

    let post = read(&output.join("posts/change/index.html"));
    assert!(post.contains("Updated Title"));
    assert!(!post.contains("Original Title"));
}

// --- SiteModel types are exported ---

#[test]
fn page_kind_variants_exist() {
    let _home = PageKind::Home;
    let _single = PageKind::Single;
    let _section = PageKind::Section;
    let _taxonomy_index = PageKind::TaxonomyIndex;
    let _term = PageKind::Term;
    let _paginate = PageKind::Paginate;
    let _not_found = PageKind::NotFound;
}
