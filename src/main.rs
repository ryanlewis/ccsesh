use std::process;

use anyhow::Result;
use chrono::Utc;
use clap::Parser;

use ccsesh::discover;
use ccsesh::display;
use ccsesh::errors::CcseshError;
use ccsesh::parse;
use ccsesh::shell;
use ccsesh::types::OutputFormat;

#[derive(Parser)]
#[command(name = "ccsesh", version, about = "List and resume recent Claude Code sessions")]
struct Cli {
    /// Session index to resume, or "init" subcommand
    command: Option<String>,

    /// Shell type for init (fish, bash, zsh)
    shell: Option<String>,

    #[arg(short, long, default_value_t = 5)]
    limit: usize,

    #[arg(long, default_value = "default")]
    format: OutputFormat,

    #[arg(long)]
    json: bool,

    #[arg(long, hide = true)]
    shell_mode: Option<String>,
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let home_dir = std::env::var("HOME")
        .map_err(|_| CcseshError::HomeDirectoryNotFound)?;

    match cli.command.as_deref() {
        None => {
            if cli.shell_mode.is_some() {
                anyhow::bail!("--shell-mode requires a session index. Usage: ccsesh --shell-mode <shell> <index>");
            }

            let candidates = discover::discover_sessions(&home_dir, cli.limit)?;

            if candidates.is_empty() && cli.limit > 0 {
                return Err(CcseshError::NoSessionsFound.into());
            }

            let mut sessions = Vec::new();
            for candidate in &candidates {
                match parse::parse_session(candidate, &home_dir) {
                    Ok(info) => sessions.push(info),
                    Err(_) => continue,
                }
            }

            if sessions.is_empty() && !candidates.is_empty() {
                return Err(CcseshError::NoSessionsFound.into());
            }

            let now = Utc::now();
            let output = if cli.json {
                display::format_json(&sessions, now)
            } else {
                match cli.format {
                    OutputFormat::Short => display::format_short(&sessions, now),
                    OutputFormat::Default => display::format_default(&sessions, now),
                }
            };

            print!("{}", output);
        }
        Some("init") => {
            let shell = cli.shell.as_deref()
                .ok_or_else(|| anyhow::anyhow!("Usage: ccsesh init <fish|bash|zsh>"))?;
            shell::print_shell_init(shell)?;
        }
        Some(s) => {
            let index: usize = s.parse()
                .map_err(|_| anyhow::anyhow!("Unknown command '{}'. Usage: ccsesh [<index>|init <shell>]", s))?;

            let candidates = discover::discover_sessions(&home_dir, cli.limit)?;

            if candidates.is_empty() && cli.limit > 0 {
                return Err(CcseshError::NoSessionsFound.into());
            }

            if index >= candidates.len() {
                let max = if candidates.is_empty() { 0 } else { candidates.len() - 1 };
                return Err(CcseshError::IndexOutOfRange { index, max }.into());
            }

            let session = parse::parse_session(&candidates[index], &home_dir)?;

            if cli.shell_mode.is_some() {
                shell::print_exec_protocol(&session)?;
            } else {
                shell::print_resume_instructions(&session);
            }
        }
    }

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", err);
        process::exit(1);
    }
}
