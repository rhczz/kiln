use anyhow::Context;
use serde::Deserialize;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::CollectionConfig;
use crate::render;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContentItem {
    #[serde(skip)]
    pub source_path: PathBuf,
    #[serde(skip)]
    pub content_hash: String,
    pub title: String,
    pub slug: String,
    pub description: String,
    pub body_html: String,
    pub collection: String,
    pub url: String,

    pub date: Option<String>,
    pub iso_date: Option<String>,
    pub short_date: Option<String>,
    pub long_date: Option<String>,
    pub year: Option<String>,
    pub featured: bool,
    pub draft: bool,
    pub tags: Vec<String>,
    pub taxonomy_terms: HashMap<String, Vec<String>>,
    #[serde(skip)]
    pub raw_date: Option<chrono::NaiveDate>,
    pub headings: Vec<crate::render::Heading>,
    #[serde(skip)]
    pub shortcodes: Vec<crate::shortcode::Shortcode>,
}

#[derive(Debug, Deserialize)]
struct ItemFrontmatter {
    title: String,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    featured: bool,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    slug: String,
    #[serde(flatten)]
    extra: HashMap<String, serde_yaml::Value>,
}

pub fn load_collection(
    content_dir: &str,
    collection: &CollectionConfig,
    include_drafts: bool,
) -> anyhow::Result<Vec<ContentItem>> {
    let items = load_collection_entries(content_dir, collection, include_drafts, None)?;
    Ok(sort_collection_items(items, collection.date_ordered))
}

pub fn load_collection_cached(
    content_dir: &str,
    collection: &CollectionConfig,
    include_drafts: bool,
    cache: &mut crate::cache::BuildCache,
) -> anyhow::Result<Vec<ContentItem>> {
    let items = load_collection_entries(content_dir, collection, include_drafts, Some(cache))?;
    Ok(sort_collection_items(items, collection.date_ordered))
}

fn sort_collection_items(mut items: Vec<ContentItem>, date_ordered: bool) -> Vec<ContentItem> {
    if date_ordered {
        items.sort_by_key(|item| Reverse(item.raw_date));
    }
    items
}

fn load_collection_entries(
    content_dir: &str,
    collection: &CollectionConfig,
    include_drafts: bool,
    mut cache: Option<&mut crate::cache::BuildCache>,
) -> anyhow::Result<Vec<ContentItem>> {
    let collection_path = std::path::Path::new(content_dir).join(&collection.directory);
    let pattern = format!("{}/**/*.md", collection_path.to_string_lossy());
    let mut items: Vec<ContentItem> = Vec::new();

    for entry in glob::glob(&pattern)? {
        let path = entry?;

        // Skip _index.md (section index files)
        if path.file_name().map(|n| n == "_index.md").unwrap_or(false) {
            continue;
        }

        let raw = std::fs::read_to_string(&path)?;

        let item = if let Some(cache) = cache.as_deref_mut() {
            cache.parse_content_item(&path, &raw, collection, include_drafts, &collection_path)?
        } else {
            parse_content_item(&path, &raw, collection, include_drafts, &collection_path)?
        };

        if let Some(item) = item {
            items.push(item);
        }
    }

    Ok(items)
}

pub(crate) fn parse_content_item(
    path: &Path,
    raw: &str,
    collection: &CollectionConfig,
    include_drafts: bool,
    collection_dir: &Path,
) -> anyhow::Result<Option<ContentItem>> {
    let fingerprint = fingerprint(raw.as_bytes());
    let (fm_str, body) = split_frontmatter(raw);
    let fm: ItemFrontmatter = serde_yaml::from_str(fm_str).map_err(|e| {
        if let Some(location) = e.location() {
            anyhow::anyhow!(
                "Failed to parse frontmatter in {:?} at line {}: {}",
                path,
                location.line() + 1,
                e
            )
        } else {
            anyhow::anyhow!("Failed to parse frontmatter in {:?}: {}", path, e)
        }
    })?;

    if fm.draft && !include_drafts {
        return Ok(None);
    }

    let taxonomy_terms = extract_taxonomy_terms(&fm);
    let slug = if fm.slug.is_empty() {
        derive_slug(path, collection_dir)
    } else {
        validate_slug(&fm.slug)
            .with_context(|| format!("invalid slug '{}' in {:?}", fm.slug, path))?
    };

    let url = collection.route.replace("{slug}", &slug);
    let (processed_body, shortcodes) = crate::shortcode::preprocess(body);
    let rendered = render::markdown_to_html(&processed_body);

    let (date, iso_date, short_date, long_date, year, raw_date) = if collection.date_ordered {
        let date_str = fm.date.unwrap_or_default();
        if date_str.is_empty() {
            anyhow::bail!("Missing required 'date' field in frontmatter in {:?}", path);
        }
        let parsed = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| anyhow::anyhow!("Invalid date '{}' in {:?}: {}", date_str, path, e))?;
        (
            Some(date_str.clone()),
            Some(date_str),
            Some(format_short_date(parsed)),
            Some(format_long_date(parsed)),
            Some(parsed.format("%Y").to_string()),
            Some(parsed),
        )
    } else {
        (None, None, None, None, None, None)
    };

    let description = if fm.description.is_empty() {
        extract_description(body)
    } else {
        fm.description.clone()
    };

    Ok(Some(ContentItem {
        source_path: path.to_path_buf(),
        content_hash: fingerprint,
        title: fm.title,
        slug,
        description,
        body_html: rendered.html,
        collection: collection.name.clone(),
        url,
        date,
        iso_date,
        short_date,
        long_date,
        year,
        featured: fm.featured,
        draft: fm.draft,
        tags: fm.tags,
        taxonomy_terms,
        raw_date,
        headings: rendered.headings,
        shortcodes,
    }))
}

