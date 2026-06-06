use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildManifest {
    pub entries: Vec<ManifestEntry>,
    pub config_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub source: PathBuf,
    pub outputs: Vec<PathBuf>,
    pub content_hash: String,
}

impl BuildManifest {
    pub fn load(output_dir: &Path) -> anyhow::Result<Self> {
        let path = manifest_path(output_dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let manifest: Self = serde_json::from_str(&content).unwrap_or_default();
        Ok(manifest)
    }

    pub fn save(&self, output_dir: &Path) -> anyhow::Result<()> {
        let path = manifest_path(output_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn record(&mut self, source: PathBuf, outputs: Vec<PathBuf>, content_hash: String) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.source == source) {
            existing.outputs = outputs;
            existing.content_hash = content_hash;
        } else {
            self.entries.push(ManifestEntry {
                source,
                outputs,
                content_hash,
            });
        }
    }

    /// Returns outputs from a previous build that are no longer present in the current build.
    pub fn stale_outputs(&self, current_outputs: &[PathBuf]) -> Vec<PathBuf> {
        let current_set: std::collections::HashSet<_> = current_outputs.iter().cloned().collect();
        let mut stale = Vec::new();
        for entry in &self.entries {
            for output in &entry.outputs {
                if !current_set.contains(output) {
                    stale.push(output.clone());
                }
            }
        }
        stale
    }

    /// Remove entries whose sources are no longer in the current source set.
    pub fn prune_missing_sources(&mut self, current_sources: &HashMap<PathBuf, String>) {
        self.entries
            .retain(|e| current_sources.contains_key(&e.source));
    }
}

fn manifest_path(output_dir: &Path) -> PathBuf {
    output_dir.join(".kiln").join("manifest.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_retrieves_entries() {
        let mut manifest = BuildManifest::default();
        manifest.record(
            PathBuf::from("content/posts/a.md"),
            vec![PathBuf::from("posts/a/index.html")],
            "hash_a".into(),
        );
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(
            manifest.entries[0].source,
            PathBuf::from("content/posts/a.md")
        );
    }

    #[test]
    fn updates_existing_entry() {
        let mut manifest = BuildManifest::default();
        manifest.record(
            PathBuf::from("content/posts/a.md"),
            vec![PathBuf::from("posts/a/index.html")],
            "hash1".into(),
        );
        manifest.record(
            PathBuf::from("content/posts/a.md"),
            vec![PathBuf::from("posts/a/index.html")],
            "hash2".into(),
        );
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].content_hash, "hash2");
    }

    #[test]
    fn detects_stale_outputs() {
        let mut manifest = BuildManifest::default();
        manifest.record(
            PathBuf::from("a.md"),
            vec![PathBuf::from("a/index.html")],
            "h".into(),
        );
        manifest.record(
            PathBuf::from("b.md"),
            vec![PathBuf::from("b/index.html")],
            "h".into(),
        );

        let stale = manifest.stale_outputs(&[PathBuf::from("a/index.html")]);
        assert_eq!(stale, vec![PathBuf::from("b/index.html")]);
    }

    #[test]
    fn prunes_removed_sources() {
        let mut manifest = BuildManifest::default();
        manifest.record(PathBuf::from("a.md"), vec![], "h".into());
        manifest.record(PathBuf::from("b.md"), vec![], "h".into());

        let current: HashMap<PathBuf, String> =
            [(PathBuf::from("a.md"), "h".into())].into_iter().collect();
        manifest.prune_missing_sources(&current);
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].source, PathBuf::from("a.md"));
    }

    #[test]
    fn saves_and_loads_roundtrip() {
        let dir = std::env::temp_dir().join(format!("kiln-manifest-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut manifest = BuildManifest {
            config_hash: "abc".into(),
            ..Default::default()
        };
        manifest.record(
            PathBuf::from("a.md"),
            vec![PathBuf::from("a/index.html")],
            "h".into(),
        );

        manifest.save(&dir).unwrap();
        let loaded = BuildManifest::load(&dir).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.config_hash, "abc");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
