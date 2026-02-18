use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum CcseshError {
    #[error("Could not determine home directory. Is $HOME set?")]
    HomeDirectoryNotFound,

    #[error("No Claude Code session directory found at {path}. Is Claude Code installed?")]
    ProjectsDirNotFound { path: PathBuf },

    #[error("No Claude Code sessions found at ~/.claude/projects/")]
    NoSessionsFound,

    #[error("Session index {index} is out of range (0\u{2013}{max})")]
    IndexOutOfRange { index: usize, max: usize },

    #[error("Failed to read session file {path}: {source}")]
    SessionReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse session data in {path}: {detail}")]
    SessionParseError { path: PathBuf, detail: String },

    #[error("Unknown shell '{shell}'. Supported: fish, bash, zsh")]
    UnknownShell { shell: String },
}