pub(crate) fn fingerprint(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest[..6]
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>()
}

fn format_short_date(date: chrono::NaiveDate) -> String {
    date.format("%Y.%m.%d").to_string()
}

fn format_long_date(date: chrono::NaiveDate) -> String {
    date.format("%B %-d, %Y").to_string()
}

fn split_frontmatter(raw: &str) -> (&str, &str) {
    let mut lines = raw.split_inclusive('\n');
    let first_line = match lines.next() {
        Some(line) => line,
        None => return ("", raw),
    };

    if first_line.trim_end_matches(['\r', '\n']) != "---" {
        return ("", raw);
    }

    let frontmatter_start = first_line.len();
    let mut cursor = frontmatter_start;
    for line in raw[frontmatter_start..].split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            let frontmatter = &raw[frontmatter_start..cursor];
            let body = &raw[cursor + line.len()..];
            return (frontmatter, body);
        }
        cursor += line.len();
    }

    ("", raw)
}

fn derive_slug(path: &std::path::Path, collection_dir: &std::path::Path) -> String {
    let relative = path.strip_prefix(collection_dir).unwrap_or(path);
    let parent = relative
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|p| !p.is_empty() && p != ".");
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let stripped = strip_date_prefix(&stem);
    match parent {
        Some(dir) => format!("{}/{}", dir, stripped),
        None => stripped,
    }
}

/// Reject slugs that could escape the output directory via path traversal.
fn validate_slug(s: &str) -> anyhow::Result<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        anyhow::bail!("slug cannot be empty");
    }
    if trimmed.contains("..") {
        anyhow::bail!("slug cannot contain '..'");
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        anyhow::bail!("slug cannot contain path separators");
    }
    Ok(trimmed.to_string())
}

fn extract_taxonomy_terms(fm: &ItemFrontmatter) -> HashMap<String, Vec<String>> {
    let mut taxonomy_terms = HashMap::new();

    if !fm.tags.is_empty() {
        taxonomy_terms.insert("tags".into(), fm.tags.clone());
    }

    for (key, value) in &fm.extra {
        if matches!(
            key.as_str(),
            "title" | "date" | "description" | "featured" | "draft" | "tags" | "slug"
        ) {
            continue;
        }

        if let Some(values) = yaml_value_to_strings(value) {
            taxonomy_terms.insert(key.clone(), values);
        }
    }

    taxonomy_terms
}

fn yaml_value_to_strings(value: &serde_yaml::Value) -> Option<Vec<String>> {
    match value {
        serde_yaml::Value::Sequence(seq) => {
            let values: Vec<String> = seq.iter().filter_map(yaml_scalar_to_string).collect();
            if values.is_empty() {
                None
            } else {
                Some(values)
            }
        }
        _ => yaml_scalar_to_string(value).map(|value| vec![value]),
    }
}

fn yaml_scalar_to_string(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn strip_date_prefix(name: &str) -> String {
    let bytes = name.as_bytes();
    if bytes.len() >= 11
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && bytes[4] == b'-'
        && bytes[5].is_ascii_digit()
        && bytes[6].is_ascii_digit()
        && bytes[7] == b'-'
        && bytes[8].is_ascii_digit()
        && bytes[9].is_ascii_digit()
        && bytes[10] == b'-'
    {
        name[11..].to_string()
    } else {
        name.to_string()
    }
}

fn extract_description(body: &str) -> String {
    let plain: String = body.chars().take(200).collect();
    plain
        .replace(['#', '*', '`', '[', ']', '(', ')', '>', '|'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn slugify(value: &str) -> String {
    value
        .to_lowercase()
        .replace([' ', '_'], "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::load_collection;
    use crate::config::CollectionConfig;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn skips_drafts_unless_included() {
        let root = temp_dir("kiln-content-test");
        let content = root.join("content");
        let pages = content.join("pages");

        std::fs::create_dir_all(&pages).unwrap();
        std::fs::write(
            pages.join("2026-06-03-draft-page.md"),
            r#"---
title: "Draft Page"
draft: true
---
Draft body."#,
        )
        .unwrap();

        let collection = CollectionConfig {
            name: "pages".into(),
            directory: "pages".into(),
            route: "/{slug}/".into(),
            template: "page.html".into(),
            date_ordered: false,
            feed: false,
        };

        let without_drafts =
            load_collection(content.to_str().unwrap(), &collection, false).unwrap();
        assert!(without_drafts.is_empty());

        let with_drafts = load_collection(content.to_str().unwrap(), &collection, true).unwrap();
        assert_eq!(with_drafts.len(), 1);
        assert_eq!(with_drafts[0].title, "Draft Page");

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), now))
    }

    #[test]
    fn rejects_slug_with_dot_dot() {
        assert!(super::validate_slug("../../../etc/passwd").is_err());
        assert!(super::validate_slug("foo..bar").is_err());
    }

    #[test]
    fn rejects_slug_with_path_separators() {
        assert!(super::validate_slug("foo/bar").is_err());
        assert!(super::validate_slug("foo\\bar").is_err());
    }

    #[test]
    fn accepts_valid_slug() {
        assert_eq!(super::validate_slug("hello-world").unwrap(), "hello-world");
        assert_eq!(super::validate_slug("my-post-123").unwrap(), "my-post-123");
    }
}
