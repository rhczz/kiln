use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Maps original asset paths to their fingerprinted (hashed) paths.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetManifest {
    pub mappings: HashMap<String, String>,
}

impl AssetManifest {
    pub fn load(output_dir: &Path) -> anyhow::Result<Self> {
        let path = output_dir.join("asset_manifest.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        match serde_json::from_str(&content) {
            Ok(manifest) => Ok(manifest),
            Err(e) => {
                eprintln!(
                    "Warning: failed to parse asset_manifest.json ({}), rebuilding all assets",
                    e
                );
                Ok(Self::default())
            }
        }
    }

    pub fn save(&self, output_dir: &Path) -> anyhow::Result<()> {
        let path = output_dir.join("asset_manifest.json");
        let sorted = serde_json::json!({
            "mappings": self
                .mappings
                .iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
        });
        let json = serde_json::to_string_pretty(&sorted)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn resolve(&self, original: &str) -> String {
        self.mappings
            .get(original)
            .cloned()
            .unwrap_or_else(|| original.to_string())
    }

    /// Stable content hash of all mappings (sorted by key).
    pub fn content_hash(&self) -> String {
        let mut keys: Vec<&String> = self.mappings.keys().collect();
        keys.sort();
        let mut combined = String::new();
        for k in keys {
            combined.push_str(k);
            combined.push('\0');
            combined.push_str(&self.mappings[k]);
            combined.push('\0');
        }
        if combined.is_empty() {
            return String::new();
        }
        crate::content::fingerprint(combined.as_bytes())
    }
}

/// File extensions that should be fingerprinted.
const FINGERPRINTABLE_EXTENSIONS: &[&str] = &[
    "css", "js", "svg", "png", "jpg", "jpeg", "gif", "webp", "woff", "woff2", "ttf", "eot",
];

fn is_fingerprintable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| FINGERPRINTABLE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

/// Scan public/ directory, fingerprint eligible files, and copy to output.
/// Returns an AssetManifest mapping original paths → fingerprinted paths.
pub fn fingerprint_public(public_dir: &Path, output_dir: &Path) -> anyhow::Result<AssetManifest> {
    let mut manifest = AssetManifest::default();

    if !public_dir.is_dir() {
        return Ok(manifest);
    }

    let entries = collect_files(public_dir)?;
    for entry in &entries {
        let relative = entry.strip_prefix(public_dir).unwrap_or(entry);
        let rel_str = relative.to_string_lossy().to_string();

        if is_fingerprintable(entry) {
            let content = std::fs::read(entry)?;
            let hash = crate::content::fingerprint(&content);
            let stem = entry.file_stem().unwrap_or_default().to_string_lossy();
            let ext = entry.extension().unwrap_or_default().to_string_lossy();
            let fingerprinted = format!("{}.{}.{}", stem, hash, ext);

            // Join parent dir (if any) with the fingerprinted filename
            let join_with_parent =
                |base: &str| match relative.parent().filter(|p| !p.as_os_str().is_empty()) {
                    Some(parent) => format!("{}/{}", parent.to_string_lossy(), base),
                    None => base.to_string(),
                };

            let dest = output_dir.join(join_with_parent(&fingerprinted));
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&dest, &content)?;

            manifest
                .mappings
                .insert(rel_str, join_with_parent(&fingerprinted));
        } else {
            let dest = output_dir.join(relative);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry, &dest)?;
            manifest.mappings.insert(rel_str.clone(), rel_str);
        }
    }

    // Second pass: rewrite url() references in fingerprinted CSS files
    for entry in &entries {
        if entry.extension().is_some_and(|e| e == "css") && is_fingerprintable(entry) {
            let relative = entry.strip_prefix(public_dir).unwrap_or(entry);
            let rel_str = relative.to_string_lossy().to_string();
            if let Some(fingerprinted_path) = manifest.mappings.get(&rel_str) {
                let dest = output_dir.join(fingerprinted_path);
                if dest.exists() {
                    let content = std::fs::read_to_string(&dest)?;
                    let rewritten = rewrite_css_urls(&content, &rel_str, &manifest);
                    if rewritten != content {
                        std::fs::write(&dest, &rewritten)?;
                    }
                }
            }
        }
    }

    Ok(manifest)
}

