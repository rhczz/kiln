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

    fn write_built_output(&self) {
        fs::create_dir_all(self.root.join("dist/.kiln")).unwrap();
        fs::create_dir_all(self.root.join("dist/posts/hello")).unwrap();
        fs::write(self.root.join("dist/index.html"), "<html></html>").unwrap();
        fs::write(
            self.root.join("dist/posts/hello/index.html"),
            "<html></html>",
        )
        .unwrap();
        fs::write(self.root.join("dist/rss.xml"), "<rss></rss>").unwrap();
        fs::write(
            self.root.join("dist/_headers"),
            "/assets/*\n  Cache-Control: public, max-age=31536000, immutable\n",
        )
        .unwrap();
        fs::create_dir_all(self.root.join("dist/css")).unwrap();
        fs::write(self.root.join("dist/css/app.abc123.css"), "body {}\n").unwrap();
        fs::write(self.root.join("dist/notes.txt"), "generated asset\n").unwrap();
        fs::write(
            self.root.join("dist/asset_manifest.json"),
            r#"{
  "mappings": {
    "css/app.css": "css/app.abc123.css",
    "notes.txt": "notes.txt"
  }
}"#,
        )
        .unwrap();
        fs::write(
            self.root.join("dist/.kiln/manifest.json"),
            r#"{
  "entries": [
    {
      "source": "content/posts/hello.md",
      "outputs": ["posts/hello/index.html"],
      "content_hash": "hello",
      "template_deps": [],
      "template_hash": ""
    },
    {
      "source": "home",
      "outputs": ["index.html"],
      "content_hash": "home",
      "template_deps": [],
      "template_hash": ""
    }
  ],
  "config_hash": "config"
}"#,
        )
        .unwrap();
    }

    fn write_route_conflict_site(&self) {
        fs::create_dir_all(self.root.join("content/pages")).unwrap();
        fs::write(self.root.join("styles.css"), "body {}\n").unwrap();
        fs::write(
            self.root.join("site.config.toml"),
            r#"[site]
title = "Route Conflict"
description = "Route conflict fixture"
base_url = "https://conflict.test"

[paths]
content = "content"
templates = "templates"
public = "public"
styles = "styles.css"

[[collections]]
name = "posts"
directory = "posts"
route = "/{slug}/"
template = "post.html"
date_ordered = true
feed = true

[[collections]]
name = "pages"
directory = "pages"
route = "/{slug}/"
template = "page.html"
date_ordered = false
feed = false
"#,
        )
        .unwrap();
        fs::write(
            self.root.join("content/posts/2026-06-01-about.md"),
            r#"---
title: "Post About"
date: "2026-06-01"
slug: "about"
---

Post body.
"#,
        )
        .unwrap();
        fs::write(
            self.root.join("content/pages/about.md"),
            r#"---
title: "Page About"
---

Page body.
"#,
        )
        .unwrap();
    }

    fn write_generated_route_conflict_site(&self) {
        fs::create_dir_all(self.root.join("content/pages")).unwrap();
        fs::write(self.root.join("styles.css"), "body {}\n").unwrap();
        fs::write(
            self.root.join("site.config.toml"),
            r#"[site]
title = "Generated Route Conflict"
description = "Generated route conflict fixture"
base_url = "https://generated-conflict.test"

[paths]
content = "content"
templates = "templates"
public = "public"
styles = "styles.css"

[[taxonomies]]
name = "tags"
slug = "tags"
"#,
        )
        .unwrap();
        fs::write(
            self.root.join("content/posts/2026-06-01-tagged.md"),
            r#"---
title: "Tagged"
date: "2026-06-01"
tags: ["kiln"]
---

Tagged post.
"#,
        )
        .unwrap();
        fs::write(
            self.root.join("content/pages/tags.md"),
            r#"---
title: "Tags Page"
---

This page collides with the generated taxonomy index.
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

#[test]
fn build_profile_json_outputs_machine_readable_profile() {
    let fixture = CliFixture::new("profile-json");
    fixture.write_profile_site();
    let output_dir = fixture.root().join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("build")
        .arg("--config")
        .arg(fixture.root().join("site.config.toml"))
        .arg("--output")
        .arg(&output_dir)
        .arg("--profile-json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "kiln build --profile-json failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    let profile: serde_json::Value =
        serde_json::from_str(&stderr).expect("profile output should be valid JSON");

    assert_eq!(profile["schema_version"], 1);
    assert!(profile["total_ms"].as_u64().is_some());
    assert!(profile["phases"]
        .as_array()
        .expect("phases should be an array")
        .iter()
        .any(|phase| phase["name"] == "render_pages"));
    assert!(profile["cache"]["misses"].as_u64().unwrap() > 0);
    assert!(profile["rendering"]["page_renders"].as_u64().unwrap() > 0);
    assert!(
        profile["parallel"]["threads"].as_u64().unwrap() > 0,
        "parallel stats should include worker count"
    );
}

#[test]
fn build_rejects_profile_and_profile_json_together() {
    let fixture = CliFixture::new("profile-conflict");
    fixture.write_profile_site();

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("build")
        .arg("--config")
        .arg(fixture.root().join("site.config.toml"))
        .arg("--profile")
        .arg("--profile-json")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "kiln build should reject conflicting profile flags\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--profile"), "{stderr}");
    assert!(stderr.contains("--profile-json"), "{stderr}");
}

#[test]
fn build_refuses_output_at_site_source_directory() {
    let fixture = CliFixture::new("build-source-output");
    fixture.write_profile_site();
    let source_file = fixture.root().join("content/posts/2026-06-01-profile.md");

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("build")
        .arg("--config")
        .arg(fixture.root().join("site.config.toml"))
        .arg("--output")
        .arg(fixture.root().join("content"))
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "kiln build should refuse source output\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to build into unsafe output path"),
        "{stderr}"
    );
    assert!(
        source_file.is_file(),
        "build must not delete source content"
    );
}

