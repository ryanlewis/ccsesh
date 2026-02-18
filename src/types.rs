use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Cheap stat-only candidate before parsing
#[derive(Debug, Clone)]
pub struct SessionCandidate {
    pub path: PathBuf,
    pub mtime: SystemTime,
}

/// Fully parsed session metadata
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub path: PathBuf,
    pub project_dir: PathBuf,
    pub project_dir_display: String,
    pub last_active: DateTime<Utc>,
    pub first_prompt: Option<String>,
    pub slug: Option<String>,
}

/// Represents a single line in the JSONL file (loosely typed).
#[derive(Debug, Deserialize)]
pub struct JsonlLine {
    #[serde(rename = "type")]
    pub msg_type: Option<String>,
    pub cwd: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "isMeta")]
    pub is_meta: Option<bool>,
    #[serde(rename = "isCompactSummary")]
    pub is_compact_summary: Option<bool>,
    pub slug: Option<String>,
    #[serde(rename = "teamName")]
    pub team_name: Option<String>,
    #[serde(rename = "agentName")]
    pub agent_name: Option<String>,
    pub message: Option<JsonlMessage>,
}

#[derive(Debug, Deserialize)]
pub struct JsonlMessage {
    pub content: Option<serde_json::Value>,
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Default,
    Short,
}

/// Wraps a string in single quotes, escaping internal single quotes as `'\''`.
pub fn shell_escape_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}
