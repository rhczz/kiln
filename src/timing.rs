use std::time::Instant;

pub struct BuildTimer {
    start: Instant,
    phases: Vec<PhaseTiming>,
    current: Option<(String, Instant)>,
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
}
