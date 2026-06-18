use std::path::{Component, Path, PathBuf};

pub(crate) fn ensure_safe_output_target(
    operation: &str,
    output: &Path,
    config_path: &Path,
    config_base_dir: &Path,
    config: &crate::config::SiteConfig,
) -> anyhow::Result<PathBuf> {
    let output = safe_absolute_path(output)?;
    let cwd = safe_absolute_path(&std::env::current_dir()?)?;
    let config_dir = safe_absolute_path(config_base_dir)?;

    if output == Path::new("/") || output == cwd || output == config_dir {
        let reason = if output == Path::new("/") {
            "is the filesystem root"
        } else if output == cwd {
            "matches the current working directory"
        } else {
            "matches the config directory"
        };
        return Err(unsafe_output_error(operation, &output, reason));
    }

    if let Some(home) = std::env::var_os("HOME") {
        if output == safe_absolute_path(Path::new(&home))? {
            return Err(unsafe_output_error(
                operation,
                &output,
                "matches the home directory",
            ));
        }
    }

    for (label, protected) in protected_site_paths(config_path, config)? {
        if paths_overlap(&output, &protected) {
            return Err(unsafe_output_error(
                operation,
                &output,
                &format!("overlaps {label}"),
            ));
        }
    }

    Ok(output)
}

fn protected_site_paths(
    config_path: &Path,
    config: &crate::config::SiteConfig,
) -> anyhow::Result<Vec<(&'static str, PathBuf)>> {
    Ok(vec![
        ("config file", safe_absolute_path(config_path)?),
        (
            "content directory",
            safe_absolute_path(Path::new(&config.paths.content))?,
        ),
        (
            "templates directory",
            safe_absolute_path(Path::new(&config.paths.templates))?,
        ),
        (
            "public directory",
            safe_absolute_path(Path::new(&config.paths.public))?,
        ),
        (
            "styles file",
            safe_absolute_path(Path::new(&config.paths.styles))?,
        ),
    ])
}

fn safe_absolute_path(path: &Path) -> anyhow::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let absolute = normalize_path(&absolute);

    if let Ok(canonical) = std::fs::canonicalize(&absolute) {
        return Ok(canonical);
    }

    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    while !existing.exists() {
        if let Some(name) = existing.file_name() {
            missing.push(name.to_os_string());
        }
        let Some(parent) = existing.parent() else {
            return Ok(absolute);
        };
        existing = parent;
    }

    let mut canonical = std::fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    for component in missing.iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(_) | Component::RootDir | Component::Prefix(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn paths_overlap(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

fn unsafe_output_error(operation: &str, output: &Path, reason: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "Refusing to {} into unsafe output path {} because it {}",
        operation,
        output.display(),
        reason
    )
}

#[cfg(test)]
mod tests {
    use super::ensure_safe_output_target;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestSite {
        root: PathBuf,
        config_path: PathBuf,
        config: crate::config::SiteConfig,
        base_dir: PathBuf,
    }

    impl Drop for TestSite {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl TestSite {
        fn new(name: &str) -> Self {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "kiln-output-safety-{name}-{}-{now}",
                std::process::id()
            ));
            let site = root.join("site");
            std::fs::create_dir_all(site.join("content/posts")).unwrap();
            std::fs::create_dir_all(site.join("templates")).unwrap();
            std::fs::create_dir_all(site.join("public")).unwrap();
            std::fs::write(site.join("styles.css"), "body {}\n").unwrap();
            let config_path = site.join("site.config.toml");
            std::fs::write(
                &config_path,
                r#"[site]
title = "Output Safety"
description = "Output safety fixture"
base_url = "https://safety.test"

[paths]
content = "content"
templates = "templates"
public = "public"
styles = "styles.css"
"#,
            )
            .unwrap();
            let (config, base_dir) = crate::config::SiteConfig::load(&config_path).unwrap();
            Self {
                root,
                config_path,
                config,
                base_dir,
            }
        }

        fn site_dir(&self) -> &Path {
            &self.base_dir
        }

        fn canonical_site_dir(&self) -> PathBuf {
            std::fs::canonicalize(&self.base_dir).unwrap()
        }

        fn ensure(&self, output: &Path) -> anyhow::Result<PathBuf> {
            ensure_safe_output_target(
                "build",
                output,
                &self.config_path,
                &self.base_dir,
                &self.config,
            )
        }
    }

    #[test]
    fn allows_normal_dist_output() {
        let site = TestSite::new("normal-dist");

        let output = site.ensure(&site.site_dir().join("dist")).unwrap();

        assert_eq!(output, site.canonical_site_dir().join("dist"));
    }

    #[test]
    fn allows_parent_dir_segments_after_normalization() {
        let site = TestSite::new("parent-segments");
        std::fs::create_dir_all(site.site_dir().join("nested")).unwrap();

        let output = site
            .ensure(&site.site_dir().join("nested/../dist"))
            .unwrap();

        assert_eq!(output, site.canonical_site_dir().join("dist"));
    }

    #[test]
    fn rejects_config_directory() {
        let site = TestSite::new("config-dir");

        let err = site.ensure(site.site_dir()).unwrap_err();

        assert!(err.to_string().contains("matches the config directory"));
    }

    #[test]
    fn rejects_filesystem_root() {
        let site = TestSite::new("root");

        let err = site.ensure(Path::new("/")).unwrap_err();

        assert!(err.to_string().contains("filesystem root"));
    }

    #[test]
    fn rejects_current_working_directory() {
        let site = TestSite::new("cwd");

        let err = site.ensure(&std::env::current_dir().unwrap()).unwrap_err();

        assert!(err.to_string().contains("current working directory"));
    }

    #[test]
    fn rejects_home_directory_when_available() {
        let site = TestSite::new("home");
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };

        let err = site.ensure(Path::new(&home)).unwrap_err();

        assert!(err.to_string().contains("home directory"));
    }

    #[test]
    fn rejects_source_directory() {
        let site = TestSite::new("source-dir");

        let err = site.ensure(&site.site_dir().join("content")).unwrap_err();

        assert!(err.to_string().contains("content directory"));
    }

    #[test]
    fn rejects_output_nested_inside_source_directory() {
        let site = TestSite::new("nested-source");

        let err = site
            .ensure(&site.site_dir().join("content/generated"))
            .unwrap_err();

        assert!(err.to_string().contains("content directory"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_output_that_resolves_to_source_directory() {
        let site = TestSite::new("symlink-source");
        let link = site.site_dir().join("output-link");
        std::os::unix::fs::symlink(site.site_dir().join("content"), &link).unwrap();

        let err = site.ensure(&link).unwrap_err();

        assert!(err.to_string().contains("content directory"));
    }
}
