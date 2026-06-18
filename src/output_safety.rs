use std::path::{Component, Path, PathBuf};

pub(crate) fn ensure_safe_output_target(
    operation: &str,
    output: &Path,
    config_path: &Path,
    config_base_dir: &Path,
    config: &crate::config::SiteConfig,
) -> anyhow::Result<PathBuf> {
    let output = absolute_normalized_path(output)?;
    let output_check_paths = check_paths(&output)?;
    let cwd = check_paths(&std::env::current_dir()?)?;
    let config_dir = check_paths(config_base_dir)?;

    for check_path in &output_check_paths {
        if is_filesystem_root(check_path) {
            return Err(unsafe_output_error(
                operation,
                &output,
                "is the filesystem root",
            ));
        }
        if cwd.iter().any(|cwd_path| check_path == cwd_path) {
            return Err(unsafe_output_error(
                operation,
                &output,
                "matches the current working directory",
            ));
        }
        if config_dir
            .iter()
            .any(|config_dir_path| check_path == config_dir_path)
        {
            return Err(unsafe_output_error(
                operation,
                &output,
                "matches the config directory",
            ));
        }
    }

    for home in home_dirs()? {
        let home_paths = check_paths(&home)?;
        if output_check_paths
            .iter()
            .any(|output| home_paths.iter().any(|home| output == home))
        {
            return Err(unsafe_output_error(
                operation,
                &output,
                "matches the home directory",
            ));
        }
    }

    for (label, protected) in protected_site_paths(config_path, config)? {
        if output_check_paths
            .iter()
            .any(|output| paths_overlap(output, &protected))
        {
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
    let config_path = absolute_normalized_path(config_path)?;
    let config_base_dir = config_path.parent().unwrap_or(Path::new("."));
    let mut paths = vec![
        ("config file", canonical_check_path(&config_path)?),
        (
            "content directory",
            canonical_check_path(Path::new(&config.paths.content))?,
        ),
        (
            "templates directory",
            canonical_check_path(Path::new(&config.paths.templates))?,
        ),
        (
            "public directory",
            canonical_check_path(Path::new(&config.paths.public))?,
        ),
        (
            "styles file",
            canonical_check_path(Path::new(&config.paths.styles))?,
        ),
    ];

    for (label, raw_path) in raw_site_paths(&config_path, config_base_dir)? {
        paths.push((label, raw_path));
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn absolute_normalized_path(path: &Path) -> anyhow::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(normalize_path(&absolute))
}

fn canonical_check_path(path: &Path) -> anyhow::Result<PathBuf> {
    let path = absolute_normalized_path(path)?;
    if let Ok(canonical) = std::fs::canonicalize(&path) {
        return Ok(canonical);
    }

    let mut existing = path.as_path();
    let mut missing = Vec::new();
    while !existing.exists() {
        if let Some(name) = existing.file_name() {
            missing.push(name.to_os_string());
        }
        let Some(parent) = existing.parent() else {
            return Ok(path);
        };
        existing = parent;
    }

    let mut canonical = std::fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    for component in missing.iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

fn check_paths(path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let normalized = absolute_normalized_path(path)?;
    let canonical = canonical_check_path(&normalized)?;
    if canonical == normalized {
        Ok(vec![normalized])
    } else {
        Ok(vec![normalized, canonical])
    }
}

fn raw_site_paths(
    config_path: &Path,
    config_base_dir: &Path,
) -> anyhow::Result<Vec<(&'static str, PathBuf)>> {
    #[derive(serde::Deserialize)]
    struct RawConfig {
        #[serde(default)]
        paths: crate::config::PathsConfig,
    }

    let content = std::fs::read_to_string(config_path)?;
    let raw: RawConfig = toml::from_str(&content)?;
    Ok(vec![
        (
            "configured content path",
            absolute_normalized_from(config_base_dir, Path::new(&raw.paths.content)),
        ),
        (
            "configured templates path",
            absolute_normalized_from(config_base_dir, Path::new(&raw.paths.templates)),
        ),
        (
            "configured public path",
            absolute_normalized_from(config_base_dir, Path::new(&raw.paths.public)),
        ),
        (
            "configured styles path",
            absolute_normalized_from(config_base_dir, Path::new(&raw.paths.styles)),
        ),
    ])
}

fn absolute_normalized_from(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize_path(path)
    } else {
        normalize_path(&base.join(path))
    }
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

fn is_filesystem_root(path: &Path) -> bool {
    path.components()
        .all(|component| !matches!(component, Component::Normal(_)))
}

fn home_dirs() -> anyhow::Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for key in ["HOME", "USERPROFILE"] {
        if let Some(value) = std::env::var_os(key) {
            dirs.push(absolute_normalized_path(Path::new(&value))?);
        }
    }
    if let (Some(drive), Some(path)) = (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
    {
        let mut home = PathBuf::from(drive);
        home.push(path);
        dirs.push(absolute_normalized_path(&home)?);
    }
    dirs.sort();
    dirs.dedup();
    Ok(dirs)
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

        fn reload_config(&mut self) {
            let (config, base_dir) = crate::config::SiteConfig::load(&self.config_path).unwrap();
            self.config = config;
            self.base_dir = base_dir;
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

        assert_eq!(output, site.site_dir().join("dist"));
    }

    #[test]
    fn allows_parent_dir_segments_after_normalization() {
        let site = TestSite::new("parent-segments");
        std::fs::create_dir_all(site.site_dir().join("nested")).unwrap();

        let output = site
            .ensure(&site.site_dir().join("nested/../dist"))
            .unwrap();

        assert_eq!(output, site.site_dir().join("dist"));
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

        assert!(err.to_string().contains("content"));
    }

    #[test]
    fn rejects_output_nested_inside_source_directory() {
        let site = TestSite::new("nested-source");

        let err = site
            .ensure(&site.site_dir().join("content/generated"))
            .unwrap_err();

        assert!(err.to_string().contains("content"));
    }

    #[cfg(unix)]
    #[test]
    fn preserves_symlink_output_path_when_target_is_safe() {
        let site = TestSite::new("symlink-output");
        let safe_target = site.root.join("external-output");
        std::fs::create_dir_all(&safe_target).unwrap();
        let link = site.site_dir().join("dist-link");
        std::os::unix::fs::symlink(&safe_target, &link).unwrap();

        let output = site.ensure(&link).unwrap();

        assert_eq!(output, link);
        assert_ne!(output, std::fs::canonicalize(safe_target).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_output_that_resolves_to_source_directory() {
        let site = TestSite::new("symlink-source");
        let link = site.site_dir().join("output-link");
        std::os::unix::fs::symlink(site.site_dir().join("content"), &link).unwrap();

        let err = site.ensure(&link).unwrap_err();

        assert!(err.to_string().contains("content"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_parent_of_raw_configured_symlink_source_path() {
        let mut site = TestSite::new("raw-symlink-source");
        let external_content = site.root.join("external-content");
        std::fs::create_dir_all(&external_content).unwrap();
        std::fs::create_dir_all(site.site_dir().join("sources")).unwrap();
        std::os::unix::fs::symlink(
            &external_content,
            site.site_dir().join("sources/content-link"),
        )
        .unwrap();
        std::fs::write(
            &site.config_path,
            r#"[site]
title = "Output Safety"
description = "Output safety fixture"
base_url = "https://safety.test"

[paths]
content = "sources/content-link"
templates = "templates"
public = "public"
styles = "styles.css"
"#,
        )
        .unwrap();
        site.reload_config();

        let err = site.ensure(&site.site_dir().join("sources")).unwrap_err();

        assert!(err.to_string().contains("configured content path"));
    }
}
