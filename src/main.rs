use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

mod config;
mod depth;
mod detect;
mod epub;
mod extract;
mod html;
mod jats;
mod markdown;
mod wiley_xml;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(about = "Extract clean markdown from publisher HTML/XML/ePub academic papers")]
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.common.apply(VERSION)?.is_exit() {
        return Ok(());
    }

    let cwd = std::env::current_dir()?;
    let cwd_utf8 = cwd.to_str().context("cwd is not valid UTF-8")?;

    // Load config: struct defaults → user config → project config
    let (cfg, _sources) = librebar::config::ConfigLoader::new("fte")
        .with_project_search(cwd_utf8)
        .load::<config::Config>()?;

    // Resolve directories: CLI > config > defaults
    let input_dir = cli.indir.unwrap_or_else(|| PathBuf::from(&cfg.input_dir));
    let output_dir = cli
        .outdir
        .unwrap_or_else(|| PathBuf::from(&cfg.output_dir));

    if !cli.stdout && !cli.detect_only {
        fs::create_dir_all(&output_dir)?;
    }

    let inputs = resolve_inputs(&cli.inputs, &input_dir)?;

    let mut ok = 0u32;
    let mut skip = 0u32;
    let mut fail = 0u32;

    for path in &inputs {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        // ePub: handle before reading to string (it's a zip)
        if ext == "epub" {
            let format = detect::Format::Epub;

            if cli.detect_only {
                println!("{id}: {format}");
                continue;
            }

            let out_path = output_dir.join(format!("{id}.md"));
            if !cli.force && out_path.exists() {
                skip += 1;
                continue;
            }

            match epub::extract(id, path) {
                Ok(md) => {
                    if cli.stdout {
                        println!("{md}");
                    } else {
                        fs::write(&out_path, &md)
                            .with_context(|| format!("writing {}", out_path.display()))?;
                        let kb = md.len() / 1024;
                        eprintln!("  OK   {id}.md ({kb}KB)");
                        ok += 1;
                    }
                }
                Err(e) => {
                    eprintln!("  FAIL {id}: {e}");
                    fail += 1;
                }
            }
            continue;
        }

        // HTML/XML: read to string, detect, extract
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

        let format = detect::detect_format(path, &content, &cfg);

        if cli.detect_only {
            println!("{id}: {format}");
            continue;
        }

        let out_path = output_dir.join(format!("{id}.md"));
        if !cli.force && out_path.exists() {
            skip += 1;
            continue;
        }

        match extract::extract(&format, id, &content) {
            Ok(md) => {
                if cli.stdout {
                    println!("{md}");
                } else {
                    fs::write(&out_path, &md)
                        .with_context(|| format!("writing {}", out_path.display()))?;
                    let kb = md.len() / 1024;
                    eprintln!("  OK   {id}.md ({kb}KB)");
                    ok += 1;
                }
            }
            Err(e) => {
                eprintln!("  FAIL {id}: {e}");
                fail += 1;
            }
        }
    }

    if !cli.stdout && !cli.detect_only {
        eprintln!("\nDone: {ok} extracted, {skip} skipped, {fail} failed");
    }

    Ok(())
}

fn resolve_inputs(inputs: &[String], input_dir: &Path) -> Result<Vec<PathBuf>> {
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
        return Ok(paths);
    }

    let mut paths = Vec::new();
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
                eprintln!("  SKIP {input}: not found");
            }
        }
    }
    Ok(paths)
}
