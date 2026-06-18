use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::CollectionConfig;
use crate::content::{self, ContentItem};

#[derive(Default)]
pub struct BuildCache {
    content_items: HashMap<PathBuf, CachedContent>,
    rendered_pages: HashMap<PathBuf, CachedRender>,
    rendered_generic: HashMap<String, CachedRender>,
    copied_public: HashMap<PathBuf, String>,
    page_outputs: HashSet<PathBuf>,
    public_outputs: HashSet<PathBuf>,
    cache_hits: Cell<usize>,
    cache_misses: Cell<usize>,
}

#[derive(Clone)]
struct CachedContent {
    hash: String,
    item: ContentItem,
}

#[derive(Clone)]
struct CachedRender {
    hash: String,
    html: String,
}

pub struct BuildCacheSnapshot {
    content_items: HashMap<PathBuf, CachedContent>,
    rendered_pages: HashMap<PathBuf, CachedRender>,
    rendered_generic: HashMap<String, CachedRender>,
    copied_public: HashMap<PathBuf, String>,
    page_outputs: HashSet<PathBuf>,
    public_outputs: HashSet<PathBuf>,
}

impl BuildCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_renders(&mut self) {
        self.rendered_pages.clear();
        self.rendered_generic.clear();
        self.copied_public.clear();
        self.page_outputs.clear();
        self.public_outputs.clear();
    }

    /// Clear only render entries whose template_deps include the given template.
    pub fn invalidate_by_template(
        &mut self,
        template: &str,
        page_template_deps: &HashMap<PathBuf, Vec<String>>,
    ) {
        self.rendered_pages.retain(|source, _| {
            !page_template_deps
                .get(source)
                .is_some_and(|deps| deps.iter().any(|d| d == template))
        });
        // For generic pages, we don't have per-entry template deps tracking yet,
        // so invalidate all generic cached renders on template change.
        // This is conservative but correct.
        self.rendered_generic.clear();
    }

    pub fn reset_stats(&self) {
        self.cache_hits.set(0);
        self.cache_misses.set(0);
    }

    pub fn parse_content_item(
        &mut self,
        path: &Path,
        raw: &str,
        collection: &CollectionConfig,
        include_drafts: bool,
        collection_dir: &Path,
    ) -> anyhow::Result<Option<ContentItem>> {
        let hash = content::fingerprint(raw.as_bytes());
        if let Some(cached) = self.content_items.get(path) {
            if cached.hash == hash {
                self.cache_hits.set(self.cache_hits.get() + 1);
                if !include_drafts && cached.item.draft {
                    return Ok(None);
                }
                return Ok(Some(cached.item.clone()));
            }
        }
        self.cache_misses.set(self.cache_misses.get() + 1);

        let parsed =
            content::parse_content_item(path, raw, collection, include_drafts, collection_dir)?;
        if let Some(item) = parsed.as_ref() {
            self.content_items.insert(
                path.to_path_buf(),
                CachedContent {
                    hash,
                    item: item.clone(),
                },
            );
        } else {
            self.content_items.remove(path);
        }
        Ok(parsed)
    }

    /// Returns cached HTML if the render hash matches.
    /// Counts stale entries (key exists, hash differs) as misses.
    pub fn cached_render(&self, source_path: &Path, hash: &str) -> Option<&str> {
        match self.rendered_pages.get(source_path) {
            Some(entry) if entry.hash == hash => {
                self.cache_hits.set(self.cache_hits.get() + 1);
                Some(entry.html.as_str())
            }
            Some(_) => {
                // Entry exists but hash is stale, so count it as a miss.
                self.cache_misses.set(self.cache_misses.get() + 1);
                None
            }
            None => {
                self.cache_misses.set(self.cache_misses.get() + 1);
                None
            }
        }
    }

    pub fn store_render(&mut self, source_path: &Path, hash: String, html: String) {
        self.rendered_pages
            .insert(source_path.to_path_buf(), CachedRender { hash, html });
    }

    pub fn page_outputs(&self) -> &HashSet<PathBuf> {
        &self.page_outputs
    }

    pub fn replace_page_outputs(&mut self, outputs: HashSet<PathBuf>) {
        self.page_outputs = outputs;
    }

    /// Remap cached output paths (page outputs and public outputs) from
    /// `from_prefix` to `to_prefix`. Used after a staged build completes so
    /// that the cache reflects the real output directory, not the temporary
    /// staging directory.
    pub fn remap_outputs(&mut self, from_prefix: &Path, to_prefix: &Path) {
        let remap = |set: &HashSet<PathBuf>| -> HashSet<PathBuf> {
            set.iter()
                .map(|p| {
                    p.strip_prefix(from_prefix)
                        .map(|rel| to_prefix.join(rel))
                        .unwrap_or_else(|_| p.clone())
                })
                .collect()
        };
        self.page_outputs = remap(&self.page_outputs);
        self.public_outputs = remap(&self.public_outputs);
    }

    pub fn copied_public_hash(&self, path: &Path) -> Option<&str> {
        self.copied_public.get(path).map(|hash| hash.as_str())
    }

    pub fn store_public_hash(&mut self, path: PathBuf, hash: String) {
        self.copied_public.insert(path, hash);
    }

    pub fn public_outputs(&self) -> &HashSet<PathBuf> {
        &self.public_outputs
    }
    pub fn replace_public_outputs(&mut self, outputs: HashSet<PathBuf>) {
        self.public_outputs = outputs;
    }

    pub fn add_public_output(&mut self, output: PathBuf) {
        self.public_outputs.insert(output);
    }

    /// Snapshot cache state so it can be restored if a staged rebuild fails
    /// partway through.
    pub fn snapshot(&self) -> BuildCacheSnapshot {
        BuildCacheSnapshot {
            content_items: self.content_items.clone(),
            rendered_pages: self.rendered_pages.clone(),
            rendered_generic: self.rendered_generic.clone(),
            copied_public: self.copied_public.clone(),
            page_outputs: self.page_outputs.clone(),
            public_outputs: self.public_outputs.clone(),
        }
    }

    /// Restore cache state from a snapshot.
    pub fn restore(&mut self, snapshot: BuildCacheSnapshot) {
        self.content_items = snapshot.content_items;
        self.rendered_pages = snapshot.rendered_pages;
        self.rendered_generic = snapshot.rendered_generic;
        self.copied_public = snapshot.copied_public;
        self.page_outputs = snapshot.page_outputs;
        self.public_outputs = snapshot.public_outputs;
    }

    /// Cached render lookup for non-Single pages, keyed by a logical key
    /// (e.g. "/", "/tags/", "/tags/rust/").
    pub fn cached_generic_render(&self, logical_key: &str, hash: &str) -> Option<&str> {
        match self.rendered_generic.get(logical_key) {
            Some(entry) if entry.hash == hash => {
                self.cache_hits.set(self.cache_hits.get() + 1);
                Some(entry.html.as_str())
            }
            Some(_) => {
                self.cache_misses.set(self.cache_misses.get() + 1);
                None
            }
            None => {
                self.cache_misses.set(self.cache_misses.get() + 1);
                None
            }
        }
    }

    pub fn store_generic_render(&mut self, logical_key: String, hash: String, html: String) {
        self.rendered_generic
            .insert(logical_key, CachedRender { hash, html });
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache_hits.get(), self.cache_misses.get())
    }

    /// Build a 4-level cache key: content:template:config:asset
    pub fn build_render_hash(
        content_hash: &str,
        template_hash: &str,
        config_hash: &str,
        asset_hash: &str,
    ) -> String {
        format!(
            "{}:{}:{}:{}",
            content_hash, template_hash, config_hash, asset_hash
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_with_render(hash: &str, html: &str) -> BuildCache {
        let mut cache = BuildCache::new();
        cache.store_render(
            Path::new("content/posts/demo.md"),
            hash.to_string(),
            html.to_string(),
        );
        cache
    }

    #[test]
    fn cached_render_counts_miss_when_entry_is_missing() {
        let cache = BuildCache::new();

        assert!(cache
            .cached_render(Path::new("content/posts/demo.md"), "abc")
            .is_none());
        assert_eq!(cache.cache_stats(), (0, 1));
    }

    #[test]
    fn cached_render_counts_miss_when_entry_is_stale() {
        let cache = cache_with_render("old", "<p>old</p>");

        assert!(cache
            .cached_render(Path::new("content/posts/demo.md"), "new")
            .is_none());
        assert_eq!(cache.cache_stats(), (0, 1));
    }

    #[test]
    fn cached_render_counts_hit_when_entry_matches() {
        let cache = cache_with_render("same", "<p>same</p>");

        assert_eq!(
            cache.cached_render(Path::new("content/posts/demo.md"), "same"),
            Some("<p>same</p>")
        );
        assert_eq!(cache.cache_stats(), (1, 0));
    }

    #[test]
    fn reset_stats_starts_a_new_reporting_window() {
        let cache = cache_with_render("same", "<p>same</p>");

        assert_eq!(
            cache.cached_render(Path::new("content/posts/demo.md"), "same"),
            Some("<p>same</p>")
        );
        assert_eq!(cache.cache_stats(), (1, 0));

        cache.reset_stats();

        assert!(cache
            .cached_render(Path::new("content/posts/missing.md"), "missing")
            .is_none());
        assert_eq!(cache.cache_stats(), (0, 1));
    }

    #[test]
    fn remap_outputs_rewrites_staging_paths_for_pages_and_public() {
        let mut cache = BuildCache::new();
        let staging = Path::new("/srv/site/.dist.staging");
        let output = Path::new("/srv/site/dist");

        cache.replace_page_outputs(
            [
                staging.join("index.html"),
                staging.join("posts/hello/index.html"),
            ]
            .into_iter()
            .collect(),
        );
        cache.add_public_output(staging.join("assets/styles.abc.css"));

        cache.remap_outputs(staging, output);

        let page_outputs = cache.page_outputs();
        assert!(page_outputs.contains(&output.join("index.html")));
        assert!(page_outputs.contains(&output.join("posts/hello/index.html")));
        assert!(!page_outputs.iter().any(|p| p.starts_with(staging)));

        let public_outputs = cache.public_outputs();
        assert!(public_outputs.contains(&output.join("assets/styles.abc.css")));
        assert!(!public_outputs.iter().any(|p| p.starts_with(staging)));
    }

    #[test]
    fn remap_outputs_leaves_unrelated_paths_unchanged() {
        let mut cache = BuildCache::new();
        let staging = Path::new("/srv/site/.dist.staging");
        let output = Path::new("/srv/site/dist");

        // A path that does NOT share the staging prefix should be left as-is.
        let unrelated = PathBuf::from("/tmp/elsewhere/index.html");
        cache.replace_page_outputs([unrelated.clone()].into_iter().collect());

        cache.remap_outputs(staging, output);

        assert!(cache.page_outputs().contains(&unrelated));
    }

    #[test]
    fn snapshot_restore_roundtrips_render_and_output_state() {
        let mut cache = BuildCache::new();
        cache.store_render(
            Path::new("content/posts/demo.md"),
            "old-hash".to_string(),
            "<p>old</p>".to_string(),
        );
        cache.store_generic_render(
            "/".to_string(),
            "generic-hash".to_string(),
            "<main>home</main>".to_string(),
        );
        cache.store_public_hash(PathBuf::from("public/app.css"), "css-hash".to_string());
        cache.replace_page_outputs([PathBuf::from("/dist/index.html")].into_iter().collect());
        cache.add_public_output(PathBuf::from("/dist/assets/app.css"));

        let snapshot = cache.snapshot();

        cache.clear_renders();
        cache.store_render(
            Path::new("content/posts/demo.md"),
            "new-hash".to_string(),
            "<p>new</p>".to_string(),
        );

        cache.restore(snapshot);

        assert_eq!(
            cache.cached_render(Path::new("content/posts/demo.md"), "old-hash"),
            Some("<p>old</p>")
        );
        assert_eq!(
            cache.cached_generic_render("/", "generic-hash"),
            Some("<main>home</main>")
        );
        assert_eq!(
            cache.copied_public_hash(Path::new("public/app.css")),
            Some("css-hash")
        );
        assert!(cache.page_outputs().contains(Path::new("/dist/index.html")));
        assert!(cache
            .public_outputs()
            .contains(Path::new("/dist/assets/app.css")));
    }
}
