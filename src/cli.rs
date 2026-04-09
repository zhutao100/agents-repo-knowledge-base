use clap::{Parser, Subcommand, ValueEnum};

use crate::error::KbError;
use crate::index::{index_check, index_regen, IndexScope};
use crate::io::json::write_json_stdout;
use crate::policy::lint::lint_all;
use crate::policy::obligations::obligations_check;
use crate::query::pack::{
    pack_diff, pack_diff_text, pack_selectors, pack_selectors_text, SelectorInputs,
};
use crate::query::plan::{plan_diff, plan_diff_text, Policy};
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
    Plan(PlanCommand),
    Pack(PackCommand),
    Lint(LintCommand),
    Obligations(ObligationsCommand),
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
struct PlanCommand {
    #[command(subcommand)]
    command: PlanCommands,
}

#[derive(Debug, Subcommand)]
enum PlanCommands {
    Diff(PlanDiffCommand),
}

#[derive(Debug, Parser)]
struct PlanDiffCommand {
    #[arg(long)]
    diff_source: String,

    #[arg(long, value_enum, default_value_t = Policy::Default)]
    policy: Policy,
}

#[derive(Debug, Parser)]
struct PackCommand {
    #[command(subcommand)]
    command: PackCommands,
}

#[derive(Debug, Subcommand)]
enum PackCommands {
    Diff(PackDiffCommand),
    Selectors(PackSelectorsCommand),
}

#[derive(Debug, Parser)]
struct PackDiffCommand {
    #[arg(long)]
    diff_source: String,

    #[arg(long, default_value_t = 0)]
    radius: u32,

    #[arg(long)]
    max_bytes: u64,

    #[arg(long)]
    snippet_lines: u64,
}

#[derive(Debug, Parser)]
struct PackSelectorsCommand {
    #[arg(long = "path")]
    paths: Vec<String>,

    #[arg(long = "module")]
    modules: Vec<String>,

    #[arg(long = "symbol")]
    symbols: Vec<String>,

    #[arg(long = "fact")]
    facts: Vec<String>,

    #[arg(long)]
    max_bytes: u64,

    #[arg(long)]
    snippet_lines: u64,
}

#[derive(Debug, Parser)]
struct LintCommand {
    #[command(subcommand)]
    command: LintCommands,
}

#[derive(Debug, Subcommand)]
enum LintCommands {
    All,
}

#[derive(Debug, Parser)]
struct ObligationsCommand {
    #[command(subcommand)]
    command: ObligationsCommands,
}

#[derive(Debug, Subcommand)]
enum ObligationsCommands {
    Check(ObligationsCheckCommand),
}

#[derive(Debug, Parser)]
struct ObligationsCheckCommand {
    #[arg(long)]
    diff_source: String,
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
        Commands::Plan(plan) => match plan.command {
            PlanCommands::Diff(cmd) => {
                let diff_source = parse_diff_source(&cmd.diff_source)?;
                let out = plan_diff(&diff_source, cmd.policy)?;
                match cli.format {
                    OutputFormat::Json => write_json_stdout(&out)
                        .map_err(|err| KbError::internal(err, "failed to write json"))?,
                    OutputFormat::Text => println!("{}", plan_diff_text(&out)),
                }
            }
        },
        Commands::Pack(pack) => match pack.command {
            PackCommands::Diff(cmd) => {
                let diff_source = parse_diff_source(&cmd.diff_source)?;
                let out = pack_diff(&diff_source, cmd.radius, cmd.max_bytes, cmd.snippet_lines)?;
                match cli.format {
                    OutputFormat::Json => write_json_stdout(&out)
                        .map_err(|err| KbError::internal(err, "failed to write json"))?,
                    OutputFormat::Text => println!("{}", pack_diff_text(&out)),
                }
            }
            PackCommands::Selectors(cmd) => {
                let selectors = SelectorInputs {
                    paths: cmd.paths,
                    modules: cmd.modules,
                    symbols: cmd.symbols,
                    facts: cmd.facts,
                };
                let out = pack_selectors(&selectors, cmd.max_bytes, cmd.snippet_lines)?;
                match cli.format {
                    OutputFormat::Json => write_json_stdout(&out)
                        .map_err(|err| KbError::internal(err, "failed to write json"))?,
                    OutputFormat::Text => println!("{}", pack_selectors_text(&out)),
                }
            }
        },
        Commands::Lint(cmd) => match cmd.command {
            LintCommands::All => {
                lint_all()?;
                match cli.format {
                    OutputFormat::Json => write_json_stdout(&OkJson { ok: true })
                        .map_err(|err| KbError::internal(err, "failed to write json"))?,
                    OutputFormat::Text => println!("ok"),
                }
            }
        },
        Commands::Obligations(cmd) => match cmd.command {
            ObligationsCommands::Check(cmd) => {
                let diff_source = parse_diff_source(&cmd.diff_source)?;
                obligations_check(&diff_source)?;
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
