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

struct CachedContent {
    hash: String,
    item: ContentItem,
}

struct CachedRender {
    hash: String,
    html: String,
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
}
