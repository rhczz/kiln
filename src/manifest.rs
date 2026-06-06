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
    #[serde(default)]
    pub template_deps: Vec<String>,
    #[serde(default)]
    pub template_hash: String,
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

    pub fn record(
        &mut self,
        source: PathBuf,
        outputs: Vec<PathBuf>,
        content_hash: String,
        template_deps: Vec<String>,
        template_hash: String,
    ) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.source == source) {
            existing.outputs = outputs;
            existing.content_hash = content_hash;
            existing.template_deps = template_deps;
            existing.template_hash = template_hash;
        } else {
            self.entries.push(ManifestEntry {
                source,
                outputs,
                content_hash,
                template_deps,
                template_hash,
            });
        }
    }

    /// Returns all entries whose `template_deps` contain the given template.
    pub fn pages_depending_on_template(&self, template: &str) -> Vec<&ManifestEntry> {
        self.entries
            .iter()
            .filter(|e| e.template_deps.iter().any(|d| d == template))
            .collect()
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

    fn empty_deps() -> (Vec<String>, String) {
        (Vec::new(), String::new())
    }

    #[test]
    fn records_and_retrieves_entries() {
        let mut manifest = BuildManifest::default();
        let (deps, hash) = empty_deps();
        manifest.record(
            PathBuf::from("content/posts/a.md"),
            vec![PathBuf::from("posts/a/index.html")],
            "hash_a".into(),
            deps,
            hash,
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
        let (deps, hash) = empty_deps();
        manifest.record(
            PathBuf::from("content/posts/a.md"),
            vec![PathBuf::from("posts/a/index.html")],
            "hash1".into(),
            deps.clone(),
            hash.clone(),
        );
        manifest.record(
            PathBuf::from("content/posts/a.md"),
            vec![PathBuf::from("posts/a/index.html")],
            "hash2".into(),
            deps,
            hash,
        );
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].content_hash, "hash2");
    }

    #[test]
    fn detects_stale_outputs() {
        let mut manifest = BuildManifest::default();
        let (deps, hash) = empty_deps();
        manifest.record(
            PathBuf::from("a.md"),
            vec![PathBuf::from("a/index.html")],
            "h".into(),
            deps.clone(),
            hash.clone(),
        );
        manifest.record(
            PathBuf::from("b.md"),
            vec![PathBuf::from("b/index.html")],
            "h".into(),
            deps,
            hash,
        );

        let stale = manifest.stale_outputs(&[PathBuf::from("a/index.html")]);
        assert_eq!(stale, vec![PathBuf::from("b/index.html")]);
    }

    #[test]
    fn prunes_removed_sources() {
        let mut manifest = BuildManifest::default();
        let (deps, hash) = empty_deps();
        manifest.record(
            PathBuf::from("a.md"),
            vec![],
            "h".into(),
            deps.clone(),
            hash.clone(),
        );
        manifest.record(PathBuf::from("b.md"), vec![], "h".into(), deps, hash);

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
        let (deps, hash) = empty_deps();
        manifest.record(
            PathBuf::from("a.md"),
            vec![PathBuf::from("a/index.html")],
            "h".into(),
            deps,
            hash,
        );

        manifest.save(&dir).unwrap();
        let loaded = BuildManifest::load(&dir).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.config_hash, "abc");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn queries_pages_by_template_dep() {
        let mut manifest = BuildManifest::default();
        let (empty, empty_hash) = empty_deps();
        manifest.record(
            PathBuf::from("a.md"),
            vec![PathBuf::from("a/index.html")],
            "h1".into(),
            vec!["post.html".into(), "layout.html".into()],
            "th1".into(),
        );
        manifest.record(
            PathBuf::from("b.md"),
            vec![PathBuf::from("b/index.html")],
            "h2".into(),
            vec!["page.html".into(), "layout.html".into()],
            "th2".into(),
        );
        manifest.record(
            PathBuf::from("c.md"),
            vec![],
            "h3".into(),
            empty,
            empty_hash,
        );

        let deps = manifest.pages_depending_on_template("layout.html");
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|e| e.source == PathBuf::from("a.md")));
        assert!(deps.iter().any(|e| e.source == PathBuf::from("b.md")));

        let deps = manifest.pages_depending_on_template("post.html");
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].source, PathBuf::from("a.md"));

        let deps = manifest.pages_depending_on_template("nonexistent.html");
        assert!(deps.is_empty());
    }

    #[test]
    fn backward_compat_missing_template_fields() {
        // Old manifest JSON without template_deps/template_hash should load
        let json = r#"{"entries":[{"source":"a.md","outputs":["a/index.html"],"content_hash":"h"}],"config_hash":""}"#;
        let manifest: BuildManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.entries.len(), 1);
        assert!(manifest.entries[0].template_deps.is_empty());
        assert!(manifest.entries[0].template_hash.is_empty());
    }
}
