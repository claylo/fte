use anyhow::{Context, Result};
use librebar::cli::clap::{self, Parser};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fte::{config, detect, epub, extract};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    version,
    about = "Extract clean markdown from publisher HTML/XML/ePub academic papers"
)]
struct Cli {
    #[command(flatten)]
    common: librebar::cli::CommonArgs,

    /// Input files or IDs (looked up in input_dir)
    inputs: Vec<String>,

    /// Input directory to scan (overrides config)
    #[arg(long)]
    indir: Option<PathBuf>,

    /// Output directory (overrides config)
    #[arg(short, long)]
    outdir: Option<PathBuf>,

    /// Print to stdout instead of writing files
    #[arg(long)]
    stdout: bool,

    /// Overwrite existing output files
    #[arg(long)]
    force: bool,

    /// Show detected format without extracting
    #[arg(long)]
    detect_only: bool,
}

fn main() -> Result<ExitCode> {
    let cli: Cli = librebar::cli::parse();

    if cli.common.apply(VERSION)?.is_exit() {
        return Ok(ExitCode::SUCCESS);
    }

    let cwd = std::env::current_dir()?;
    let cwd_utf8 = cwd.to_str().context("cwd is not valid UTF-8")?;

    // Load config: struct defaults → user config → project config.
    // An explicit `-c/--config` file is layered on top of whatever discovery finds.
    let mut loader = librebar::config::ConfigLoader::new("fte").with_project_search(cwd_utf8);
    if let Some(path) = cli.common.config_path()? {
        loader = loader.with_file(&path);
    }
    let (cfg, _sources) = loader.load::<config::Config>()?;

    // Resolve directories: CLI > config > defaults
    let input_dir = cli.indir.unwrap_or_else(|| PathBuf::from(&cfg.input_dir));
    let output_dir = cli.outdir.unwrap_or_else(|| PathBuf::from(&cfg.output_dir));

    if !cli.stdout && !cli.detect_only {
        fs::create_dir_all(&output_dir)?;
    }

    let quiet = cli.common.quiet;
    let verbose = cli.common.verbose > 0;

    // Inputs that were requested but don't exist count as failures: the run
    // was asked to do work it couldn't do, and the exit code must say so.
    let (inputs, mut fail) = resolve_inputs(&cli.inputs, &input_dir)?;

    let mut ok = 0u32;
    let mut skip = 0u32;

    for path in &inputs {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // ePub is a zip, extracted from the path; everything else is read to
        // a string and detected by content.
        let (format, content) = if ext == "epub" {
            (detect::Format::Epub, None)
        } else {
            let text =
                fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            let format = detect::detect_format(path, &text, &cfg);
            (format, Some(text))
        };

        if cli.detect_only {
            println!("{id}: {format}");
            continue;
        }

        if verbose {
            eprintln!("  detect {id}: {format}");
        }

        let out_path = output_dir.join(format!("{id}.md"));
        if !cli.force && out_path.exists() {
            skip += 1;
            continue;
        }

        let result = match &content {
            None => epub::extract(id, path),
            Some(text) => extract::extract(&format, id, text),
        };

        match result {
            Ok(md) => {
                if cli.stdout {
                    println!("{md}");
                } else {
                    fs::write(&out_path, &md)
                        .with_context(|| format!("writing {}", out_path.display()))?;
                    if !quiet {
                        let kb = md.len() / 1024;
                        eprintln!("  OK   {id}.md ({kb}KB)");
                    }
                }
                ok += 1;
            }
            Err(e) => {
                eprintln!("  FAIL {id}: {e}");
                fail += 1;
            }
        }
    }

    if !quiet && !cli.stdout && !cli.detect_only {
        eprintln!("\nDone: {ok} extracted, {skip} skipped, {fail} failed");
    }

    Ok(if fail > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Resolve CLI inputs to concrete paths, plus a count of requested inputs
/// that don't exist anywhere — the caller folds that count into its failure
/// total so the summary and exit code reflect them.
fn resolve_inputs(inputs: &[String], input_dir: &Path) -> Result<(Vec<PathBuf>, u32)> {
    if inputs.is_empty() {
        // Process all files in input_dir
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
            // Try as ID in input_dir
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
