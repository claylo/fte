//! Resolving CLI inputs to concrete paths.

use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{AppError, Kind};
use crate::output::Render;

/// Where `input_dir` came from — determines how a missing directory is
/// treated by [`resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDirOrigin {
    /// The caller passed `--indir` explicitly.
    Explicit,
    /// `input_dir` came from config, default or otherwise.
    Default,
}

/// Concrete paths found, plus the requested inputs that resolved nowhere.
///
/// `resolve` no longer prints anything about `missing` itself — the caller
/// folds each entry into a row (status `"failed"`) so JSON mode reports it
/// through the structured envelope instead of stderr chatter.
#[derive(Debug, Default)]
pub struct Resolved {
    /// Concrete paths that exist on disk.
    pub paths: Vec<PathBuf>,
    /// Requested inputs (by their original string) that resolved nowhere.
    pub missing: Vec<String>,
}

/// Resolve CLI inputs to concrete paths, plus the requested inputs that
/// don't exist anywhere — the caller folds those into its row output and
/// failure total so the summary and exit code reflect them.
///
/// When `inputs` is empty, the whole `input_dir` is scanned. A missing
/// directory named explicitly via `--indir` is a [`Kind::NotFound`] error; a
/// missing directory that came from config (its default included) is not a
/// fault — in text mode it prints a warning and resolves to zero inputs,
/// since a run asked to process "everything in a directory that isn't
/// there" has honestly processed nothing. `render` decides whether that
/// warning is human chatter (printed) or noise on an otherwise-clean JSON
/// stderr (suppressed).
///
/// # Errors
///
/// Returns [`Kind::NotFound`] when `origin` is [`InputDirOrigin::Explicit`]
/// and `input_dir` does not exist, and [`Kind::IoError`] if reading an
/// existing directory fails.
pub fn resolve(
    inputs: &[String],
    input_dir: &Path,
    origin: InputDirOrigin,
    render: Render,
) -> Result<Resolved, AppError> {
    if inputs.is_empty() {
        if !input_dir.exists() {
            return match origin {
                InputDirOrigin::Explicit => Err(AppError::new(
                    Kind::NotFound,
                    format!("input directory {} not found", input_dir.display()),
                )
                .hint("check --indir")),
                InputDirOrigin::Default => {
                    if render == Render::Text {
                        eprintln!(
                            "warning: input directory {} does not exist",
                            input_dir.display()
                        );
                    }
                    Ok(Resolved::default())
                }
            };
        }

        let mut paths: Vec<PathBuf> = fs::read_dir(input_dir)
            .map_err(|e| {
                AppError::new(
                    Kind::IoError,
                    format!("reading {}: {e}", input_dir.display()),
                )
            })?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext == "html" || ext == "xml" || ext == "epub")
            })
            .collect();
        paths.sort();
        return Ok(Resolved {
            paths,
            missing: Vec::new(),
        });
    }

    let mut paths = Vec::new();
    let mut missing = Vec::new();
    for input in inputs {
        let p = Path::new(input);
        if p.exists() {
            paths.push(p.to_path_buf());
        } else {
            let epub = input_dir.join(format!("{input}.epub"));
            let xml = input_dir.join(format!("{input}.xml"));
            let html = input_dir.join(format!("{input}.html"));
            if epub.exists() {
                paths.push(epub);
            } else if xml.exists() {
                paths.push(xml);
            } else if html.exists() {
                paths.push(html);
            } else {
                missing.push(input.clone());
            }
        }
    }
    Ok(Resolved { paths, missing })
}
