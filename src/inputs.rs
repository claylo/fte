//! Resolving CLI inputs to concrete paths.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Resolve CLI inputs to concrete paths, plus a count of requested inputs
/// that don't exist anywhere — the caller folds that count into its failure
/// total so the summary and exit code reflect them.
pub fn resolve(inputs: &[String], input_dir: &Path) -> Result<(Vec<PathBuf>, u32)> {
    if inputs.is_empty() {
        let mut paths: Vec<PathBuf> = fs::read_dir(input_dir)
            .with_context(|| format!("reading {}", input_dir.display()))?
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
