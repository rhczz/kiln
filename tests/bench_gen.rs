use std::fs;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use kiln::{build, AuthorConfig, FeedConfig, PathsConfig, SiteConfig, SiteMeta};

struct BenchSite {
    root: PathBuf,
}

impl Drop for BenchSite {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl BenchSite {
    fn new(post_count: usize) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "kiln-bench-{}-{}-{}",
            post_count,
            std::process::id(),
            now
        ));
        fs::create_dir_all(root.join("content/posts")).unwrap();
        fs::write(root.join("styles.css"), "body { margin: 0; }\n").unwrap();

        for i in 0..post_count {
            let day = ((i % 28) + 1).min(28);
            let month = ((i % 12) + 1).min(12);
            let filename = format!("2024-{:02}-{:02}-post-{:04}.md", month, day, i);
            let frontmatter = format!(
                r#"title: "Benchmark Post {}"
date: "2024-{:02}-{:02}"
description: "Description for benchmark post number {} with enough words to simulate real content."
tags: ["bench", "tag-{}", "perf"]
featured: {}
"#,
                i,
                month,
                day,
                i,
                i % 10,
                i == 0
            );
            let body = format!(
                "## Section One\n\n\
                 This is the body of benchmark post {}. \
                 It contains enough text to simulate a real Markdown document. \
                 The content should be long enough to exercise the rendering pipeline.\n\n\
                 ## Section Two\n\n\
                 | Column A | Column B | Column C |\n\
                 |----------|----------|----------|\n\
                 | {} | {} | {} |\n\n\
                 - [ ] Task item one\n\
                 - [x] Task item two\n\n\
                 ```rust\n\
                 fn main() {{\n\
                     println!(\"post {}\");\n\
                 }}\n\
                 ```\n",
                i,
                i,
                i + 1,
                i + 2,
                i
            );
            fs::write(
                root.join("content/posts").join(&filename),
                format!("---\n{}\n---\n{}", frontmatter, body),
            )
            .unwrap();
        }

        Self { root }
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
                title: "Benchmark Site".into(),
                subtitle: String::new(),
                description: "Benchmark".into(),
                language: "en".into(),
                base_url: "https://bench.test".into(),
            },
            author: Some(AuthorConfig {
                name: "Bench".into(),
                email: String::new(),
            }),
            feed: FeedConfig { item_count: 50 },
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
        }
    }
}

#[test]
#[ignore]
fn bench_100_posts() {
    bench_build(100);
}

#[test]
#[ignore]
fn bench_500_posts() {
    bench_build(500);
}

#[test]
#[ignore]
fn bench_1000_posts() {
    bench_build(1000);
}

#[test]
#[ignore]
fn bench_5000_posts() {
    bench_build(5000);
}

fn bench_build(post_count: usize) {
    let site = BenchSite::new(post_count);
    let config = site.config();
    let output = site.root.join("dist");

    let start = Instant::now();
    build(&config, &output, false).expect("build should succeed");
    let elapsed = start.elapsed();

    eprintln!(
        "\nBenchmark: {} posts built in {:.0}ms ({:.0} posts/sec)",
        post_count,
        elapsed.as_millis(),
        post_count as f64 / elapsed.as_secs_f64()
    );
}
