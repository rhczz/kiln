use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{CollectionConfig, SiteConfig};
use crate::content::ContentItem;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    Home,
    Single,
    Section,
    TaxonomyIndex,
    Term,
    Paginate,
    NotFound,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub kind: PageKind,
    pub url: String,
    pub output_path: PathBuf,
    pub template: String,
    pub title: String,
    pub description: String,
    pub source_path: Option<PathBuf>,
    pub content_item: Option<ContentItem>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BreadcrumbItem {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub slug: String,
    pub title: String,
    pub url: String,
    pub collection: String,
    pub parent_slug: Option<String>,
    pub children_slugs: Vec<String>,
    pub weight: i32,
    pub breadcrumb: Vec<BreadcrumbItem>,
}

#[derive(Debug, Clone)]
pub struct Taxonomy {
    pub name: String,
    pub slug: String,
    pub template: String,
    pub terms: Vec<Term>,
}

#[derive(Debug, Clone)]
pub struct Term {
    pub name: String,
    pub slug: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct SiteModel {
    pub pages: Vec<Page>,
    pub sections: HashMap<String, Section>,
    pub collections: HashMap<String, Vec<ContentItem>>,
    pub taxonomies: HashMap<String, Taxonomy>,
    pub all_items: Vec<ContentItem>,
}

pub fn build_site_model(
    all_items: Vec<ContentItem>,
    collections: &[CollectionConfig],
    config: &SiteConfig,
) -> SiteModel {
    let mut pages = Vec::new();
    let mut collection_map: HashMap<String, Vec<ContentItem>> = HashMap::new();

    for item in &all_items {
        let col = collections
            .iter()
            .find(|c| c.name == item.collection)
            .map(|c| c.template.clone())
            .unwrap_or_else(|| format!("{}.html", item.collection));

        let output_path = page_output_path(&item.url);
        pages.push(Page {
            kind: PageKind::Single,
            url: item.url.clone(),
            output_path,
            template: col,
            title: item.title.clone(),
            description: item.description.clone(),
            source_path: Some(item.source_path.clone()),
            content_item: Some(item.clone()),
        });
    }

    // Homepage
    pages.push(Page {
        kind: PageKind::Home,
        url: "/".into(),
        output_path: PathBuf::from("index.html"),
        template: "home.html".into(),
        title: String::new(),
        description: config.site.description.clone(),
        source_path: None,
        content_item: None,
    });

    // 404
    pages.push(Page {
        kind: PageKind::NotFound,
        url: "/404.html".into(),
        output_path: PathBuf::from("404.html"),
        template: "404.html".into(),
        title: "Page Not Found".into(),
        description: String::new(),
        source_path: None,
        content_item: None,
    });

    for item in all_items.iter() {
        collection_map
            .entry(item.collection.clone())
            .or_default()
            .push(item.clone());
    }

    // Sections
    let sections = build_sections(&all_items, collections, &config.paths.content);
    let mut section_list: Vec<&Section> = sections.values().collect();
    section_list.sort_by_key(|a| section_sort_key(a));

    // Add section pages
    for section in &section_list {
        pages.push(Page {
            kind: PageKind::Section,
            url: section.url.clone(),
            output_path: page_output_path(&section.url),
            template: "section.html".into(),
            title: section.title.clone(),
            description: String::new(),
            source_path: None,
            content_item: None,
        });
    }

    // Taxonomies
    let taxonomy_configs = if config.taxonomies.is_empty() {
        vec![crate::config::TaxonomyConfig {
            name: "tags".into(),
            slug: "tags".into(),
            template: "term.html".into(),
        }]
    } else {
        config.taxonomies.clone()
    };

    let taxonomies = build_taxonomies(&all_items, &taxonomy_configs);
    let mut taxonomy_list: Vec<&Taxonomy> = taxonomies.values().collect();
    taxonomy_list.sort_by_key(|a| taxonomy_sort_key(a));

    // Add taxonomy index and term pages
    for tax in &taxonomy_list {
        pages.push(Page {
            kind: PageKind::TaxonomyIndex,
            url: format!("/{}/", tax.slug),
            output_path: page_output_path(&format!("/{}/", tax.slug)),
            template: "taxonomy.html".into(),
            title: tax.name.clone(),
            description: String::new(),
            source_path: None,
            content_item: None,
        });
        for term in &tax.terms {
            pages.push(Page {
                kind: PageKind::Term,
                url: term.url.clone(),
                output_path: page_output_path(&term.url),
                template: tax.template.clone(),
                title: term.name.clone(),
                description: String::new(),
                source_path: None,
                content_item: None,
            });
        }
    }

    // Pagination: Home, Section, and Term pages
    if config.paginate_by > 0 {
        let date_ordered = all_items.iter().filter(|i| i.year.is_some()).count();
        push_paginate_pages(
            &mut pages,
            date_ordered,
            "/",
            "home.html",
            &|n| format!("Page {}", n),
            &config.site.description,
            config,
        );

        for section in &section_list {
            let section_count = all_items
                .iter()
                .filter(|item| url_is_under_section(&item.url, &section.url))
                .count();
            push_paginate_pages(
                &mut pages,
                section_count,
                &section.url,
                "section.html",
                &|n| format!("{} — Page {}", section.title, n),
                "",
                config,
            );
        }

        for taxonomy in &taxonomy_list {
            for term in &taxonomy.terms {
                let term_count = all_items
                    .iter()
                    .filter(|item| {
                        item_taxonomy_terms(item, &taxonomy.name)
                            .iter()
                            .any(|tag| crate::content::slugify(tag) == term.slug)
                    })
                    .count();
                push_paginate_pages(
                    &mut pages,
                    term_count,
                    &term.url,
                    "term.html",
                    &|n| format!("{} — Page {}", term.name, n),
                    "",
                    config,
                );
            }
        }
    }

    SiteModel {
        pages,
        sections,
        collections: collection_map,
        taxonomies,
        all_items,
    }
}

fn build_sections(
    _all_items: &[ContentItem],
    collections: &[CollectionConfig],
    content_dir: &str,
) -> HashMap<String, Section> {
    let mut sections: HashMap<String, Section> = HashMap::new();

    for col in collections {
        let col_path = Path::new(content_dir).join(&col.directory);
        if !col_path.is_dir() {
            continue;
        }

        discover_sections_recursive(
            &col_path,
            &col_path,
            &mut sections,
            None,
            &col.route,
            &col.name,
        );
    }

    sections
}

fn discover_sections_recursive(
    dir: &Path,
    collection_root: &Path,
    sections: &mut HashMap<String, Section>,
    parent_slug: Option<&str>,
    collection_route: &str,
    collection_name: &str,
) {
    let current_section_key = if dir.join("_index.md").exists() {
        let relative = dir.strip_prefix(collection_root).unwrap_or(dir);
        let section_slug = relative.to_string_lossy().replace('\\', "/");
        let section_url = section_url_from_route(collection_route, &section_slug);
        let section_key = format!("{}:{}", collection_route, section_slug);
        let (title, weight) = parse_section_index(&dir.join("_index.md"));
        let breadcrumb = build_breadcrumb(parent_slug, sections);

        sections.insert(
            section_key.clone(),
            Section {
                slug: section_slug.clone(),
                title,
                url: section_url,
                collection: collection_name.to_string(),
                parent_slug: parent_slug.map(|s| s.to_string()),
                children_slugs: Vec::new(),
                weight,
                breadcrumb,
            },
        );

        if let Some(parent_key) = parent_slug {
            if let Some(parent) = sections.get_mut(parent_key) {
                parent.children_slugs.push(section_key.clone());
            }
        }

        Some(section_key)
    } else {
        None
    };

    let next_parent = current_section_key.as_deref().or(parent_slug);

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if dir_name.starts_with('_') || dir_name.starts_with('.') {
            continue;
        }

        discover_sections_recursive(
            &path,
            collection_root,
            sections,
            next_parent,
            collection_route,
            collection_name,
        );
    }
}