/// Rewrite `url()` references in CSS to point to fingerprinted asset paths.
///
/// CSS at `css/style.css` with `url(../images/logo.png)`:
/// - resolves `../images/logo.png` relative to `css/` → `images/logo.png`
/// - looks up `images/logo.png` in manifest → `images/logo.abc123.png`
/// - rewrites to `url(../images/logo.abc123.png)` (same relative prefix, hashed filename)
fn rewrite_css_urls(css: &str, css_original_path: &str, manifest: &AssetManifest) -> String {
    let css_dir = Path::new(css_original_path)
        .parent()
        .unwrap_or(Path::new(""));
    let mut result = String::with_capacity(css.len());
    let mut rest = css;

    while let Some(url_start) = rest.find("url(") {
        let before = &rest[..url_start];
        result.push_str(before);

        let after_url = &rest[url_start + 4..]; // skip "url("
        let (url_value, quote, remaining) = match after_url.chars().next() {
            Some('"') => {
                let (v, r) = extract_quoted(after_url, '"');
                (v, Some('"'), r)
            }
            Some('\'') => {
                let (v, r) = extract_quoted(after_url, '\'');
                (v, Some('\''), r)
            }
            Some(_) => {
                let (v, r) = extract_unquoted(after_url);
                (v, None, r)
            }
            None => break,
        };

        let new_value = if let Some(resolved) = resolve_and_lookup(url_value, css_dir, manifest) {
            rel_path(css_dir, &resolved)
        } else {
            url_value.to_string()
        };

        match quote {
            Some('"') => result.push_str(&format!("url(\"{}\")", new_value)),
            Some('\'') => result.push_str(&format!("url('{}')", new_value)),
            Some(_) => unreachable!(),
            None => result.push_str(&format!("url({})", new_value)),
        }

        rest = remaining;
    }

    result.push_str(rest);
    result
}

/// Extract a quoted string, returning (inner_value, rest_after_closing).
fn extract_quoted(s: &str, quote: char) -> (&str, &str) {
    let inner = &s[1..]; // skip opening quote
    if let Some(end) = inner.find(quote) {
        (&inner[..end], &inner[end + 1..])
    } else {
        (inner, "")
    }
}

/// Extract an unquoted url() value (no quotes), returning (value, rest_after_closing_paren).
fn extract_unquoted(s: &str) -> (&str, &str) {
    let end = s.find(')').unwrap_or(s.len());
    (s[..end].trim(), &s[end + 1..])
}

/// Resolve a url() value relative to the CSS file's directory and look it up in the manifest.
/// Returns the fingerprinted path if found.
fn resolve_and_lookup(url_value: &str, css_dir: &Path, manifest: &AssetManifest) -> Option<String> {
    // Skip data: urls, http(s): urls, fragments
    let trimmed = url_value.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("data:")
        || trimmed.starts_with("http:")
        || trimmed.starts_with("https:")
        || trimmed.starts_with('#')
    {
        return None;
    }

    // Resolve relative to css_dir
    let resolved = css_dir.join(trimmed);
    // Normalize: drop leading ./ and resolve ..
    let normalized = normalize_path(&resolved);
    manifest.mappings.get(&normalized).cloned()
}

/// Normalize a path: resolve `.` and `..`, produce a clean relative path.
fn normalize_path(path: &Path) -> String {
    let mut components: Vec<&str> = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(c) => components.push(c.to_str().unwrap_or("")),
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            _ => {}
        }
    }
    components.join("/")
}

/// Compute the relative path from `base` dir to `target` path.
fn rel_path(base: &Path, target: &str) -> String {
    let target_path = Path::new(target);
    // Find common prefix
    let base_comps: Vec<_> = base.components().collect();
    let target_comps: Vec<_> = target_path.components().collect();

    let mut common = 0;
    for (a, b) in base_comps.iter().zip(target_comps.iter()) {
        if a == b {
            common += 1;
        } else {
            break;
        }
    }

    let up = base_comps.len() - common;
    let mut result = String::new();
    for _ in 0..up {
        if !result.is_empty() {
            result.push('/');
        }
        result.push_str("..");
    }
    for comp in &target_comps[common..] {
        if let std::path::Component::Normal(c) = comp {
            if !result.is_empty() {
                result.push('/');
            }
            result.push_str(c.to_str().unwrap_or(""));
        }
    }
    result
}

