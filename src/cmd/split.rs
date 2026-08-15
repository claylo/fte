//! `fte split` — book markdown into per-chapter files.

use std::path::PathBuf;
use std::process::ExitCode;

use fte::errors::{AppError, Kind};
use fte::output::{self, Render};
use fte::{chapter, config::Config, epub, splitter};

use crate::SplitArgs;

#[derive(serde::Serialize)]
struct SplitRow {
    index: usize,
    title: String,
    path: String,
    bytes: usize,
}

pub fn run(
    args: &SplitArgs,
    cfg: &Config,
    quiet: bool,
    render: Render,
) -> Result<ExitCode, AppError> {
    let book = args
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| AppError::new(Kind::Usage, "input has no usable filename"))?
        .to_string();

    let ext = args
        .input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // An ePub is extracted in memory rather than round-tripped through disk;
    // `fte extract` is what writes the combined document.
    let md = if ext == "epub" {
        epub::extract(&book, &args.input)
            .map_err(|e| AppError::new(Kind::ExtractionFailed, e.to_string()))?
    } else {
        std::fs::read_to_string(&args.input).map_err(|e| {
            AppError::new(
                Kind::IoError,
                format!("reading {}: {e}", args.input.display()),
            )
        })?
    };

    let doc = chapter::parse(&md);

    let front = if args.no_front {
        None
    } else {
        let tpl = args
            .front
            .clone()
            .unwrap_or_else(|| cfg.split.front.clone());
        if tpl.is_empty() { None } else { Some(tpl) }
    };

    let opts = splitter::Options {
        outdir: args
            .outdir
            .clone()
            .unwrap_or_else(|| PathBuf::from(&cfg.output_dir)),
        subdir: args
            .subdir
            .clone()
            .unwrap_or_else(|| cfg.split.subdir.clone()),
        file: args.name.clone().unwrap_or_else(|| cfg.split.file.clone()),
        front,
        pad: cfg.split.pad,
        force: args.force,
    };

    let written = splitter::split(&book, &doc, &opts).map_err(|e| {
        let msg = e.to_string();
        let kind = if msg.contains("no chapter markers") {
            Kind::NoChapters
        } else if msg.contains("unknown template token") || msg.contains("unterminated") {
            Kind::ConfigError
        } else if msg.contains("pass --force to overwrite") {
            Kind::OutputExists
        } else {
            Kind::IoError
        };
        AppError::new(kind, msg)
    })?;

    if !quiet && render == Render::Text {
        eprintln!("\nDone: {} files written", written.len());
    }

    let rows: Vec<SplitRow> = written
        .iter()
        .map(|w| SplitRow {
            index: w.index,
            title: w.title.clone(),
            path: w.path.display().to_string(),
            bytes: w.bytes,
        })
        .collect();

    output::emit_items(&rows, render, |r| {
        let kb = r.bytes / 1024;
        format!("  OK   {} ({kb}KB)", r.path)
    });

    Ok(ExitCode::SUCCESS)
}
