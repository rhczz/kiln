use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct CliFixture {
    root: PathBuf,
}

impl Drop for CliFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl CliFixture {
    fn new(prefix: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "kiln-cli-{}-{}-{}",
            prefix,
            std::process::id(),
            now
        ));
        fs::create_dir_all(root.join("content/posts")).unwrap();
        Self { root }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn write_profile_site(&self) {
        fs::write(self.root.join("styles.css"), "body {}\n").unwrap();
        fs::write(
            self.root.join("site.config.toml"),
            r#"[site]
title = "CLI Profile"
description = "CLI profile fixture"
base_url = "https://cli.test"

[paths]
content = "content"
templates = "templates"
public = "public"
styles = "styles.css"
"#,
        )
        .unwrap();
        fs::write(
            self.root.join("content/posts/2026-06-01-profile.md"),
            r#"---
title: "Profile Post"
date: "2026-06-01"
---

Profile body.
"#,
        )
        .unwrap();
    }
}

#[test]
fn build_profile_reports_cache_activity_from_cli_path() {
    let fixture = CliFixture::new("profile");
    fixture.write_profile_site();
    let output_dir = fixture.root().join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("build")
        .arg("--config")
        .arg(fixture.root().join("site.config.toml"))
        .arg("--output")
        .arg(&output_dir)
        .arg("--profile")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "kiln build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Profile report:"));
    assert!(stderr.contains("misses"));
    assert!(
        !stderr.contains("no cache activity"),
        "profile output should include cache miss activity, got:\n{}",
        stderr
    );
}
