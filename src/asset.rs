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
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn resolve(&self, original: &str) -> String {
        self.mappings
            .get(original)
            .cloned()
            .unwrap_or_else(|| original.to_string())
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
            let join_with_parent = |base: &str| {
                match relative.parent().filter(|p| !p.as_os_str().is_empty()) {
                    Some(parent) => format!("{}/{}", parent.to_string_lossy(), base),
                    None => base.to_string(),
                }
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
            manifest
                .mappings
                .insert(rel_str.clone(), rel_str);
        }
    }

    Ok(manifest)
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
    before_ext
        .rsplit('.')
        .next()
        .is_some_and(|segment| segment.len() == 12 && segment.chars().all(|c| c.is_ascii_hexdigit()))
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
        std::env::temp_dir().join(format!(
            "kiln-asset-test-{}-{}",
            prefix,
            std::process::id()
        ))
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
}
