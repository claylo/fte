//! `fte split` — book markdown into per-chapter files.

use std::path::PathBuf;
use std::process::ExitCode;

use fte::errors::{AppError, Kind};
use fte::{chapter, config::Config, epub, splitter};

use crate::SplitArgs;

pub fn run(args: &SplitArgs, cfg: &Config, quiet: bool) -> Result<ExitCode, AppError> {
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
        } else if msg.contains("exists") {
            Kind::OutputExists
        } else {
            Kind::IoError
        };
        AppError::new(kind, msg)
    })?;

    if !quiet {
        for w in &written {
            let kb = w.bytes / 1024;
            eprintln!("  OK   {} ({kb}KB)", w.path.display());
        }
        eprintln!("\nDone: {} files written", written.len());
    }

    Ok(ExitCode::SUCCESS)
}
