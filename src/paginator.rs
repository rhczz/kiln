use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Paginator {
    pub current_index: usize,
    pub total_pages: usize,
    pub total_items: usize,
    pub per_page: usize,
    pub first_url: String,
    pub current_url: String,
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
    pub pages: Vec<PagerPage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PagerPage {
    pub index: usize,
    pub url: String,
    pub is_current: bool,
}

pub fn paginate(
    total_items: usize,
    per_page: usize,
    base_url: &str,
    paginate_path: &str,
) -> Vec<Paginator> {
    if per_page == 0 || total_items == 0 {
        return Vec::new();
    }

    let total_pages = total_items.div_ceil(per_page);
    let base = base_url.trim_end_matches('/');

    (1..=total_pages)
        .map(|i| {
            let url = if i == 1 {
                format!("{}/", base)
            } else {
                format!("{}/{}/{}/", base, paginate_path, i)
            };

            let prev_url = if i > 1 && i - 1 == 1 {
                Some(format!("{}/", base))
            } else if i > 1 {
                Some(format!("{}/{}/{}/", base, paginate_path, i - 1))
            } else {
                None
            };

            let next_url = if i < total_pages {
                Some(format!("{}/{}/{}/", base, paginate_path, i + 1))
            } else {
                None
            };

            let pages: Vec<PagerPage> = (1..=total_pages)
                .map(|p| {
                    let p_url = if p == 1 {
                        format!("{}/", base)
                    } else {
                        format!("{}/{}/{}/", base, paginate_path, p)
                    };
                    PagerPage {
                        index: p,
                        url: p_url,
                        is_current: p == i,
                    }
                })
                .collect();

            Paginator {
                current_index: i,
                total_pages,
                total_items,
                per_page,
                first_url: format!("{}/", base),
                current_url: url,
                prev_url,
                next_url,
                pages,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_correct_page_count() {
        let paginators = paginate(25, 10, "/posts", "page");
        assert_eq!(paginators.len(), 3);
        assert_eq!(paginators[0].current_index, 1);
        assert_eq!(paginators[1].current_index, 2);
        assert_eq!(paginators[2].current_index, 3);
    }

    #[test]
    fn generates_correct_urls() {
        let paginators = paginate(25, 10, "/posts", "page");
        assert_eq!(paginators[0].current_url, "/posts/");
        assert_eq!(paginators[1].current_url, "/posts/page/2/");
        assert_eq!(paginators[2].current_url, "/posts/page/3/");
    }

    #[test]
    fn links_prev_and_next() {
        let paginators = paginate(25, 10, "/posts", "page");
        assert!(paginators[0].prev_url.is_none());
        assert_eq!(paginators[0].next_url, Some("/posts/page/2/".into()));
        assert_eq!(paginators[1].prev_url, Some("/posts/".into()));
        assert_eq!(paginators[1].next_url, Some("/posts/page/3/".into()));
        assert_eq!(paginators[2].prev_url, Some("/posts/page/2/".into()));
        assert!(paginators[2].next_url.is_none());
    }

    #[test]
    fn returns_empty_for_zero_per_page() {
        let paginators = paginate(100, 0, "/posts", "page");
        assert!(paginators.is_empty());
    }

    #[test]
    fn handles_exact_fit() {
        let paginators = paginate(20, 10, "/posts", "page");
        assert_eq!(paginators.len(), 2);
    }

    #[test]
    fn single_page() {
        let paginators = paginate(5, 10, "/posts", "page");
        assert_eq!(paginators.len(), 1);
        assert!(paginators[0].prev_url.is_none());
        assert!(paginators[0].next_url.is_none());
    }
}
