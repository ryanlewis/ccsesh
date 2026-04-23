use std::io::{self, Write};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use crossterm::{cursor, execute, terminal};

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
    about = "List and resume recent Claude Code sessions"
)]
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

    /// Watch mode: continuously refresh the session list
    #[arg(short, long)]
    watch: bool,

    /// Refresh interval in seconds for watch mode (default: 2)
    #[arg(short, long, default_value_t = 2)]
    interval: u64,
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

/// Format sessions for display (shared between normal and watch modes).
fn format_sessions(
    sessions: &[SessionInfo],
    format: &OutputFormat,
    json: bool,
    now: chrono::DateTime<Utc>,
) -> String {
    if json {
        display::format_json(sessions, now)
    } else {
        match format {
            OutputFormat::Short => display::format_short(sessions, now),
            OutputFormat::Default => display::format_default(sessions, now),
        }
    }
}

/// Run the watch loop: clear screen, display sessions, sleep, repeat.
fn run_watch(
    home_dir: &str,
    limit: usize,
    format: &OutputFormat,
    json: bool,
    interval: Duration,
    running: &AtomicBool,
) -> Result<()> {
    let mut stdout = io::stdout();

    while running.load(Ordering::Relaxed) {
        // Clear screen and move cursor to top-left
        execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        let now = Utc::now();

        match load_sessions(home_dir, limit) {
            Ok(sessions) => {
                let output = format_sessions(&sessions, format, json, now);
                print!("{}", output);
            }
            Err(_) => {
                println!("No sessions found.");
            }
        }

        // Show watch status line
        let timestamp = now.format("%Y-%m-%d %H:%M:%S");
        println!(
            "\nLast updated: {}  [watching... press Ctrl+C to exit]",
            timestamp
        );

        stdout.flush()?;

        // Sleep in small increments so we can check the running flag
        let sleep_ms = interval.as_millis() as u64;
        let chunk = 100; // check every 100ms
        let mut elapsed = 0u64;
        while elapsed < sleep_ms && running.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(chunk.min(sleep_ms - elapsed)));
            elapsed += chunk;
        }
    }

    Ok(())
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let home_dir = std::env::var("HOME").map_err(|_| CcseshError::HomeDirectoryNotFound)?;

    // Watch mode is only valid for listing (no command, no shell-mode)
    if cli.watch {
        if cli.command.is_some() {
            anyhow::bail!("--watch cannot be used with a session index or subcommand");
        }
        if cli.shell_mode.is_some() {
            anyhow::bail!("--watch cannot be used with --shell-mode");
        }

        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        // Register Ctrl+C handler
        ctrlc::set_handler(move || {
            r.store(false, Ordering::Relaxed);
        })?;

        let interval = Duration::from_secs(cli.interval.max(1));
        run_watch(
            &home_dir,
            cli.limit,
            &cli.format,
            cli.json,
            interval,
            &running,
        )?;

        return Ok(());
    }

    match cli.command.as_deref() {
        None => {
            if cli.shell_mode.is_some() {
                anyhow::bail!(
                    "--shell-mode requires a session index. Usage: ccsesh --shell-mode <shell> <index>"
                );
            }

            let sessions = load_sessions(&home_dir, cli.limit)?;

            let now = Utc::now();
            let output = format_sessions(&sessions, &cli.format, cli.json, now);
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

            let sessions = load_sessions(&home_dir, cli.limit)?;

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
