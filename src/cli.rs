use clap::{Parser, Subcommand, ValueEnum};

use crate::error::KbError;
use crate::io::json::write_json_stdout;
use crate::repo::diff_source::{DiffSource, DiffSourceParseError};
use crate::repo::root::discover_repo_root;

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
    Debug(DebugCommand),
}

#[derive(Debug, Parser)]
struct DebugCommand {
    #[command(subcommand)]
    command: DebugCommands,
}

#[derive(Debug, Subcommand)]
enum DebugCommands {
    #[command(name = "diff-source")]
    DiffSource(DebugDiffSourceCommand),
}

#[derive(Debug, Parser)]
struct DebugDiffSourceCommand {
    #[arg(long)]
    diff_source: String,
}

#[derive(serde::Serialize)]
struct VersionJson<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(serde::Serialize)]
struct DebugDiffSourceJson<'a> {
    diff_source: &'a str,
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
        Commands::Debug(debug) => match debug.command {
            DebugCommands::DiffSource(cmd) => {
                discover_repo_root().map_err(|err| err.with_message("not a git repo"))?;

                let parsed = DiffSource::parse(&cmd.diff_source)
                    .map_err(|err| map_diff_source_parse_error(err, &cmd.diff_source))?;
                let diff_source = parsed.as_display();

                match cli.format {
                    OutputFormat::Json => write_json_stdout(&DebugDiffSourceJson {
                        diff_source: diff_source.as_str(),
                    })
                    .map_err(|err| KbError::internal(err, "failed to write json"))?,
                    OutputFormat::Text => println!("{diff_source}"),
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