#[test]
fn build_refuses_output_nested_inside_site_source_directory() {
    let fixture = CliFixture::new("build-nested-source-output");
    fixture.write_profile_site();
    let nested_output = fixture.root().join("content/generated");

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("build")
        .arg("--config")
        .arg(fixture.root().join("site.config.toml"))
        .arg("--output")
        .arg(&nested_output)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "kiln build should refuse nested source output\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Refusing to build into unsafe output path"),
        "{stderr}"
    );
    assert!(
        !nested_output.exists(),
        "build must not write inside source content"
    );
}

#[test]
fn init_creates_a_site_that_builds() {
    let fixture = CliFixture::new("init");
    let site_dir = fixture.root().join("my-site");
    fs::remove_dir_all(&fixture.root).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("init")
        .arg(&site_dir)
        .arg("--title")
        .arg("CLI Init")
        .arg("--base-url")
        .arg("https://init.test")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "kiln init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(site_dir.join("site.config.toml").is_file());
    assert!(site_dir.join("content/posts/hello.md").is_file());
    assert!(site_dir.join("styles.css").is_file());
    assert!(site_dir.join("public").is_dir());

    let dist = site_dir.join("dist");
    let build = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("build")
        .arg("--config")
        .arg(site_dir.join("site.config.toml"))
        .arg("--output")
        .arg(&dist)
        .output()
        .unwrap();

    assert!(
        build.status.success(),
        "generated site did not build\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(dist.join("index.html").is_file());
    assert!(dist.join("posts/hello/index.html").is_file());
}

#[test]
fn doctor_reports_project_health_without_writing_dist() {
    let fixture = CliFixture::new("doctor");
    fixture.write_profile_site();
    let dist = fixture.root().join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("doctor")
        .arg("--config")
        .arg(fixture.root().join("site.config.toml"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "kiln doctor failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("kiln doctor"));
    assert!(stderr.contains("dry build completed without writing dist"));
    assert!(
        !dist.exists(),
        "doctor should not write the configured dist"
    );
}

#[test]
fn clean_removes_generated_output_but_keeps_cache_by_default() {
    let fixture = CliFixture::new("clean-output");
    fixture.write_built_output();
    let dist = fixture.root().join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("clean")
        .arg("--output")
        .arg(&dist)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "kiln clean failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dist.join("index.html").exists());
    assert!(!dist.join("posts").exists());
    assert!(!dist.join("rss.xml").exists());
    assert!(!dist.join("_headers").exists());
    assert!(!dist.join("css/app.abc123.css").exists());
    assert!(!dist.join("css").exists());
    assert!(!dist.join("notes.txt").exists());
    assert!(!dist.join("asset_manifest.json").exists());
    assert!(dist.join(".kiln/manifest.json").is_file());
}

#[test]
fn clean_cache_removes_only_kiln_state() {
    let fixture = CliFixture::new("clean-cache");
    fixture.write_built_output();
    let dist = fixture.root().join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("clean")
        .arg("--output")
        .arg(&dist)
        .arg("--cache")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "kiln clean --cache failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dist.join("index.html").is_file());
    assert!(dist.join("posts/hello/index.html").is_file());
    assert!(!dist.join(".kiln").exists());
}

#[test]
fn clean_cache_refuses_non_kiln_state_directory() {
    let fixture = CliFixture::new("clean-cache-refusal");
    let output_dir = fixture.root().join("other-output");
    fs::create_dir_all(output_dir.join(".kiln")).unwrap();
    fs::write(output_dir.join(".kiln/state.json"), "{}").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("clean")
        .arg("--output")
        .arg(&output_dir)
        .arg("--cache")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "kiln clean --cache should refuse non-kiln state\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing .kiln/manifest.json"), "{stderr}");
    assert!(output_dir.join(".kiln/state.json").is_file());
}

#[test]
fn clean_refuses_source_like_directory_without_manifest() {
    let fixture = CliFixture::new("clean-source-refusal");
    fixture.write_profile_site();

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("clean")
        .arg("--output")
        .arg(fixture.root())
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "kiln clean should refuse source-like directories\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing .kiln/manifest.json"), "{stderr}");
    assert!(fixture.root().join("site.config.toml").is_file());
    assert!(fixture.root().join("content/posts").is_dir());
    assert!(fixture.root().join("styles.css").is_file());
}