/// Remove stale fingerprinted files from output_dir that aren't in the current manifest.
/// Only removes files that match the fingerprint pattern (name.{hash}.ext),
/// leaving manually placed files untouched.
pub fn prune_stale(manifest: &AssetManifest, output_dir: &Path) -> anyhow::Result<()> {
    let entries = collect_files(output_dir)?;
    let keep: std::collections::HashSet<String> = manifest
        .mappings
        .values()
        .cloned()
        .chain(manifest.mappings.keys().cloned())
        .collect();

    for entry in &entries {
        let relative = entry.strip_prefix(output_dir).unwrap_or(entry);
        let rel_str = relative.to_string_lossy().to_string();
        // Only clean up files that look like they were produced by fingerprint_public.
        // Files manually placed in output_dir (e.g. CNAME, .htaccess) are left alone.
        if !keep.contains(&rel_str) && looks_like_fingerprinted(&rel_str) && entry.is_file() {
            let _ = std::fs::remove_file(entry);
        }
    }
    Ok(())
}

/// Returns true if the path looks like a fingerprinted file: `name.{12-hex}.ext`
fn looks_like_fingerprinted(path: &str) -> bool {
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    let parts: Vec<&str> = name.rsplitn(2, '.').collect();
    // Need at least stem.hash.ext
    if parts.len() < 2 {
        return false;
    }
    let before_ext = parts[1]; // "logo.a1b2c3d4e5f6" or just "style"
    before_ext.rsplit('.').next().is_some_and(|segment| {
        segment.len() == 12 && segment.chars().all(|c| c.is_ascii_hexdigit())
    })
}

