//! `fte split` — book markdown into per-chapter files.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::ExitCode;

use fte::{chapter, config::Config, epub, splitter};

use crate::SplitArgs;

pub fn run(args: &SplitArgs, cfg: &Config, quiet: bool) -> Result<ExitCode> {
    let book = args
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .context("input has no usable filename")?
        .to_string();

    let ext = args
        .input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // An ePub is extracted in memory rather than round-tripped through disk;
    // `fte extract` is what writes the combined document.
    let md = if ext == "epub" {
        epub::extract(&book, &args.input)?
    } else {
        std::fs::read_to_string(&args.input)
            .with_context(|| format!("reading {}", args.input.display()))?
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

    let written = splitter::split(&book, &doc, &opts)?;

    if !quiet {
        for w in &written {
            let kb = w.bytes / 1024;
            eprintln!("  OK   {} ({kb}KB)", w.path.display());
        }
        eprintln!("\nDone: {} files written", written.len());
    }

    Ok(ExitCode::SUCCESS)
}
