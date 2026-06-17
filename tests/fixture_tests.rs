use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kiln::{
    build, build_with_artifacts, AuthorConfig, BuildArtifacts, BuildCache, BuildMode, BuildOptions,
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

fn build_opts(mode: BuildMode) -> BuildOptions {
    BuildOptions {
        include_drafts: false,
        mode,
        emit_report: false,
        profile: false,
        profile_json: false,
    }
}

fn collect_output_files(root: &Path) -> Vec<PathBuf> {
    fn visit(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {:?}: {}", dir, e)) {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.push(path.strip_prefix(root).unwrap().to_path_buf());
            }
        }
    }

    let mut files = Vec::new();
    visit(root, root, &mut files);
    files.sort();
    files
}

fn assert_output_dirs_match(case: &str, incremental: &Path, clean: &Path) {
    let incremental_files = collect_output_files(incremental);
    let clean_files = collect_output_files(clean);
    assert_eq!(
        incremental_files, clean_files,
        "{case}: output file lists differ"
    );

    for relative in incremental_files {
        let incremental_bytes = fs::read(incremental.join(&relative)).unwrap();
        let clean_bytes = fs::read(clean.join(&relative)).unwrap();
        assert_eq!(
            incremental_bytes,
            clean_bytes,
            "{case}: output file differs: {}",
            relative.display()
        );
    }
}

// --- Minimal site ---

