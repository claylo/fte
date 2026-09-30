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
    /// Why a `"failed"` row failed. `None` for every other status.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
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
            // only diagnostic a missing input gets, and only in text mode —
            // JSON mode's stderr carries the structured error envelope only.
            if render == Render::Text {
                eprintln!("  FAIL {id}: not found");
            }
        } else {
            rows.push(ExtractRow {
                id: id.clone(),
                source_format: "unknown".to_string(),
                output_path: None,
                bytes: 0,
                status: "failed",
                reason: Some("not found".to_string()),
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

        if verbose && render == Render::Text {
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
                reason: None,
            });
            continue;
        }

        let result = match &content {
            None => epub::extract(id, path, &cfg.epub),
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
                        reason: None,
                    });
                }
                ok += 1;
            }
            Err(e) => {
                if args.stdout {
                    // No JSON items channel exists in --stdout mode; this is
                    // the only diagnostic a failed extraction gets, and only
                    // in text mode — see the missing-input arm above.
                    if render == Render::Text {
                        eprintln!("  FAIL {id}: {e}");
                    }
                } else {
                    // The row is the sole report of this failure — it goes
                    // out once, from output::emit_items below, on whichever
                    // stream `render` selects. Printing it here too would
                    // double-report the same failure under two identifiers.
                    rows.push(ExtractRow {
                        id: id.to_string(),
                        source_format: format.to_string(),
                        output_path: None,
                        bytes: 0,
                        status: "failed",
                        reason: Some(e.to_string()),
                    });
                }
                fail += 1;
            }
        }
    }

    if !args.stdout {
        output::emit_items(&rows, render, |r| match r.status {
            "skipped" => format!("  SKIP {}.md (exists; use --force)", r.id),
            // Discriminate on `reason`, not `source_format`: `source_format`
            // is "unknown" both for an input that never resolved to a path
            // AND for a resolved file whose format `detect_format` could not
            // classify (N1) — it does not distinguish "missing" from
            // "unsupported". `reason` always carries the real cause for
            // either case, so there is nothing left to special-case.
            "failed" => format!(
                "  FAIL {}: {}",
                r.id,
                r.reason.as_deref().unwrap_or("failed")
            ),
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
