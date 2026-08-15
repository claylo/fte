//! `fte detect` — report the detected format without extracting.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use fte::errors::{AppError, Kind};
use fte::inputs::InputDirOrigin;
use fte::output::{self, Render};
use fte::{config::Config, detect, inputs};

use crate::DetectArgs;

#[derive(serde::Serialize)]
struct DetectRow {
    id: String,
    path: String,
    format: String,
    status: &'static str,
}

pub fn run(args: &DetectArgs, cfg: &Config, render: Render) -> Result<ExitCode, AppError> {
    let origin = if args.indir.is_some() {
        InputDirOrigin::Explicit
    } else {
        InputDirOrigin::Default
    };
    let input_dir = args
        .indir
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.input_dir));

    let resolved = inputs::resolve(&args.inputs, &input_dir, origin, render)?;

    let mut rows = Vec::with_capacity(resolved.paths.len() + resolved.missing.len());
    for path in &resolved.paths {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let format = if ext == "epub" {
            detect::Format::Epub
        } else {
            let text = fs::read_to_string(path).map_err(|e| {
                AppError::new(Kind::IoError, format!("reading {}: {e}", path.display()))
            })?;
            detect::detect_format(path, &text, cfg)
        };

        rows.push(DetectRow {
            id: id.to_string(),
            path: path.display().to_string(),
            format: format.to_string(),
            status: "detected",
        });
    }

    for id in &resolved.missing {
        rows.push(DetectRow {
            id: id.clone(),
            path: String::new(),
            format: "unknown".to_string(),
            status: "failed",
        });
    }

    output::emit_items(&rows, render, |r| match r.status {
        "failed" => format!("  FAIL {}: not found", r.id),
        _ => format!("{}: {}", r.id, r.format),
    });

    Ok(if resolved.missing.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(fte::errors::PARTIAL_FAILURE)
    })
}
