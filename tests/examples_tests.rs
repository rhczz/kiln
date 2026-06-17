use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kiln::{build, SiteConfig};

struct ExampleOutput {
    root: PathBuf,
}

impl Drop for ExampleOutput {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl ExampleOutput {
    fn new(name: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "kiln-example-{}-{}-{}",
            name,
            std::process::id(),
            now
        ));
        Self { root }
    }
}

#[test]
fn examples_build_successfully() {
    for (example, expected_outputs) in [
        (
            "blog-basic",
            &[
                "index.html",
                "page/2/index.html",
                "posts/welcome/index.html",
                "posts/assets/index.html",
                "about/index.html",
                "tags/index.html",
                "tags/kiln/index.html",
                "asset_manifest.json",
                "sitemap.xml",
            ][..],
        ),
        (
            "docs-site",
            &[
                "index.html",
                "docs/index.html",
                "docs/intro/index.html",
                "docs/reference/index.html",
                "note/index.html",
                "asset_manifest.json",
                "sitemap.xml",
            ][..],
        ),
        (
            "portfolio",
            &[
                "index.html",
                "work/index.html",
                "work/atlas/index.html",
                "work/linea/index.html",
                "about/index.html",
                "asset_manifest.json",
                "sitemap.xml",
            ][..],
        ),
    ] {
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(example)
            .join("site.config.toml");
        let (config, _base_dir) = SiteConfig::load(&config_path).unwrap();
        let output = ExampleOutput::new(example);

        build(&config, &output.root, false, false)
            .unwrap_or_else(|e| panic!("example {example} failed to build: {e}"));

        for expected in expected_outputs {
            assert!(
                output.root.join(expected).is_file(),
                "example {example} did not write {expected}"
            );
        }
    }
}
