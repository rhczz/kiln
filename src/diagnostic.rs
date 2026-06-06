use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone)]
pub struct SourceContext {
    pub snippet: String,
    pub highlight_line: usize,
}

#[derive(Debug, Clone)]
pub struct TemplateFrame {
    pub template: String,
    pub line: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub source: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub hint: Option<String>,
    pub source_context: Option<SourceContext>,
    pub template_stack: Vec<TemplateFrame>,
}

impl Diagnostic {
    fn new(level: DiagnosticLevel, source: PathBuf, message: String) -> Self {
        Self {
            level,
            source,
            line: None,
            column: None,
            message,
            hint: None,
            source_context: None,
            template_stack: Vec::new(),
        }
    }

    pub fn error(source: PathBuf, message: String) -> Self {
        Self::new(DiagnosticLevel::Error, source, message)
    }

    pub fn warning(source: PathBuf, message: String) -> Self {
        Self::new(DiagnosticLevel::Warning, source, message)
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_column(mut self, col: usize) -> Self {
        self.column = Some(col);
        self
    }

    pub fn with_hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }

    pub fn with_source_context(mut self) -> Self {
        self.source_context = read_source_context(&self.source, self.line, 2);
        self
    }

    pub fn with_template_stack(mut self, stack: Vec<TemplateFrame>) -> Self {
        self.template_stack = stack;
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level_str = match self.level {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
            DiagnosticLevel::Note => "note",
        };
        let location = format_location(&self.source, self.line, self.column);
        write!(f, "{}: {}: {}", location, level_str, self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {}", hint)?;
        }
        for frame in &self.template_stack {
            write!(
                f,
                "\n  in template: {}:{}",
                frame.template,
                frame.line.map_or("-".to_string(), |l| l.to_string())
            )?;
        }
        Ok(())
    }
}

pub fn format_location(source: &Path, line: Option<usize>, column: Option<usize>) -> String {
    match (line, column) {
        (Some(l), Some(c)) => format!("{}:{}:{}", source.display(), l, c),
        (Some(l), None) => format!("{}:{}", source.display(), l),
        _ => source.display().to_string(),
    }
}

pub fn read_source_context(
    source: &Path,
    line: Option<usize>,
    context_lines: usize,
) -> Option<SourceContext> {
    let line = line?;
    let content = std::fs::read_to_string(source).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let target = line.saturating_sub(1);
    let start = target.saturating_sub(context_lines);
    let end = (target + context_lines + 1).min(lines.len());
    if start >= end {
        return None;
    }
    let snippet = lines[start..end].join("\n");
    Some(SourceContext {
        snippet,
        highlight_line: target - start,
    })
}

#[allow(dead_code)]
pub fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();
    for diag in diagnostics {
        output.push_str(&format!("{}\n", diag));
    }
    output
}

fn use_color() -> bool {
    use std::sync::OnceLock;
    static COLOR: OnceLock<bool> = OnceLock::new();
    *COLOR.get_or_init(|| {
        std::env::var("NO_COLOR").is_err()
            && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(false)
    })
}

pub fn emit_diagnostic(diag: &Diagnostic) {
    if use_color() {
        emit_colored(diag);
    } else {
        eprintln!("{}", diag);
    }
}