fn parse_section_index(path: &Path) -> (String, i32) {
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    let (fm_str, _body) = split_frontmatter(&raw);
    let mut title = String::new();
    let mut weight = 0;

    for line in fm_str.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("title:") {
            title = val.trim().trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(val) = trimmed.strip_prefix("weight:") {
            weight = val.trim().parse().unwrap_or(0);
        }
    }

    if title.is_empty() {
        title = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    (title, weight)
}

fn section_url_from_route(route: &str, section_slug: &str) -> String {
    let mut url = route.replace("{slug}", section_slug);
    while url.contains("//") {
        url = url.replace("//", "/");
    }
    if !url.starts_with('/') {
        url.insert(0, '/');
    }
    if !url.ends_with('/') {
        url.push('/');
    }
    url
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

    let start = first_line.len();
    let mut cursor = start;
    for line in raw[start..].split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            let frontmatter = &raw[start..cursor];
            let body = &raw[cursor + line.len()..];
            return (frontmatter, body);
        }
        cursor += line.len();
    }

    ("", raw)
}

fn build_breadcrumb(
    parent_slug: Option<&str>,
    sections: &HashMap<String, Section>,
) -> Vec<BreadcrumbItem> {
    let mut breadcrumb = Vec::new();
    let mut current = parent_slug;
    while let Some(slug) = current {
        if let Some(section) = sections.get(slug) {
            breadcrumb.push(BreadcrumbItem {
                title: section.title.clone(),
                url: section.url.clone(),
            });
            current = section.parent_slug.as_deref();
        } else {
            break;
        }
    }
    breadcrumb.reverse();
    breadcrumb
}

