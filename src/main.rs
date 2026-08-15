use librebar::cli::clap::{self, Args, Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

mod cmd;

use fte::config;
use fte::errors::{AppError, Kind};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    version,
    about = "Extract clean markdown from publisher HTML/XML/ePub academic papers",
    arg_required_else_help = true
)]
struct Cli {
    #[command(flatten)]
    common: librebar::cli::CommonArgs,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract markdown from publisher markup
    Extract(ExtractArgs),
    /// Report the detected format without extracting
    Detect(DetectArgs),
    /// Split book markdown into per-chapter files
    Split(SplitArgs),
}

#[derive(Args)]
pub struct ExtractArgs {
    /// Input files or IDs (looked up in input_dir)
    pub inputs: Vec<String>,

    /// Input directory to scan (overrides config)
    #[arg(long)]
    pub indir: Option<PathBuf>,

    /// Output directory (overrides config)
    #[arg(short, long)]
    pub outdir: Option<PathBuf>,

    /// Print to stdout instead of writing files
    #[arg(long)]
    pub stdout: bool,

    /// Overwrite existing output files
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct DetectArgs {
    /// Input files or IDs (looked up in input_dir)
    pub inputs: Vec<String>,

    /// Input directory to scan (overrides config)
    #[arg(long)]
    pub indir: Option<PathBuf>,
}

#[derive(Args)]
pub struct SplitArgs {
    /// Book markdown (.md) or ePub (.epub) to split
    pub input: PathBuf,

    /// Output directory (overrides config)
    #[arg(short, long)]
    pub outdir: Option<PathBuf>,

    /// Subdirectory template (overrides config)
    #[arg(long)]
    pub subdir: Option<String>,

    /// Chapter filename template (overrides config)
    #[arg(long)]
    pub name: Option<String>,

    /// Frontmatter filename template (overrides config)
    #[arg(long, conflicts_with = "no_front")]
    pub front: Option<String>,

    /// Do not write a frontmatter file
    #[arg(long)]
    pub no_front: bool,

    /// Overwrite existing output files
    #[arg(long)]
    pub force: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            fte::errors::emit(&err, wants_json_errors());
            ExitCode::from(err.kind.code())
        }
    }
}

/// Whether a failure should print the structured JSON envelope.
///
/// This runs in `main`, before and independently of clap: `run` can fail while
/// loading config, long before a parsed `CommonArgs` exists. It mirrors
/// librebar's `--format auto` rule — JSON when stdout is not a terminal —
/// while honoring an explicit flag either way.
fn wants_json_errors() -> bool {
    let mut explicit = None;
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" | "--format=json" => explicit = Some(true),
            "--format=text" => explicit = Some(false),
            "--format" => match args.next().as_deref() {
                Some("json") => explicit = Some(true),
                Some("text") => explicit = Some(false),
                _ => {}
            },
            _ => {}
        }
    }
    explicit.unwrap_or_else(|| !std::io::IsTerminal::is_terminal(&std::io::stdout()))
}

fn run() -> std::result::Result<ExitCode, fte::errors::AppError> {
    let cli: Cli = librebar::cli::parse();

    if cli
        .common
        .apply(VERSION)
        .map_err(|e| AppError::new(Kind::ConfigError, e.to_string()))?
        .is_exit()
    {
        return Ok(ExitCode::SUCCESS);
    }

    let cwd = std::env::current_dir()
        .map_err(|e| AppError::new(Kind::IoError, format!("reading current directory: {e}")))?;
    let cwd_utf8 = cwd
        .to_str()
        .ok_or_else(|| AppError::new(Kind::ConfigError, "cwd is not valid UTF-8"))?;

    // Load config: struct defaults → user config → project config.
    // An explicit `-c/--config` file is layered on top of whatever discovery finds.
    let mut loader = librebar::config::ConfigLoader::new("fte").with_project_search(cwd_utf8);
    if let Some(path) = cli
        .common
        .config_path()
        .map_err(|e| AppError::new(Kind::ConfigError, e.to_string()))?
    {
        loader = loader.with_file(&path);
    }
    let (cfg, _sources) = loader
        .load::<config::Config>()
        .map_err(|e| AppError::new(Kind::ConfigError, e.to_string()))?;

    let quiet = cli.common.quiet;
    let verbose = cli.common.verbose > 0;

    use librebar::cli::ResolvedOutputFormat;
    let render = match cli.common.output_format() {
        ResolvedOutputFormat::Json => fte::output::Render::Json,
        _ => fte::output::Render::Text,
    };

    match &cli.command {
        Commands::Extract(args) => cmd::extract::run(args, &cfg, quiet, verbose, render),
        Commands::Detect(args) => cmd::detect::run(args, &cfg, render),
        Commands::Split(args) => cmd::split::run(args, &cfg, quiet, render),
    }
}
