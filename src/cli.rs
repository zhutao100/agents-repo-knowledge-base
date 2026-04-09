use clap::{Parser, Subcommand, ValueEnum};

use crate::error::KbError;
use crate::index::{index_check, index_regen, IndexScope};
use crate::io::json::write_json_stdout;
use crate::repo::diff_source::{DiffSource, DiffSourceParseError};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Text,
}

#[derive(Debug, Parser)]
#[command(name = "kb", disable_help_subcommand = false)]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json, global = true)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Version,
    Index(IndexCommand),
}

#[derive(serde::Serialize)]
struct VersionJson<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(serde::Serialize)]
struct OkJson {
    ok: bool,
}

#[derive(Debug, Parser)]
struct IndexCommand {
    #[command(subcommand)]
    command: IndexCommands,
}

#[derive(Debug, Subcommand)]
enum IndexCommands {
    Regen(IndexRegenCommand),
    Check(IndexCheckCommand),
}

#[derive(Debug, Parser)]
struct IndexRegenCommand {
    #[arg(long, value_enum, default_value_t = IndexScope::All)]
    scope: IndexScope,

    #[arg(long)]
    diff_source: String,
}

#[derive(Debug, Parser)]
struct IndexCheckCommand {
    #[arg(long)]
    diff_source: String,
}

pub fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let format = cli.format;

    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            if matches!(format, OutputFormat::Json) {
                let _ = write_json_stdout(&err.to_json_error());
            } else {
                eprintln!("error[{}]: {}", err.code.as_str(), err.message);
                for detail in &err.details {
                    eprintln!("  {}={}", detail.key, detail.value);
                }
            }
            std::process::ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<(), KbError> {
    match cli.command {
        Commands::Version => match cli.format {
            OutputFormat::Json => write_json_stdout(&VersionJson {
                name: "kb",
                version: env!("CARGO_PKG_VERSION"),
            })
            .map_err(|err| KbError::internal(err, "failed to write json"))?,
            OutputFormat::Text => {
                println!("kb {}", env!("CARGO_PKG_VERSION"));
            }
        },
        Commands::Index(index) => match index.command {
            IndexCommands::Regen(cmd) => {
                let diff_source = parse_diff_source(&cmd.diff_source)?;
                index_regen(&diff_source, cmd.scope)?;
                match cli.format {
                    OutputFormat::Json => write_json_stdout(&OkJson { ok: true })
                        .map_err(|err| KbError::internal(err, "failed to write json"))?,
                    OutputFormat::Text => println!("ok"),
                }
            }
            IndexCommands::Check(cmd) => {
                let diff_source = parse_diff_source(&cmd.diff_source)?;
                index_check(&diff_source)?;
                match cli.format {
                    OutputFormat::Json => write_json_stdout(&OkJson { ok: true })
                        .map_err(|err| KbError::internal(err, "failed to write json"))?,
                    OutputFormat::Text => println!("ok"),
                }
            }
        },
    }

    Ok(())
}

fn map_diff_source_parse_error(err: DiffSourceParseError, input: &str) -> KbError {
    match err {
        DiffSourceParseError::Empty => KbError::invalid_argument("diff_source must not be empty")
            .with_detail("diff_source", input),
        DiffSourceParseError::Unknown => {
            KbError::invalid_argument("diff_source must be one of: staged, worktree, commit:<sha>")
                .with_detail("diff_source", input)
        }
        DiffSourceParseError::MissingCommit => {
            KbError::invalid_argument("commit sha is required").with_detail("diff_source", input)
        }
    }
}

fn parse_diff_source(input: &str) -> Result<DiffSource, KbError> {
    DiffSource::parse(input).map_err(|err| map_diff_source_parse_error(err, input))
}
