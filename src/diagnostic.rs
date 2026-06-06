use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub source: PathBuf,
    pub line: Option<usize>,
    pub message: String,
    pub hint: Option<String>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.source.display(), self.message)?;
        if let Some(line) = self.line {
            write!(f, " (line {})", line)?;
        }
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {}", hint)?;
        }
        Ok(())
    }
}

impl Diagnostic {
    pub fn error(source: PathBuf, message: String) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            source,
            line: None,
            message,
            hint: None,
        }
    }

    pub fn warning(source: PathBuf, message: String) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            source,
            line: None,
            message,
            hint: None,
        }
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }
}

#[allow(dead_code)]
pub fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();
    for diag in diagnostics {
        let prefix = match diag.level {
            DiagnosticLevel::Error => "ERROR",
            DiagnosticLevel::Warning => "WARN ",
            DiagnosticLevel::Note => "NOTE ",
        };
        output.push_str(&format!("[{}] {}\n", prefix, diag));
    }
    output
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
        assert!(output.contains("[ERROR]"));
        assert!(output.contains("a.md"));
        assert!(output.contains("date"));
        assert!(output.contains("line 3"));
        assert!(output.contains("YYYY-MM-DD"));
    }

    #[test]
    fn formats_warning_without_line() {
        let diag = Diagnostic::warning(
            PathBuf::from("content/posts/old.md"),
            "no date-ordered items found".into(),
        );
        let output = format_diagnostics(&[diag]);
        assert!(output.contains("[WARN ]"));
        assert!(!output.contains("line"));
    }
}