fn collect_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name().is_some_and(|n| n == ".DS_Store") {
            continue;
        }
        if path.is_dir() {
            collect_files_recursive(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("kiln-asset-test-{}-{}", prefix, std::process::id()))
    }

    #[test]
    fn fingerprints_css_files() {
        let dir = test_dir("css");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("public")).unwrap();
        std::fs::write(dir.join("public/style.css"), "body{margin:0}").unwrap();

        let output = dir.join("dist");
        let manifest = fingerprint_public(&dir.join("public"), &output).unwrap();

        let hashed = manifest.resolve("style.css");
        assert!(hashed.contains("style."));
        assert!(hashed.ends_with(".css"));
        assert!(hashed != "style.css");
        assert!(output.join(&hashed).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copies_non_fingerprintable_files_as_is() {
        let dir = test_dir("nonfp");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("public")).unwrap();
        std::fs::write(dir.join("public/data.json"), "{}").unwrap();

        let output = dir.join("dist");
        let manifest = fingerprint_public(&dir.join("public"), &output).unwrap();

        assert_eq!(manifest.resolve("data.json"), "data.json");
        assert!(output.join("data.json").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fingerprints_js_and_images() {
        let dir = test_dir("multi");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("public/js")).unwrap();
        std::fs::write(dir.join("public/js/app.js"), "console.log(1)").unwrap();
        // Create a minimal valid PNG
        let png: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        std::fs::write(dir.join("public/logo.png"), png).unwrap();

        let output = dir.join("dist");
        let manifest = fingerprint_public(&dir.join("public"), &output).unwrap();

        let hashed_js = manifest.resolve("js/app.js");
        assert!(hashed_js.contains("app."));
        assert!(hashed_js.ends_with(".js"));

        let hashed_png = manifest.resolve("logo.png");
        assert!(hashed_png.contains("logo."));
        assert!(hashed_png.ends_with(".png"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn manifest_save_and_load_roundtrip() {
        let dir = test_dir("roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("public")).unwrap();
        std::fs::write(dir.join("public/style.css"), "body{margin:0}").unwrap();

        let output = dir.join("dist");
        let manifest = fingerprint_public(&dir.join("public"), &output).unwrap();
        manifest.save(&output).unwrap();

        let loaded = AssetManifest::load(&output).unwrap();
        assert_eq!(loaded.resolve("style.css"), manifest.resolve("style.css"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handles_empty_public_dir() {
        let dir = test_dir("empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("public")).unwrap();

        let output = dir.join("dist");
        let manifest = fingerprint_public(&dir.join("public"), &output).unwrap();
        assert!(manifest.mappings.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_fingerprintable_matches_extensions() {
        assert!(is_fingerprintable(Path::new("style.css")));
        assert!(is_fingerprintable(Path::new("app.js")));
        assert!(is_fingerprintable(Path::new("img.png")));
        assert!(is_fingerprintable(Path::new("font.woff2")));
        assert!(!is_fingerprintable(Path::new("data.json")));
        assert!(!is_fingerprintable(Path::new("readme.txt")));
        assert!(!is_fingerprintable(Path::new("index.html")));
    }

    // ── CSS url() rewrite ──

    #[test]
    fn rewrites_url_with_fingerprinted_image() {
        let mut manifest = AssetManifest::default();
        manifest
            .mappings
            .insert("images/logo.png".into(), "images/logo.abc123.png".into());

        let css = r#"body { background: url(../images/logo.png); }"#;
        let rewritten = super::rewrite_css_urls(css, "css/style.css", &manifest);
        assert!(rewritten.contains("url(../images/logo.abc123.png)"));
        assert!(!rewritten.contains("logo.png"));
    }

    #[test]
    fn rewrites_quoted_url_with_single_quotes() {
        let mut manifest = AssetManifest::default();
        manifest.mappings.insert(
            "fonts/roboto.woff2".into(),
            "fonts/roboto.def456.woff2".into(),
        );

        let css = "@font-face { src: url('../fonts/roboto.woff2'); }";
        let rewritten = super::rewrite_css_urls(css, "css/style.css", &manifest);
        assert!(rewritten.contains("url('../fonts/roboto.def456.woff2')"));
    }

    #[test]
    fn preserves_data_urls() {
        let manifest = AssetManifest::default();
        let css = "background: url(data:image/svg+xml;base64,PHN2Zy8+);";
        let rewritten = super::rewrite_css_urls(css, "css/style.css", &manifest);
        assert_eq!(rewritten, css);
    }

    #[test]
    fn preserves_http_urls() {
        let manifest = AssetManifest::default();
        let css = "background: url(https://cdn.example.com/bg.jpg);";
        let rewritten = super::rewrite_css_urls(css, "css/style.css", &manifest);
        assert_eq!(rewritten, css);
    }

    #[test]
    fn rewrites_same_directory_url() {
        let mut manifest = AssetManifest::default();
        manifest
            .mappings
            .insert("css/reset.css".into(), "css/reset.xyz789.css".into());

        let css = r#"@import url("reset.css");"#;
        let rewritten = super::rewrite_css_urls(css, "css/style.css", &manifest);
        assert!(rewritten.contains(r#"url("reset.xyz789.css")"#));
    }

    #[test]
    fn ignores_unmatched_url_without_manifest_entry() {
        let manifest = AssetManifest::default();
        let css = "background: url(nonexistent.png);";
        let rewritten = super::rewrite_css_urls(css, "css/style.css", &manifest);
        assert_eq!(rewritten, css);
    }

    #[test]
    fn rewrites_multiple_urls_in_one_css() {
        let mut manifest = AssetManifest::default();
        manifest
            .mappings
            .insert("images/a.png".into(), "images/a.h1.png".into());
        manifest
            .mappings
            .insert("images/b.png".into(), "images/b.h2.png".into());

        let css = "url(../images/a.png) url(../images/b.png)";
        let rewritten = super::rewrite_css_urls(css, "css/style.css", &manifest);
        assert!(rewritten.contains("a.h1.png"));
        assert!(rewritten.contains("b.h2.png"));
        assert!(!rewritten.contains("a.png"));
    }

    #[test]
    fn css_url_rewrite_is_integrated_in_fingerprint_public() {
        let dir = test_dir("css-rewrite");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("public/css")).unwrap();
        std::fs::create_dir_all(dir.join("public/images")).unwrap();

        // CSS references the image
        std::fs::write(
            dir.join("public/css/style.css"),
            "body { background: url(../images/logo.png); }\n",
        )
        .unwrap();
        std::fs::write(dir.join("public/images/logo.png"), b"PNGDATA").unwrap();

        let output = dir.join("dist");
        let manifest = fingerprint_public(&dir.join("public"), &output).unwrap();

        let css_hashed = manifest.resolve("css/style.css");
        let css_content = std::fs::read_to_string(output.join(&css_hashed)).unwrap();
        let img_hashed = manifest.resolve("images/logo.png");

        // The url() should contain the fingerprinted image filename
        let img_filename = std::path::Path::new(&img_hashed)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            css_content.contains(&format!("url(../images/{})", img_filename)),
            "expected CSS to contain ../images/{}, got: {}",
            img_filename,
            css_content
        );
        assert!(!css_content.contains("logo.png"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