#[test]
fn minimal_site_builds_homepage_and_feeds() {
    let f = FixtureBuilder::new("minimal");
    f.write_styles("body {}");
    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false, false).unwrap();

    assert!(file_exists(&output.join("index.html")));
    assert!(file_exists(&output.join("rss.xml")));
    assert!(file_exists(&output.join("sitemap.xml")));
    assert!(file_exists(&output.join("robots.txt")));

    let index = read(&output.join("index.html"));
    assert!(index.contains("Fixture Site"));
    assert!(index.contains(r#"href="/assets/styles."#));
}

#[test]
fn build_fails_when_collections_generate_same_url() {
    let f = FixtureBuilder::new("collection-url-conflict");
    fs::create_dir_all(f.root().join("content/notes")).unwrap();
    f.write_styles("body {}");
    f.write_post(
        "about.md",
        r#"title: "About Post"
date: "2026-06-01"
slug: "about""#,
        "Post body",
    );
    fs::write(
        f.root().join("content/notes/about.md"),
        "---\ntitle: \"About Note\"\nslug: \"about\"\n---\nNote body",
    )
    .unwrap();

    let mut config = f.config();
    config.collections = vec![
        CollectionConfig {
            name: "posts".into(),
            directory: "posts".into(),
            route: "/{slug}/".into(),
            template: "post.html".into(),
            date_ordered: true,
            feed: false,
        },
        CollectionConfig {
            name: "notes".into(),
            directory: "notes".into(),
            route: "/{slug}/".into(),
            template: "page.html".into(),
            date_ordered: false,
            feed: false,
        },
    ];

    let err = build(&config, &f.root().join("dist"), false, false).unwrap_err();
    assert!(err
        .to_string()
        .contains("output path conflict at about/index.html"));
    assert!(err.to_string().contains("content/posts/about.md"));
    assert!(err.to_string().contains("content/notes/about.md"));
}

#[test]
fn build_fails_when_content_url_collides_with_taxonomy_index() {
    let f = FixtureBuilder::new("taxonomy-url-conflict");
    f.write_styles("body {}");
    f.write_page(
        "tags.md",
        r#"title: "Tags"
slug: "tags""#,
        "This page collides with the generated taxonomy index.",
    );

    let config = f.config();
    let err = build(&config, &f.root().join("dist"), false, false).unwrap_err();
    assert!(err
        .to_string()
        .contains("output path conflict at tags/index.html"));
    assert!(err.to_string().contains("content/pages/tags.md"));
    assert!(err
        .to_string()
        .contains("generated TaxonomyIndex page /tags/"));
}

#[test]
fn build_fails_when_manual_config_taxonomy_slug_conflicts_with_collection_route() {
    let f = FixtureBuilder::new("manual-taxonomy-route-conflict");
    f.write_styles("body {}");

    let mut config = f.config();
    config.collections = vec![CollectionConfig {
        name: "articles".into(),
        directory: "posts".into(),
        route: "/tags/{slug}/".into(),
        template: "post.html".into(),
        date_ordered: false,
        feed: false,
    }];

    let err = build(&config, &f.root().join("dist"), false, false).unwrap_err();
    assert!(err.to_string().contains("taxonomy slug \"tags\""));
    assert!(err
        .to_string()
        .contains("conflicts with collection \"articles\" route \"/tags/{slug}/\""));
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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

    let about = read(&output.join("about/index.html"));
    assert!(about.contains("About"));
    assert!(about.contains("This is the about page"));
    assert!(!about.contains(r#"<time"#));

    let contact = read(&output.join("contact/index.html"));
    assert!(contact.contains("Contact"));
}

#[test]
fn page_extra_aliases_and_headings_are_available_to_templates() {
    let f = FixtureBuilder::new("content-model");
    f.write_styles("body {}");
    f.write_template(
        "page.html",
        r#"TITLE {{ page.title }}
COVER {{ page.extra.cover }}
CTA {{ page.extra.cta.label }}={{ page.extra.cta.href }}
ALIASES {% for alias in page.aliases %}{{ alias }} {% endfor %}
HEADINGS {% for heading in page.headings %}{{ heading.level }}:{{ heading.id }}:{{ heading.text }} {% endfor %}
TOC {% for heading in page.toc %}{{ heading.id }} {% endfor %}
BODY {{ page.body_html | safe }}"#,
    );
    f.write_page(
        "guide.md",
        r#"title: "Guide"
cover: "/images/guide.jpg"
aliases:
  - /old-guide/
  - legacy/guide
  - /guide.html
cta:
  label: "Read"
  href: "/start/""#,
        "# Intro\n\n## Install\n\nBody.",
    );

    let config = f.config();
    let output = f.root().join("dist");

    build(&config, &output, false, false).unwrap();

    let page = read(&output.join("guide/index.html"));
    assert!(page.contains("COVER &#x2F;images&#x2F;guide.jpg"));
    assert!(page.contains("CTA Read=&#x2F;start&#x2F;"));
    assert!(page
        .contains("ALIASES &#x2F;old-guide&#x2F; &#x2F;legacy&#x2F;guide&#x2F; &#x2F;guide.html"));
    assert!(page.contains("HEADINGS 1:intro:Intro 2:install:Install"));
    assert!(page.contains("TOC intro install"));

    let redirect = read(&output.join("old-guide/index.html"));
    assert!(redirect.contains(r#"<meta http-equiv="refresh" content="0; url=/guide/">"#));
    assert!(redirect.contains(r#"<link rel="canonical" href="/guide/">"#));
    assert!(redirect.contains(r#"<a href="/guide/">/guide/</a>"#));

    let second_redirect = read(&output.join("legacy/guide/index.html"));
    assert!(second_redirect.contains("url=/guide/"));

    let file_redirect = read(&output.join("guide.html"));
    assert!(file_redirect.contains("url=/guide/"));
}

#[test]
fn alias_that_collides_with_real_page_fails_build() {
    let f = FixtureBuilder::new("alias-real-conflict");
    f.write_styles("body {}");
    f.write_page(
        "guide.md",
        r#"title: "Guide"
aliases:
  - /about/"#,
        "Guide body.",
    );
    f.write_page("about.md", r#"title: "About""#, "About body.");

    let config = f.config();
    let err = build(&config, &f.root().join("dist"), false, false).unwrap_err();

    assert!(err
        .to_string()
        .contains("output path conflict at about/index.html"));
    assert!(err.to_string().contains("alias /about/ from"));
    assert!(err.to_string().contains("content/pages/guide.md"));
    assert!(err.to_string().contains("content/pages/about.md"));
}

#[test]
fn duplicate_aliases_fail_build() {
    let f = FixtureBuilder::new("alias-alias-conflict");
    f.write_styles("body {}");
    f.write_page(
        "guide.md",
        r#"title: "Guide"
aliases:
  - /old/"#,
        "Guide body.",
    );
    f.write_page(
        "about.md",
        r#"title: "About"
aliases:
  - /old/"#,
        "About body.",
    );

    let config = f.config();
    let err = build(&config, &f.root().join("dist"), false, false).unwrap_err();

    assert!(err
        .to_string()
        .contains("output path conflict at old/index.html"));
    assert!(err.to_string().contains("alias /old/ from"));
    assert!(err.to_string().contains("content/pages/guide.md"));
    assert!(err.to_string().contains("content/pages/about.md"));
}

#[test]
fn unsafe_alias_paths_fail_before_writing_outputs() {
    for alias in ["/../escaped/", "../escaped", "/a/../../escaped/"] {
        let f = FixtureBuilder::new("unsafe-alias");
        f.write_styles("body {}");
        f.write_page(
            "guide.md",
            &format!(
                r#"title: "Guide"
aliases:
  - "{}""#,
                alias
            ),
            "Guide body.",
        );

        let output = f.root().join("dist");
        let err = build(&f.config(), &output, false, false).unwrap_err();

        assert!(err.to_string().contains("invalid aliases in"));
        assert!(
            err.to_string().contains("parent path segments"),
            "{alias}: {err}"
        );
        assert!(!f.root().join("escaped").exists(), "{alias}");
    }
}

#[test]
fn aliases_that_collide_with_generated_pages_fail_build() {
    for (alias, conflict) in [
        ("/", "generated Home page /"),
        ("/404.html", "generated NotFound page /404.html"),
        ("/tags/", "generated TaxonomyIndex page /tags/"),
    ] {
        let f = FixtureBuilder::new("alias-generated-conflict");
        f.write_styles("body {}");
        f.write_page(
            "guide.md",
            &format!(
                r#"title: "Guide"
aliases:
  - "{}"
tags: ["docs"]"#,
                alias
            ),
            "Guide body.",
        );

        let err = build(&f.config(), &f.root().join("dist"), false, false).unwrap_err();

        assert!(
            err.to_string().contains(conflict),
            "{alias}: expected {conflict} in {err}"
        );
        assert!(err.to_string().contains("alias"));
        assert!(err.to_string().contains("content/pages/guide.md"));
    }
}

#[test]
fn alias_redirect_escapes_target_url_in_html() {
    let f = FixtureBuilder::new("alias-escape");
    f.write_styles("body {}");
    f.write_page(
        "guide.md",
        r#"title: "Guide"
slug: 'bad"<tag>&'
aliases:
  - /old-guide/"#,
        "Guide body.",
    );

    build(&f.config(), &f.root().join("dist"), false, false).unwrap();

    let redirect = read(&f.root().join("dist/old-guide/index.html"));
    assert!(redirect.contains("url=/bad&quot;&lt;tag&gt;&amp;/"));
    assert!(redirect.contains(r#"href="/bad&quot;&lt;tag&gt;&amp;/""#));
    assert!(redirect
        .contains(r#"<a href="/bad&quot;&lt;tag&gt;&amp;/">/bad&quot;&lt;tag&gt;&amp;/</a>"#));
    assert!(!redirect.contains(r#"url=/bad"<tag>&/"#));
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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

    // Fingerprintable files are hashed: read asset_manifest to locate them
    let manifest_path = output.join("asset_manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let mappings = manifest["mappings"].as_object().unwrap();

    let logo_hashed = mappings["images/logo.png"].as_str().unwrap();
    assert!(logo_hashed.contains("logo."));
    assert!(logo_hashed.ends_with(".png"));
    assert_eq!(fs::read(output.join(logo_hashed)).unwrap(), b"PNG_DATA");

    // Non-fingerprintable files keep their original names
    assert_eq!(fs::read(output.join("favicon.ico")).unwrap(), b"ICO_DATA");
    assert_eq!(fs::read(output.join("sub/deep.txt")).unwrap(), b"DEEP");
}

#[test]
fn missing_public_directory_removes_recorded_public_outputs() {
    let f = FixtureBuilder::new("missing-public");
    f.write_styles("body {}");
    f.write_public("images/logo.png", b"PNG_DATA");
    f.write_post(
        "2026-06-01-hello.md",
        r#"title: "Hello"
date: "2026-06-01"
"#,
        "Hello body.",
    );

    let config = f.config();
    let output = f.root().join("dist");
    let mut cache = BuildCache::new();
    let artifacts = BuildArtifacts::load(&config).unwrap();

    build_with_artifacts(
        &config,
        &output,
        Some(&mut cache),
        &artifacts,
        BuildOptions {
            include_drafts: false,
            mode: BuildMode::Full,
            emit_report: true,
            profile: false,
            profile_json: false,
        },
    )
    .unwrap();

    // Logo is fingerprintable; find its actual name from asset_manifest
    let manifest_path = output.join("asset_manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let logo_hashed = manifest["mappings"]["images/logo.png"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(output.join(&logo_hashed).is_file());

    fs::remove_dir_all(f.root().join("public")).unwrap();

    build_with_artifacts(
        &config,
        &output,
        Some(&mut cache),
        &artifacts,
        BuildOptions {
            include_drafts: false,
            mode: BuildMode::Public,
            emit_report: true,
            profile: false,
            profile_json: false,
        },
    )
    .unwrap();

    assert!(
        !output.join(&logo_hashed).exists(),
        "recorded public outputs should be removed when public/ disappears"
    );
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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, true, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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

    build(&config, &output, false, false).unwrap();

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
        Some(&mut cache),
        &artifacts,
        BuildOptions {
            include_drafts: false,
            mode: BuildMode::Full,
            emit_report: true,
            profile: false,
            profile_json: false,
        },
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
        Some(&mut cache),
        &artifacts,
        BuildOptions {
            include_drafts: false,
            mode: BuildMode::Content,
            emit_report: true,
            profile: false,
            profile_json: false,
        },
    )
    .unwrap();

    let post = read(&output.join("posts/change/index.html"));
    assert!(post.contains("Updated Title"));
    assert!(!post.contains("Original Title"));
}

#[test]
fn clean_and_incremental_outputs_match_for_basic_blog_changes() {
    let f = FixtureBuilder::new("matrix-basic-blog");
    f.write_styles("body {}");
    f.write_post(
        "2026-06-01-first.md",
        r#"title: "First"
date: "2026-06-01"
tags: ["rust"]
"#,
        "First body.",
    );
    f.write_post(
        "2026-06-02-second.md",
        r#"title: "Second"
date: "2026-06-02"
tags: ["rust", "kiln"]
"#,
        "Second body.",
    );
    f.write_page("about.md", r#"title: "About""#, "About page.");

    let mut config = f.config();
    config.taxonomies = vec![TaxonomyConfig {
        name: "tags".into(),
        slug: "tags".into(),
        template: "term.html".into(),
    }];
    let incremental_output = f.root().join("dist-incremental");
    let clean_output = f.root().join("dist-clean");
    let mut cache = BuildCache::new();
    let artifacts = BuildArtifacts::load(&config).unwrap();

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Full),
    )
    .unwrap();

    f.write_post(
        "2026-06-02-second.md",
        r#"title: "Second Updated"
date: "2026-06-02"
tags: ["kiln"]
"#,
        "Second body updated.",
    );
    fs::remove_file(f.root().join("content/pages/about.md")).unwrap();

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Content),
    )
    .unwrap();
    build(&config, &clean_output, false, false).unwrap();

    assert!(!incremental_output.join("about/index.html").exists());
    assert_output_dirs_match(
        "basic blog content/delete",
        &incremental_output,
        &clean_output,
    );
}

#[test]
fn clean_and_incremental_outputs_match_for_multi_collection_changes() {
    let f = FixtureBuilder::new("matrix-collections");
    f.write_styles("body {}");
    f.write_template(
        "note.html",
        "NOTE {{ page.title }} {{ page.body_html | safe }}",
    );
    f.write_template(
        "project.html",
        "PROJECT {{ page.title }} {{ page.body_html | safe }}",
    );
    fs::create_dir_all(f.root().join("content/notes")).unwrap();
    fs::create_dir_all(f.root().join("content/projects")).unwrap();
    fs::write(
        f.root().join("content/posts/2026-06-01-post.md"),
        r#"---
title: "Post"
date: "2026-06-01"
---
Post body.
"#,
    )
    .unwrap();
    fs::write(
        f.root().join("content/notes/2026-06-02-note.md"),
        r#"---
title: "Note"
date: "2026-06-02"
---
Note body.
"#,
    )
    .unwrap();
    fs::write(
        f.root().join("content/projects/kiln.md"),
        r#"---
title: "Kiln"
---
Project body.
"#,
    )
    .unwrap();

    let mut config = f.config();
    config.collections = vec![
        CollectionConfig {
            name: "posts".into(),
            directory: "posts".into(),
            route: "/blog/{slug}/".into(),
            template: "post.html".into(),
            date_ordered: true,
            feed: true,
        },
        CollectionConfig {
            name: "notes".into(),
            directory: "notes".into(),
            route: "/notes/{slug}/".into(),
            template: "note.html".into(),
            date_ordered: true,
            feed: true,
        },
        CollectionConfig {
            name: "projects".into(),
            directory: "projects".into(),
            route: "/projects/{slug}/".into(),
            template: "project.html".into(),
            date_ordered: false,
            feed: false,
        },
    ];
    let incremental_output = f.root().join("dist-incremental");
    let clean_output = f.root().join("dist-clean");
    let mut cache = BuildCache::new();
    let artifacts = BuildArtifacts::load(&config).unwrap();

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Full),
    )
    .unwrap();

    fs::write(
        f.root().join("content/notes/2026-06-02-note.md"),
        r#"---
title: "Note Revised"
date: "2026-06-02"
---
Note body revised.
"#,
    )
    .unwrap();

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Content),
    )
    .unwrap();
    build(&config, &clean_output, false, false).unwrap();

    assert_output_dirs_match(
        "multi collection content",
        &incremental_output,
        &clean_output,
    );
}

#[test]
fn clean_and_incremental_outputs_match_for_taxonomy_and_pagination_changes() {
    let f = FixtureBuilder::new("matrix-taxonomy-pagination");
    f.write_styles("body {}");
    f.write_template("tag_term.html", "TERM {{ term.name }}{% for page in term.pages %} {{ page.title }}{% endfor %}{% if paginator %} P{{ paginator.current_index }}{% endif %}");

    for i in 1..=5 {
        let topic = if i % 2 == 0 { "rust" } else { "kiln" };
        f.write_post(
            &format!("2026-06-0{}-post-{}.md", i, i),
            &format!(
                r#"title: "Post {}"
date: "2026-06-0{}"
tags: ["{}"]
"#,
                i, i, topic
            ),
            &format!("Post {} body.", i),
        );
    }

    let mut config = f.config();
    config.paginate_by = 2;
    config.taxonomies = vec![TaxonomyConfig {
        name: "tags".into(),
        slug: "topics".into(),
        template: "tag_term.html".into(),
    }];
    let incremental_output = f.root().join("dist-incremental");
    let clean_output = f.root().join("dist-clean");
    let mut cache = BuildCache::new();
    let artifacts = BuildArtifacts::load(&config).unwrap();

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Full),
    )
    .unwrap();

    fs::remove_file(f.root().join("content/posts/2026-06-05-post-5.md")).unwrap();
    f.write_post(
        "2026-06-04-post-4.md",
        r#"title: "Post 4 Retagged"
date: "2026-06-04"
tags: ["kiln"]
"#,
        "Post 4 body retagged.",
    );

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Content),
    )
    .unwrap();
    build(&config, &clean_output, false, false).unwrap();

    assert!(!incremental_output.join("posts/post-5/index.html").exists());
    assert_output_dirs_match(
        "taxonomy pagination content/delete",
        &incremental_output,
        &clean_output,
    );
}

#[test]
fn clean_and_incremental_outputs_match_for_public_asset_changes() {
    let f = FixtureBuilder::new("matrix-assets");
    f.write_styles("body {}");
    f.write_template(
        "home.html",
        r#"<link rel="stylesheet" href="{{ asset_url(path="css/site.css") | safe }}"><script src="{{ asset_url(path="js/app.js") | safe }}"></script>"#,
    );
    f.write_public(
        "css/site.css",
        b".hero { background: url('../images/logo.png'); }",
    );
    f.write_public("images/logo.png", b"LOGO_V1");
    f.write_public("js/app.js", b"console.log('v1');");
    f.write_post(
        "2026-06-01-assets.md",
        r#"title: "Assets"
date: "2026-06-01"
"#,
        "Asset body.",
    );

    let config = f.config();
    let incremental_output = f.root().join("dist-incremental");
    let clean_output = f.root().join("dist-clean");
    let mut cache = BuildCache::new();
    let artifacts = BuildArtifacts::load(&config).unwrap();

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Full),
    )
    .unwrap();
    let old_manifest: serde_json::Value =
        serde_json::from_str(&read(&incremental_output.join("asset_manifest.json"))).unwrap();
    let old_logo = old_manifest["mappings"]["images/logo.png"]
        .as_str()
        .unwrap()
        .to_string();

    f.write_public(
        "css/site.css",
        b".hero { background: url('../images/logo.png'); color: red; }",
    );
    f.write_public("images/logo.png", b"LOGO_V2");
    f.write_public("js/app.js", b"console.log('v2');");

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Public),
    )
    .unwrap();
    build(&config, &clean_output, false, false).unwrap();

    let index = read(&incremental_output.join("index.html"));
    assert!(index.contains("css/site."));
    assert!(index.contains("js/app."));
    assert!(
        !incremental_output.join(&old_logo).exists(),
        "stale fingerprinted public asset should be pruned"
    );
    assert_output_dirs_match("public asset change", &incremental_output, &clean_output);
}

#[test]
fn clean_and_incremental_outputs_match_for_template_and_shortcode_changes() {
    let f = FixtureBuilder::new("matrix-templates");
    f.write_styles("body {}");
    f.write_template(
        "layout.html",
        "<html><body>Layout v1 {{ body | safe }}</body></html>",
    );
    f.write_template(
        "shortcodes/callout.html",
        "<aside>Callout v1 {{ content }}</aside>",
    );
    f.write_post(
        "2026-06-01-template.md",
        r#"title: "Template"
date: "2026-06-01"
"#,
        "Before\n{{< callout >}}inside{{< /callout >}}\nAfter",
    );

    let config = f.config();
    let incremental_output = f.root().join("dist-incremental");
    let clean_output = f.root().join("dist-clean");
    let mut cache = BuildCache::new();
    let artifacts = BuildArtifacts::load(&config).unwrap();

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Full),
    )
    .unwrap();

    f.write_template(
        "layout.html",
        "<html><body>Layout v2 {{ body | safe }}</body></html>",
    );
    f.write_template(
        "shortcodes/callout.html",
        "<aside>Callout v2 {{ content }}</aside>",
    );
    let artifacts = BuildArtifacts::load(&config).unwrap();
    cache.clear_renders();

    build_with_artifacts(
        &config,
        &incremental_output,
        Some(&mut cache),
        &artifacts,
        build_opts(BuildMode::Content),
    )
    .unwrap();
    build(&config, &clean_output, false, false).unwrap();

    let post = read(&incremental_output.join("posts/template/index.html"));
    assert!(post.contains("Layout v2"));
    assert!(post.contains("Callout v2"));
    assert_output_dirs_match(
        "template and shortcode change",
        &incremental_output,
        &clean_output,
    );
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