fn indent_lines(text: &str, indent: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                line.to_string()
            } else {
                format!("{}{}", indent, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn emit_colored(diag: &Diagnostic) {
    let (level_str, color) = match diag.level {
        DiagnosticLevel::Error => ("error", "\x1b[31m"),
        DiagnosticLevel::Warning => ("warning", "\x1b[33m"),
        DiagnosticLevel::Note => ("note", "\x1b[34m"),
    };
    let reset = "\x1b[0m";
    let dim = "\x1b[2m";
    let bold = "\x1b[1m";

    let location = format_location(&diag.source, diag.line, diag.column);
    let message = indent_lines(&diag.message, &format!("{}  {}{}", reset, color, bold));

    eprintln!("{}{}:{} {}{}", color, bold, level_str, message, reset);
    eprintln!("  {}-->{} {}", dim, reset, location);

    if let Some(ctx) = &diag.source_context {
        for (i, line) in ctx.snippet.lines().enumerate() {
            let marker = if i == ctx.highlight_line {
                " > "
            } else {
                "   "
            };
            eprintln!("{}{}{}{}", dim, marker, reset, line);
            if i == ctx.highlight_line {
                eprintln!("{}   {}^{}", dim, color, reset);
            }
        }
    }

    if let Some(hint) = &diag.hint {
        let hint_indented = indent_lines(hint, "         ");
        eprintln!("  \x1b[34mhint:{} {}", reset, hint_indented);
    }

    for frame in &diag.template_stack {
        eprintln!(
            "  {}in template:{} {}:{}",
            dim,
            reset,
            frame.template,
            frame.line.map_or("-".to_string(), |l| l.to_string())
        );
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticCollector {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
    }

    pub fn summary(&self) -> (usize, usize) {
        let errors = self
            .diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .count();
        let warnings = self
            .diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Warning)
            .count();
        (errors, warnings)
    }

    pub fn emit_all(&self) {
        for diag in &self.diagnostics {
            emit_diagnostic(diag);
        }
    }
}

pub fn print_build_summary(collector: &DiagnosticCollector) {
    let (errors, warnings) = collector.summary();
    if errors > 0 || warnings > 0 {
        eprintln!(
            "\nBuild finished with {} error(s), {} warning(s).",
            errors, warnings
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_error_diagnostic() {
        let diag = Diagnostic::error(
            PathBuf::from("content/posts/a.md"),
            "frontmatter field `date` invalid".into(),
        )
        .with_line(3)
        .with_hint("expected format YYYY-MM-DD");
        let output = format_diagnostics(&[diag]);
        assert!(output.contains("a.md:3"));
        assert!(output.contains("error"));
        assert!(output.contains("date"));
        assert!(output.contains("YYYY-MM-DD"));
    }

    #[test]
    fn formats_warning_without_line() {
        let diag = Diagnostic::warning(
            PathBuf::from("content/posts/old.md"),
            "no date-ordered items found".into(),
        );
        let output = format_diagnostics(&[diag]);
        assert!(output.contains("warning"));
        assert!(!output.contains("(line"));
    }

    #[test]
    fn formats_location_with_column() {
        let loc = format_location(&PathBuf::from("content/a.md"), Some(12), Some(5));
        assert_eq!(loc, "content/a.md:12:5");
    }

    #[test]
    fn formats_location_without_column() {
        let loc = format_location(&PathBuf::from("content/a.md"), Some(12), None);
        assert_eq!(loc, "content/a.md:12");
    }

    #[test]
    fn collector_counts_errors_and_warnings() {
        let mut collector = DiagnosticCollector::new();
        collector.push(Diagnostic::error(PathBuf::from("a.md"), "e1".into()));
        collector.push(Diagnostic::warning(PathBuf::from("b.md"), "w1".into()));
        collector.push(Diagnostic::error(PathBuf::from("c.md"), "e2".into()));

        assert!(collector.has_errors());
        assert_eq!(collector.summary(), (2, 1));
    }

    #[test]
    fn empty_collector_has_no_errors() {
        let collector = DiagnosticCollector::new();
        assert!(!collector.has_errors());
        assert_eq!(collector.summary(), (0, 0));
    }

    #[test]
    fn read_source_context_extracts_lines() {
        let dir = std::env::temp_dir().join(format!(
            "kiln-diag-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.md");
        std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\nline6\nline7\n").unwrap();

        let ctx = read_source_context(&file, Some(4), 2).unwrap();
        assert_eq!(ctx.highlight_line, 2);
        assert!(ctx.snippet.contains("line2"));
        assert!(ctx.snippet.contains("line4"));
        assert!(ctx.snippet.contains("line6"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_source_context_returns_none_for_missing_line() {
        let ctx = read_source_context(Path::new("nonexistent.md"), Some(1), 2);
        assert!(ctx.is_none());
    }

    #[test]
    fn display_includes_template_stack() {
        let diag = Diagnostic::error(PathBuf::from("a.md"), "bad template".into())
            .with_template_stack(vec![TemplateFrame {
                template: "base.html".into(),
                line: Some(10),
            }]);
        let output = format!("{}", diag);
        assert!(output.contains("in template: base.html:10"));
    }
}