#[test]
fn init_refuses_non_empty_directory() {
    let fixture = CliFixture::new("init-non-empty");
    fs::write(fixture.root().join("keep.txt"), "do not touch\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("init")
        .arg(fixture.root())
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "kiln init should refuse non-empty directories\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("directory is not empty"), "{stderr}");
    assert!(fixture.root().join("keep.txt").is_file());
    assert!(!fixture.root().join("site.config.toml").exists());
}

#[test]
fn init_doctor_build_clean_and_rebuild_roundtrip() {
    let fixture = CliFixture::new("product-roundtrip");
    let site_dir = fixture.root().join("site");
    let dist = site_dir.join("dist");

    let init = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("init")
        .arg(&site_dir)
        .arg("--title")
        .arg("Roundtrip \"Site\"")
        .arg("--base-url")
        .arg("https://roundtrip.test")
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "kiln init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&init.stdout),
        String::from_utf8_lossy(&init.stderr)
    );

    let doctor = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("doctor")
        .arg("--config")
        .arg(site_dir.join("site.config.toml"))
        .output()
        .unwrap();
    assert!(
        doctor.status.success(),
        "kiln doctor failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );
    let doctor_stderr = String::from_utf8_lossy(&doctor.stderr);
    assert!(doctor_stderr.contains("Doctor passed with 0 warning(s)."));

    let build = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("build")
        .arg("--config")
        .arg(site_dir.join("site.config.toml"))
        .arg("--output")
        .arg(&dist)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "kiln build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(dist.join("index.html").is_file());
    assert!(dist.join("posts/hello/index.html").is_file());
    assert!(dist.join(".kiln/manifest.json").is_file());

    let clean = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("clean")
        .arg("--output")
        .arg(&dist)
        .output()
        .unwrap();
    assert!(
        clean.status.success(),
        "kiln clean failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&clean.stdout),
        String::from_utf8_lossy(&clean.stderr)
    );
    assert!(!dist.join("index.html").exists());
    assert!(dist.join(".kiln/manifest.json").is_file());

    let rebuild = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("build")
        .arg("--config")
        .arg(site_dir.join("site.config.toml"))
        .arg("--output")
        .arg(&dist)
        .output()
        .unwrap();
    assert!(
        rebuild.status.success(),
        "kiln rebuild failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&rebuild.stdout),
        String::from_utf8_lossy(&rebuild.stderr)
    );
    assert!(dist.join("index.html").is_file());
    assert!(dist.join("posts/hello/index.html").is_file());
}

#[test]
fn doctor_fails_when_collections_generate_the_same_route() {
    let fixture = CliFixture::new("doctor-route-conflict");
    fixture.write_route_conflict_site();

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("doctor")
        .arg("--config")
        .arg(fixture.root().join("site.config.toml"))
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "kiln doctor should fail for route conflicts\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("output path conflict at about/index.html"),
        "{stderr}"
    );
    assert!(
        stderr.contains("change one slug, collection route, taxonomy slug, or section path"),
        "{stderr}"
    );
}

#[test]
fn doctor_fails_when_content_collides_with_generated_page() {
    let fixture = CliFixture::new("doctor-generated-route-conflict");
    fixture.write_generated_route_conflict_site();

    let output = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("doctor")
        .arg("--config")
        .arg(fixture.root().join("site.config.toml"))
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "kiln doctor should fail for content/generated output conflicts\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("output path conflict at tags/index.html"),
        "{stderr}"
    );
    assert!(
        stderr.contains("change one slug, collection route, taxonomy slug, or section path"),
        "{stderr}"
    );
}