fn section_sort_key(section: &Section) -> (i32, String, String) {
    (section.weight, section.title.clone(), section.url.clone())
}

fn taxonomy_sort_key(taxonomy: &Taxonomy) -> (String, String) {
    (taxonomy.slug.clone(), taxonomy.name.clone())
}

pub fn url_is_under_section(item_url: &str, section_url: &str) -> bool {
    let prefix = section_url.trim_matches('/');
    if prefix.is_empty() {
        return false;
    }

    let item = item_url.trim_start_matches('/');
    item == prefix || item.starts_with(&format!("{}/", prefix))
}

fn build_taxonomies(
    all_items: &[ContentItem],
    configs: &[crate::config::TaxonomyConfig],
) -> HashMap<String, Taxonomy> {
    let mut taxonomies: HashMap<String, Taxonomy> = HashMap::new();

    for config in configs {
        let slug = config.effective_slug().to_string();
        let mut term_map: std::collections::BTreeMap<String, Vec<&ContentItem>> =
            std::collections::BTreeMap::new();

        for item in all_items {
            let terms = item_taxonomy_terms(item, &config.name);
            for term in terms {
                term_map.entry(term.to_string()).or_default().push(item);
            }
        }

        let terms = term_map
            .into_keys()
            .map(|name| {
                let term_slug = crate::content::slugify(&name);
                Term {
                    name,
                    slug: term_slug.clone(),
                    url: format!("/{}/{}/", slug, term_slug),
                }
            })
            .collect();

        taxonomies.insert(
            config.name.clone(),
            Taxonomy {
                name: config.name.clone(),
                slug: slug.clone(),
                template: if config.template.is_empty() {
                    "term.html".into()
                } else {
                    config.template.clone()
                },
                terms,
            },
        );
    }

    taxonomies
}

fn item_taxonomy_terms<'a>(item: &'a ContentItem, taxonomy_name: &str) -> Vec<&'a str> {
    item.taxonomy_terms
        .get(taxonomy_name)
        .map(|terms| terms.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default()
}

fn page_output_path(url: &str) -> PathBuf {
    let trimmed = url.trim_start_matches('/').trim_end_matches('/');
    if trimmed.is_empty() {
        PathBuf::from("index.html")
    } else {
        PathBuf::from(trimmed).join("index.html")
    }
}

