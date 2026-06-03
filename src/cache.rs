use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::CollectionConfig;
use crate::content::{self, ContentItem};

#[derive(Default)]
pub struct BuildCache {
    content_items: HashMap<PathBuf, CachedContent>,
    rendered_pages: HashMap<PathBuf, CachedRender>,
    copied_public: HashMap<PathBuf, String>,
    page_outputs: HashSet<PathBuf>,
    public_outputs: HashSet<PathBuf>,
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
        self.copied_public.clear();
        self.page_outputs.clear();
        self.public_outputs.clear();
    }

    pub fn parse_content_item(
        &mut self,
        path: &Path,
        raw: &str,
        collection: &CollectionConfig,
        include_drafts: bool,
    ) -> anyhow::Result<Option<ContentItem>> {
        let hash = content::fingerprint(raw.as_bytes());
        if let Some(cached) = self.content_items.get(path) {
            if cached.hash == hash {
                if !include_drafts && cached.item.draft {
                    return Ok(None);
                }
                return Ok(Some(cached.item.clone()));
            }
        }

        let parsed = content::parse_content_item(path, raw, collection, include_drafts)?;
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

    pub fn cached_render(&self, source_path: &Path, hash: &str) -> Option<&str> {
        self.rendered_pages.get(source_path).and_then(|entry| {
            if entry.hash == hash {
                Some(entry.html.as_str())
            } else {
                None
            }
        })
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
}
