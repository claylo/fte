//! `fte detect` — report the detected format without extracting.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use fte::{config::Config, detect, inputs};

use crate::DetectArgs;

pub fn run(args: &DetectArgs, cfg: &Config) -> Result<ExitCode> {
    let input_dir = args
        .indir
        .clone()
        .unwrap_or_else(|| PathBuf::from(&cfg.input_dir));

    let (paths, fail) = inputs::resolve(&args.inputs, &input_dir)?;

    for path in &paths {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let format = if ext == "epub" {
            detect::Format::Epub
        } else {
            let text =
                fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            detect::detect_format(path, &text, cfg)
        };

        println!("{id}: {format}");
    }

    Ok(if fail > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