fn push_paginate_pages(
    pages: &mut Vec<Page>,
    item_count: usize,
    base_url: &str,
    template: &str,
    title_for_page: &dyn Fn(usize) -> String,
    description: &str,
    config: &SiteConfig,
) {
    if item_count <= config.paginate_by {
        return;
    }
    let paginators = crate::paginator::paginate(
        item_count,
        config.paginate_by,
        base_url,
        &config.paginate_path,
    );
    for p in &paginators {
        if p.current_index == 1 {
            continue;
        }
        pages.push(Page {
            kind: PageKind::Paginate,
            url: p.current_url.clone(),
            output_path: page_output_path(&p.current_url),
            template: template.into(),
            title: title_for_page(p.current_index),
            description: description.into(),
            source_path: None,
            content_item: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CollectionConfig, PathsConfig, SiteMeta};
    use crate::content::ContentItem;
    use std::path::PathBuf;

    fn test_config() -> SiteConfig {
        SiteConfig {
            paths: PathsConfig::default(),
            site: SiteMeta {
                title: "Test".into(),
                subtitle: String::new(),
                description: "Test site".into(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: None,
            feed: Default::default(),
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
            taxonomies: vec![],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
        }
    }

    fn test_item(collection: &str, slug: &str, url: &str) -> ContentItem {
        ContentItem {
            source_path: PathBuf::from(format!("content/{collection}/{slug}.md")),
            content_hash: "abc123".into(),
            title: format!("Post {slug}"),
            slug: slug.into(),
            description: String::new(),
            body_html: String::new(),
            collection: collection.into(),
            url: url.into(),
            date: None,
            iso_date: None,
            short_date: None,
            long_date: None,
            year: None,
            featured: false,
            draft: false,
            tags: vec![],
            taxonomy_terms: HashMap::new(),
            raw_date: None,
            headings: vec![],
            shortcodes: vec![],
        }
    }

    #[test]
    fn builds_single_pages_from_items() {
        let items = vec![
            test_item("posts", "hello", "/posts/hello/"),
            test_item("pages", "about", "/about/"),
        ];
        let collections = vec![
            CollectionConfig {
                name: "posts".into(),
                directory: "posts".into(),
                route: "/posts/{slug}/".into(),
                template: "post.html".into(),
                date_ordered: true,
                feed: true,
            },
            CollectionConfig {
                name: "pages".into(),
                directory: "pages".into(),
                route: "/{slug}/".into(),
                template: "page.html".into(),
                date_ordered: false,
                feed: false,
            },
        ];
        let config = test_config();

        let model = build_site_model(items, &collections, &config);

        let singles: Vec<_> = model
            .pages
            .iter()
            .filter(|p| p.kind == PageKind::Single)
            .collect();
        assert_eq!(singles.len(), 2);
        assert_eq!(singles[0].template, "post.html");
        assert_eq!(singles[1].template, "page.html");
        assert_eq!(
            singles[0].output_path,
            PathBuf::from("posts/hello/index.html")
        );
    }

    #[test]
    fn includes_homepage() {
        let items = vec![test_item("posts", "hello", "/posts/hello/")];
        let config = test_config();

        let model = build_site_model(items, &[], &config);

        let home = model
            .pages
            .iter()
            .find(|p| p.kind == PageKind::Home)
            .unwrap();
        assert_eq!(home.url, "/");
        assert_eq!(home.output_path, PathBuf::from("index.html"));
        assert_eq!(home.template, "home.html");
    }

    #[test]
    fn groups_items_by_collection() {
        let items = vec![
            test_item("posts", "a", "/posts/a/"),
            test_item("posts", "b", "/posts/b/"),
            test_item("pages", "about", "/about/"),
        ];
        let config = test_config();

        let model = build_site_model(items, &[], &config);

        assert_eq!(model.collections["posts"].len(), 2);
        assert_eq!(model.collections["pages"].len(), 1);
    }

    #[test]
    fn url_is_under_section_respects_path_boundaries() {
        assert!(url_is_under_section("/blog/docs/", "/blog/docs/"));
        assert!(url_is_under_section("/blog/docs/item/", "/blog/docs/"));
        assert!(!url_is_under_section("/blog/docs2/item/", "/blog/docs/"));
        assert!(!url_is_under_section("/blog/docs/", "/blog/docs2/"));
    }

    #[test]
    fn taxonomy_pages_are_sorted_by_slug() {
        let config = SiteConfig {
            paths: PathsConfig::default(),
            site: SiteMeta {
                title: "Test".into(),
                subtitle: String::new(),
                description: "Test site".into(),
                language: "en".into(),
                base_url: "https://example.com".into(),
            },
            author: None,
            feed: Default::default(),
            collections: vec![],
            extra: toml::Value::Table(Default::default()),
            taxonomies: vec![
                crate::config::TaxonomyConfig {
                    name: "topics".into(),
                    slug: "topics".into(),
                    template: "term.html".into(),
                },
                crate::config::TaxonomyConfig {
                    name: "categories".into(),
                    slug: "categories".into(),
                    template: "term.html".into(),
                },
            ],
            paginate_by: 0,
            paginate_path: "page".into(),
            menus: Default::default(),
        };

        let model = build_site_model(vec![], &[], &config);
        let taxonomy_urls: Vec<_> = model
            .pages
            .iter()
            .filter(|p| p.kind == PageKind::TaxonomyIndex)
            .map(|p| p.url.clone())
            .collect();

        assert_eq!(
            taxonomy_urls,
            vec!["/categories/".to_string(), "/topics/".to_string()]
        );
    }
}
