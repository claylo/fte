//! Resolving CLI inputs to concrete paths.

use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{AppError, Kind};

/// Where `input_dir` came from — determines how a missing directory is
/// treated by [`resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDirOrigin {
    /// The caller passed `--indir` explicitly.
    Explicit,
    /// `input_dir` came from config, default or otherwise.
    Default,
}

/// Resolve CLI inputs to concrete paths, plus a count of requested inputs
/// that don't exist anywhere — the caller folds that count into its failure
/// total so the summary and exit code reflect them.
///
/// When `inputs` is empty, the whole `input_dir` is scanned. A missing
/// directory named explicitly via `--indir` is a [`Kind::NotFound`] error; a
/// missing directory that came from config (its default included) is not a
/// fault — it prints a warning and resolves to zero inputs, since a run
/// asked to process "everything in a directory that isn't there" has
/// honestly processed nothing.
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
) -> Result<(Vec<PathBuf>, u32), AppError> {
    if inputs.is_empty() {
        if !input_dir.exists() {
            return match origin {
                InputDirOrigin::Explicit => Err(AppError::new(
                    Kind::NotFound,
                    format!("input directory {} not found", input_dir.display()),
                )
                .hint("check --indir")),
                InputDirOrigin::Default => {
                    eprintln!(
                        "warning: input directory {} does not exist",
                        input_dir.display()
                    );
                    Ok((Vec::new(), 0))
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
        return Ok((paths, 0));
    }

    let mut paths = Vec::new();
    let mut missing = 0u32;
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
                eprintln!("  FAIL {input}: not found");
                missing += 1;
            }
        }
    }
    Ok((paths, missing))
}
