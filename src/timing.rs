use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
pub struct ProfileJson<'a> {
    schema_version: u8,
    total_ms: u128,
    phases: Vec<PhaseTimingJson<'a>>,
    cache: CacheProfileJson,
    parallel: ParallelProfileJson,
    rendering: RenderingProfileJson<'a>,
}

#[derive(Serialize)]
struct PhaseTimingJson<'a> {
    name: &'a str,
    elapsed_ms: u128,
}

#[derive(Serialize)]
struct CacheProfileJson {
    hits: usize,
    misses: usize,
    hit_rate: Option<f64>,
}

#[derive(Serialize)]
struct ParallelProfileJson {
    threads: usize,
    render_wall_time_ms: u128,
    total_cpu_time_ms: u128,
    speedup: Option<f64>,
}

#[derive(Serialize)]
struct RenderingProfileJson<'a> {
    page_renders: usize,
    avg_page_ms: Option<u128>,
    max_page_ms: Option<u128>,
    slowest_pages: Vec<PageTimingJson<'a>>,
}

#[derive(Serialize)]
struct PageTimingJson<'a> {
    url: &'a str,
    template: &'a str,
    elapsed_ms: u128,
}

pub struct ProfileData {
    cache_hits: usize,
    cache_misses: usize,
    render_count: usize,
    page_timings: Vec<PageTiming>,
    current_page: Option<PageTimingState>,
    pub parallel_threads: usize,
    pub render_wall_time_ms: u128,
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
            parallel_threads: 0,
            render_wall_time_ms: 0,
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

    #[allow(dead_code)]
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
        let state = self
            .profile
            .as_mut()
            .and_then(|profile| profile.current_page.take());
        if let Some(state) = state {
            if rendered {
                self.record_page_render(
                    &state.url,
                    &state.template,
                    state.start.elapsed().as_millis(),
                );
            }
        }
    }

    pub fn record_page_render(&mut self, url: &str, template: &str, elapsed_ms: u128) {
        if let Some(p) = &mut self.profile {
            p.render_count += 1;
            p.page_timings.push(PageTiming {
                url: url.to_string(),
                template: template.to_string(),
                elapsed_ms,
            });
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

    pub fn set_parallel_stats(&mut self, threads: usize, wall_time_ms: u128) {
        if let Some(p) = &mut self.profile {
            p.parallel_threads = threads;
            p.render_wall_time_ms = wall_time_ms;
        }
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

        if p.parallel_threads > 0 {
            eprintln!("\n  Parallel rendering:");
            eprintln!("    {} threads", p.parallel_threads);
            eprintln!("    render wall time: {}ms", p.render_wall_time_ms);
            if p.render_wall_time_ms > 0 {
                let cpu_sum: u128 = p.page_timings.iter().map(|t| t.elapsed_ms).sum();
                if cpu_sum > 0 {
                    let speedup = cpu_sum as f64 / p.render_wall_time_ms as f64;
                    eprintln!(
                        "    total cpu time: {}ms, speedup: {:.1}x",
                        cpu_sum, speedup
                    );
                }
            }
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

    pub fn profile_json(&self) -> Option<String> {
        let profile = self.profile_json_data()?;
        serde_json::to_string_pretty(&profile).ok()
    }

    fn profile_json_data(&self) -> Option<ProfileJson<'_>> {
        let p = self.profile.as_ref()?;
        let total_cache = p.cache_hits + p.cache_misses;
        let hit_rate = if total_cache > 0 {
            Some((p.cache_hits as f64 / total_cache as f64) * 100.0)
        } else {
            None
        };
        let total_cpu_time_ms: u128 = p.page_timings.iter().map(|t| t.elapsed_ms).sum();
        let speedup = if p.render_wall_time_ms > 0 && total_cpu_time_ms > 0 {
            Some(total_cpu_time_ms as f64 / p.render_wall_time_ms as f64)
        } else {
            None
        };
        let avg_page_ms = if p.page_timings.is_empty() {
            None
        } else {
            Some(total_cpu_time_ms / p.page_timings.len() as u128)
        };
        let max_page_ms = p.page_timings.iter().map(|t| t.elapsed_ms).max();
        let mut slowest_pages: Vec<&PageTiming> = p.page_timings.iter().collect();
        slowest_pages.sort_by_key(|t| std::cmp::Reverse(t.elapsed_ms));

        Some(ProfileJson {
            schema_version: 1,
            total_ms: self.start.elapsed().as_millis(),
            phases: self
                .phases
                .iter()
                .map(|phase| PhaseTimingJson {
                    name: &phase.name,
                    elapsed_ms: phase.elapsed_ms,
                })
                .collect(),
            cache: CacheProfileJson {
                hits: p.cache_hits,
                misses: p.cache_misses,
                hit_rate,
            },
            parallel: ParallelProfileJson {
                threads: p.parallel_threads,
                render_wall_time_ms: p.render_wall_time_ms,
                total_cpu_time_ms,
                speedup,
            },
            rendering: RenderingProfileJson {
                page_renders: p.render_count,
                avg_page_ms,
                max_page_ms,
                slowest_pages: slowest_pages
                    .into_iter()
                    .take(5)
                    .map(|timing| PageTimingJson {
                        url: &timing.url,
                        template: &timing.template,
                        elapsed_ms: timing.elapsed_ms,
                    })
                    .collect(),
            },
        })
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

    #[test]
    fn profile_json_contains_stage_cache_render_and_parallel_metrics() {
        let mut timer = BuildTimer::with_profile();
        timer.phase("load_content");
        timer.finish();
        timer.set_cache_stats(3, 7);
        timer.set_parallel_stats(4, 12);
        timer.record_page_render("/post/", "post.html", 5);

        let json = timer.profile_json().expect("profile JSON should exist");
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["cache"]["hits"], 3);
        assert_eq!(value["cache"]["misses"], 7);
        assert_eq!(value["rendering"]["page_renders"], 1);
        assert_eq!(value["rendering"]["slowest_pages"][0]["url"], "/post/");
        assert_eq!(value["parallel"]["threads"], 4);
        assert!(value["phases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|phase| phase["name"] == "load_content"));
    }
}
