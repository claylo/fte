//! Declared error kinds, exit codes, and structured error rendering.

use std::fmt;

/// Exit code for the `partial_failure` outcome.
///
/// CLIspec 0.3 separates outcomes from errors: a run where some inputs
/// succeeded and others failed is data, not a fault, so it keeps exit 1 and
/// error kinds start at 2.
pub const PARTIAL_FAILURE: u8 = 1;

/// A declared error kind.
///
/// Every kind carries a stable snake_case name and a distinct exit code, both
/// published through `fte schema`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// Bad arguments, unknown subcommand, or a bare invocation.
    Usage,
    /// An input file or ID did not resolve.
    NotFound,
    /// Parsing produced no usable body content.
    ExtractionFailed,
    /// A target file exists and `--force` was not given.
    OutputExists,
    /// A read or write failed.
    IoError,
    /// `split` was given a document with no chapter markers.
    NoChapters,
    /// Malformed config, or an unknown filename-template token.
    ConfigError,
}

impl Kind {
    /// Every declared kind, for schema emission and exhaustiveness tests.
    pub const ALL: [Self; 7] = [
        Self::Usage,
        Self::NotFound,
        Self::ExtractionFailed,
        Self::OutputExists,
        Self::IoError,
        Self::NoChapters,
        Self::ConfigError,
    ];

    /// The process exit code for this kind.
    ///
    /// Codes are not contiguous: 4 (`unsupported_format`) was retired — see
    /// M18 in the final review — rather than reused, so a script that still
    /// checks for it fails closed instead of silently matching something
    /// else.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Usage => 2,
            Self::NotFound => 3,
            Self::ExtractionFailed => 5,
            Self::OutputExists => 6,
            Self::IoError => 7,
            Self::NoChapters => 8,
            Self::ConfigError => 9,
        }
    }

    /// The stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::NotFound => "not_found",
            Self::ExtractionFailed => "extraction_failed",
            Self::OutputExists => "output_exists",
            Self::IoError => "io_error",
            Self::NoChapters => "no_chapters",
            Self::ConfigError => "config_error",
        }
    }

    /// Whether retrying the same request can succeed.
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(self, Self::IoError)
    }

    /// Human-readable description, published in the schema.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Usage => "Invalid arguments, unrecognized command, or bad flag value.",
            Self::NotFound => "An input file or ID could not be found.",
            Self::ExtractionFailed => "Parsing produced no usable body content.",
            Self::OutputExists => "An output file already exists and --force was not given.",
            Self::IoError => "A read or write operation failed.",
            Self::NoChapters => "The document contains no chapter markers to split on.",
            Self::ConfigError => "Configuration or a filename template is invalid.",
        }
    }
}

/// A failure carrying a declared kind.
#[derive(Debug, Clone)]
pub struct AppError {
    /// Declared kind; determines the exit code.
    pub kind: Kind,
    /// Human-readable message.
    pub message: String,
    /// Optional actionable hint.
    pub hint: Option<String>,
}

impl AppError {
    /// Create an error of the given kind.
    pub fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    /// Attach an actionable hint.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Render the CLIspec error envelope as a single JSON line.
    #[must_use]
    pub fn to_json_line(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("kind".into(), self.kind.as_str().into());
        map.insert("message".into(), self.message.clone().into());
        if let Some(hint) = &self.hint {
            map.insert("hint".into(), hint.clone().into());
        }
        serde_json::Value::Object(map).to_string()
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, " ({hint})")?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}

/// Print a failure to stderr in the selected format.
///
/// JSON mode prints the envelope as the last line of stderr, per CLIspec 0.3.
pub fn emit(err: &AppError, json: bool) {
    if json {
        eprintln!("{}", err.to_json_line());
    } else {
        eprintln!("error: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_distinct_code_above_the_outcome() {
        let kinds = Kind::ALL;
        let mut codes: Vec<u8> = kinds.iter().map(|k| k.code()).collect();
        codes.sort_unstable();
        let unique = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), unique, "duplicate exit codes");
        assert!(
            codes.iter().all(|&c| c > PARTIAL_FAILURE),
            "an error code collides with the partial_failure outcome"
        );
    }

    #[test]
    fn codes_match_the_spec() {
        assert_eq!(Kind::Usage.code(), 2);
        assert_eq!(Kind::NotFound.code(), 3);
        assert_eq!(Kind::ExtractionFailed.code(), 5);
        assert_eq!(Kind::OutputExists.code(), 6);
        assert_eq!(Kind::IoError.code(), 7);
        assert_eq!(Kind::NoChapters.code(), 8);
        assert_eq!(Kind::ConfigError.code(), 9);
    }

    #[test]
    fn kind_names_are_snake_case() {
        for kind in Kind::ALL {
            let name = kind.as_str();
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{name} is not snake_case"
            );
        }
    }

    #[test]
    fn only_io_errors_are_retryable() {
        assert!(Kind::IoError.retryable());
        assert!(!Kind::NotFound.retryable());
        assert!(!Kind::Usage.retryable());
    }

    #[test]
    fn json_rendering_is_a_single_line_envelope() {
        let err = AppError::new(Kind::NotFound, "input 'foo.html' not found").hint("check --indir");
        let line = err.to_json_line();
        assert!(!line.contains('\n'));

        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["kind"], "not_found");
        assert_eq!(parsed["message"], "input 'foo.html' not found");
        assert_eq!(parsed["hint"], "check --indir");
    }

    #[test]
    fn a_hintless_error_omits_the_field() {
        let line = AppError::new(Kind::IoError, "disk full").to_json_line();
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(parsed.get("hint").is_none());
    }
}
