//! `fte extract` — publisher markup to markdown.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use fte::errors::{AppError, Kind};
use fte::inputs::InputDirOrigin;
use fte::output::{self, Render};
use fte::{config::Config, detect, epub, extract, inputs};

use crate::ExtractArgs;

#[derive(serde::Serialize)]
struct ExtractRow {
    id: String,
    source_format: String,
    output_path: Option<String>,
    bytes: usize,
    status: &'static str,
}

pub fn run(
    args: &ExtractArgs,
    cfg: &Config,
    quiet: bool,
    verbose: bool,
    render: Render,
) -> Result<ExitCode, AppError> {
    let origin = if args.indir.is_some() {
        InputDirOrigin::Explicit
    } else {
        InputDirOrigin::Default
    };
    let input_dir = args
        .indir
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.input_dir));
    let output_dir = args
        .outdir
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.output_dir));

    if !args.stdout {
        fs::create_dir_all(&output_dir).map_err(|e| {
            AppError::new(
                Kind::IoError,
                format!("creating {}: {e}", output_dir.display()),
            )
        })?;
    }

    let resolved = inputs::resolve(&args.inputs, &input_dir, origin, render)?;
    let mut fail = u32::try_from(resolved.missing.len()).unwrap_or(u32::MAX);

    let mut ok = 0u32;
    let mut skip = 0u32;
    let mut rows = Vec::with_capacity(resolved.paths.len() + resolved.missing.len());

    for id in &resolved.missing {
        if args.stdout {
            // No JSON items channel exists in --stdout mode; this is the
            // only diagnostic a missing input gets.
            eprintln!("  FAIL {id}: not found");
        } else {
            rows.push(ExtractRow {
                id: id.clone(),
                source_format: "unknown".to_string(),
                output_path: None,
                bytes: 0,
                status: "failed",
            });
        }
    }

    for path in &resolved.paths {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let (format, content) = if ext == "epub" {
            (detect::Format::Epub, None)
        } else {
            let text = fs::read_to_string(path).map_err(|e| {
                AppError::new(Kind::IoError, format!("reading {}: {e}", path.display()))
            })?;
            let format = detect::detect_format(path, &text, cfg);
            (format, Some(text))
        };

        if verbose {
            eprintln!("  detect {id}: {format}");
        }

        let out_path = output_dir.join(format!("{id}.md"));
        if !args.force && !args.stdout && out_path.exists() {
            skip += 1;
            rows.push(ExtractRow {
                id: id.to_string(),
                source_format: format.to_string(),
                output_path: Some(out_path.display().to_string()),
                bytes: 0,
                status: "skipped",
            });
            continue;
        }

        let result = match &content {
            None => epub::extract(id, path),
            Some(text) => extract::extract(&format, id, text),
        };

        match result {
            Ok(md) => {
                if args.stdout {
                    println!("{md}");
                } else {
                    fs::write(&out_path, &md).map_err(|e| {
                        AppError::new(
                            Kind::IoError,
                            format!("writing {}: {e}", out_path.display()),
                        )
                    })?;
                    rows.push(ExtractRow {
                        id: id.to_string(),
                        source_format: format.to_string(),
                        output_path: Some(out_path.display().to_string()),
                        bytes: md.len(),
                        status: "extracted",
                    });
                }
                ok += 1;
            }
            Err(e) => {
                // In --stdout mode this is the only diagnostic (no JSON
                // items channel); otherwise it's stderr chatter, so it
                // stays text-mode only — JSON mode reports it as a row.
                if args.stdout || render == Render::Text {
                    eprintln!("  FAIL {id}: {e}");
                }
                if !args.stdout {
                    rows.push(ExtractRow {
                        id: id.to_string(),
                        source_format: format.to_string(),
                        output_path: None,
                        bytes: 0,
                        status: "failed",
                    });
                }
                fail += 1;
            }
        }
    }

    if !args.stdout {
        output::emit_items(&rows, render, |r| match r.status {
            "skipped" => format!("  SKIP {}.md (exists; use --force)", r.id),
            // `source_format` stays "unknown" only for an input that never
            // resolved to a path; a resolved input that failed extraction
            // always has a real detected format.
            "failed" if r.source_format == "unknown" => format!("  FAIL {}: not found", r.id),
            "failed" => format!("  FAIL {}.md", r.id),
            _ => format!("  OK   {}.md ({}KB)", r.id, r.bytes / 1024),
        });
    }

    if !quiet && !args.stdout && render == Render::Text {
        eprintln!("\nDone: {ok} extracted, {skip} skipped, {fail} failed");
    }

    Ok(if fail > 0 {
        ExitCode::from(fte::errors::PARTIAL_FAILURE)
    } else {
        ExitCode::SUCCESS
    })
}
