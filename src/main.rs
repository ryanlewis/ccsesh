use std::process;

use anyhow::Result;
use chrono::Utc;
use clap::Parser;

use ccsesh::config;
use ccsesh::discover;
use ccsesh::display;
use ccsesh::errors::CcseshError;
use ccsesh::parse;
use ccsesh::shell;
use ccsesh::types::{OutputFormat, SessionInfo};

#[derive(Parser)]
#[command(
    name = "ccsesh",
    version,
    about = "List and resume recent Claude Code sessions",
    after_help = "Config file: ~/.config/ccsesh/config.toml"
)]
struct Cli {
    /// Session index to resume, or "init" subcommand
    command: Option<String>,

    /// Shell type for init (fish, bash, zsh)
    shell: Option<String>,

    #[arg(short, long)]
    limit: Option<usize>,

    #[arg(long)]
    format: Option<OutputFormat>,

    #[arg(long)]
    json: bool,

    #[arg(long, hide = true)]
    shell_mode: Option<String>,
}

/// Discover, parse, and filter sessions. Returns up to `limit` valid sessions
/// (excludes team subagent sessions and empty sessions with no prompt or slug).
fn load_sessions(home_dir: &str, limit: usize) -> Result<Vec<SessionInfo>> {
    if limit == 0 {
        return Ok(vec![]);
    }

    // Over-discover to compensate for filtered subagent/empty sessions
    let discover_limit = (limit * 5).max(50);
    let candidates = discover::discover_sessions(home_dir, discover_limit)?;

    if candidates.is_empty() {
        return Err(CcseshError::NoSessionsFound.into());
    }

    let mut sessions = Vec::new();
    for candidate in &candidates {
        if sessions.len() >= limit {
            break;
        }
        match parse::parse_session(candidate, home_dir) {
            Ok(info) => {
                // Skip empty sessions (no prompt and no slug)
                if info.first_prompt.is_none() && info.slug.is_none() {
                    continue;
                }
                sessions.push(info);
            }
            Err(_) => continue, // Includes subagent sessions and parse errors
        }
    }

    if sessions.is_empty() {
        return Err(CcseshError::NoSessionsFound.into());
    }

    Ok(sessions)
}

fn run() -> Result<()> {
    let config = config::load_config();

    // Build effective args: prepend default_args from config before CLI args.
    // Skip argv[0] (the binary name), prepend config args, then append real args.
    let real_args: Vec<String> = std::env::args().collect();
    let mut effective_args = vec![real_args[0].clone()];
    if let Some(ref default_args) = config.default_args {
        effective_args.extend(default_args.iter().cloned());
    }
    effective_args.extend(real_args.into_iter().skip(1));

    let cli = Cli::parse_from(&effective_args);

    // Merge: CLI > config > defaults
    let limit = cli.limit.or(config.limit).unwrap_or(5);
    let format = cli.format.unwrap_or(OutputFormat::Default);

    // Apply color override from config (if set).
    // SAFETY: This runs single-threaded at startup before any other threads are spawned.
    if let Some(ref color_setting) = config.colors {
        match color_setting {
            config::ColorSetting::Always => {
                unsafe {
                    std::env::remove_var("NO_COLOR");
                    std::env::set_var("FORCE_COLOR", "1");
                }
            }
            config::ColorSetting::Never => {
                unsafe {
                    std::env::set_var("NO_COLOR", "1");
                }
            }
            config::ColorSetting::Auto => {}
        }
    }

    let home_dir = std::env::var("HOME").map_err(|_| CcseshError::HomeDirectoryNotFound)?;

    match cli.command.as_deref() {
        None => {
            if cli.shell_mode.is_some() {
                anyhow::bail!(
                    "--shell-mode requires a session index. Usage: ccsesh --shell-mode <shell> <index>"
                );
            }

            let sessions = load_sessions(&home_dir, limit)?;

            let now = Utc::now();
            let output = if cli.json {
                display::format_json(&sessions, now)
            } else {
                match format {
                    OutputFormat::Short => display::format_short(&sessions, now),
                    OutputFormat::Default => display::format_default(&sessions, now),
                }
            };

            print!("{}", output);
        }
        Some("init") => {
            let shell = cli
                .shell
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Usage: ccsesh init <fish|bash|zsh>"))?;
            shell::print_shell_init(shell)?;
        }
        Some(s) => {
            let index: usize = s.parse().map_err(|_| {
                anyhow::anyhow!(
                    "Unknown command '{}'. Usage: ccsesh [<index>|init <shell>]",
                    s
                )
            })?;

            let sessions = load_sessions(&home_dir, limit)?;

            if index >= sessions.len() {
                let max = sessions.len() - 1;
                return Err(CcseshError::IndexOutOfRange { index, max }.into());
            }

            let session = &sessions[index];

            if cli.shell_mode.is_some() {
                shell::print_exec_protocol(session)?;
            } else {
                shell::print_resume_instructions(session);
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
