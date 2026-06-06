use std::time::Instant;

#[allow(dead_code)]
pub struct ProfileData {
    cache_hits: usize,
    cache_misses: usize,
    render_count: usize,
    page_timings: Vec<PageTiming>,
    current_page: Option<PageTimingState>,
}

#[allow(dead_code)]
struct PageTimingState {
    url: String,
    template: String,
    start: Instant,
}

pub struct PageTiming {
    pub url: String,
    pub template: String,
    pub elapsed_ms: u128,
}

impl ProfileData {
    fn new() -> Self {
        Self {
            cache_hits: 0,
            cache_misses: 0,
            render_count: 0,
            page_timings: Vec::new(),
            current_page: None,
        }
    }
}

pub struct BuildTimer {
    start: Instant,
    phases: Vec<PhaseTiming>,
    current: Option<(String, Instant)>,
    profile: Option<ProfileData>,
}

struct PhaseTiming {
    name: String,
    elapsed_ms: u128,
}

impl BuildTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            phases: Vec::new(),
            current: None,
            profile: None,
        }
    }

    pub fn with_profile() -> Self {
        Self {
            start: Instant::now(),
            phases: Vec::new(),
            current: None,
            profile: Some(ProfileData::new()),
        }
    }

    pub fn phase(&mut self, name: &str) {
        self.end_current();
        self.current = Some((name.to_string(), Instant::now()));
    }

    pub fn finish(&mut self) {
        self.end_current();
    }

    pub fn total_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    #[allow(dead_code)]
    pub fn start_page(&mut self, url: &str, template: &str) {
        if let Some(p) = &mut self.profile {
            p.current_page = Some(PageTimingState {
                url: url.to_string(),
                template: template.to_string(),
                start: Instant::now(),
            });
        }
    }

    #[allow(dead_code)]
    pub fn end_page(&mut self, rendered: bool) {
        if let Some(p) = &mut self.profile {
            if let Some(state) = p.current_page.take() {
                if rendered {
                    p.render_count += 1;
                    p.page_timings.push(PageTiming {
                        url: state.url,
                        template: state.template,
                        elapsed_ms: state.start.elapsed().as_millis(),
                    });
                }
            }
        }
    }

    pub fn set_cache_stats(&mut self, hits: usize, misses: usize) {
        if let Some(p) = &mut self.profile {
            p.cache_hits = hits;
            p.cache_misses = misses;
        }
    }

    #[allow(dead_code)]
    pub fn is_profiling(&self) -> bool {
        self.profile.is_some()
    }

    fn end_current(&mut self) {
        if let Some((name, start)) = self.current.take() {
            self.phases.push(PhaseTiming {
                name,
                elapsed_ms: start.elapsed().as_millis(),
            });
        }
    }

    pub fn print_report(&self, items: usize, date_ordered: usize, outputs: usize) {
        let total = self.start.elapsed().as_millis();
        eprintln!("\nBuild report:");
        for phase in &self.phases {
            eprintln!("  {:30} {:>5}ms", phase.name, phase.elapsed_ms);
        }
        eprintln!("  {:30} {:>5}ms", "total", total);
        eprintln!(
            "  {} items ({} date-ordered), {} outputs",
            items, date_ordered, outputs
        );
    }

    pub fn print_profile_report(&self) {
        let Some(p) = &self.profile else { return };
        let total = self.start.elapsed().as_millis();

        eprintln!("\nProfile report:");
        eprintln!("  Phase timings:");
        for phase in &self.phases {
            eprintln!("    {:28} {:>5}ms", phase.name, phase.elapsed_ms);
        }

        eprintln!("\n  Cache:");
        let total_cache = p.cache_hits + p.cache_misses;
        if total_cache > 0 {
            let hit_rate = (p.cache_hits as f64 / total_cache as f64) * 100.0;
            eprintln!(
                "    {} hits / {} misses ({:.1}% hit rate, stale entries counted as misses)",
                p.cache_hits, p.cache_misses, hit_rate
            );
        } else {
            eprintln!("    no cache activity");
        }

        eprintln!("\n  Rendering:");
        eprintln!("    {} page renders", p.render_count);
        if !p.page_timings.is_empty() {
            let avg = p.page_timings.iter().map(|t| t.elapsed_ms).sum::<u128>()
                / p.page_timings.len() as u128;
            let max = p
                .page_timings
                .iter()
                .map(|t| t.elapsed_ms)
                .max()
                .unwrap_or(0);
            eprintln!("    avg {}ms, max {}ms per page", avg, max);

            let mut sorted: Vec<&PageTiming> = p.page_timings.iter().collect();
            sorted.sort_by_key(|b| std::cmp::Reverse(b.elapsed_ms));
            eprintln!("    slowest pages:");
            for t in sorted.iter().take(5) {
                eprintln!("      {:>5}ms  {} ({})", t.elapsed_ms, t.url, t.template);
            }
        }

        eprintln!("\n  Total: {}ms", total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_profile_initializes_profile_data() {
        let timer = BuildTimer::with_profile();
        assert!(timer.is_profiling());
    }

    #[test]
    fn new_has_no_profile() {
        let timer = BuildTimer::new();
        assert!(!timer.is_profiling());
    }

    #[test]
    fn set_cache_stats_sets_values() {
        let mut timer = BuildTimer::with_profile();
        timer.set_cache_stats(10, 5);
        let p = timer.profile.as_ref().unwrap();
        assert_eq!(p.cache_hits, 10);
        assert_eq!(p.cache_misses, 5);
    }

    #[test]
    fn page_timing_records() {
        let mut timer = BuildTimer::with_profile();
        timer.start_page("/test/", "post.html");
        timer.end_page(true);
        let p = timer.profile.as_ref().unwrap();
        assert_eq!(p.page_timings.len(), 1);
        assert_eq!(p.page_timings[0].url, "/test/");
        assert_eq!(p.render_count, 1);
    }

    #[test]
    fn profile_counts_multiple_page_renders() {
        let mut timer = BuildTimer::with_profile();
        timer.start_page("/first/", "post.html");
        timer.end_page(true);
        timer.start_page("/second/", "page.html");
        timer.end_page(true);

        let p = timer.profile.as_ref().unwrap();
        assert_eq!(p.render_count, 2);
        assert_eq!(p.page_timings.len(), 2);
        assert_eq!(p.page_timings[1].url, "/second/");
    }

    #[test]
    fn profile_does_not_count_cached_pages_as_renders() {
        let mut timer = BuildTimer::with_profile();

        timer.start_page("/cached/", "post.html");
        timer.end_page(false);

        let p = timer.profile.as_ref().unwrap();
        assert_eq!(p.render_count, 0);
        assert!(p.page_timings.is_empty());
    }

    #[test]
    fn no_op_without_profile() {
        let mut timer = BuildTimer::new();
        timer.start_page("/test/", "post.html");
        timer.end_page(true);
        assert!(!timer.is_profiling());
    }
}
