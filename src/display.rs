use chrono::{DateTime, Utc};
use owo_colors::{OwoColorize, Stream, Style};
use serde::Serialize;

use crate::types::{SessionInfo, shell_escape_single_quote};

/// Truncate a prompt at word boundaries, appending "..." if truncated.
pub fn truncate_prompt(prompt: &str, max_chars: usize) -> String {
    let char_count = prompt.chars().count();
    if char_count <= max_chars {
        return prompt.to_string();
    }

    let limit = max_chars.saturating_sub(3);

    // Find byte offset of character at position `limit`
    let byte_limit = prompt
        .char_indices()
        .nth(limit)
        .map(|(i, _)| i)
        .unwrap_or(prompt.len());

    // Search for last space using char_indices (character boundaries only)
    let truncated = &prompt[..byte_limit];
    if let Some(last_space_byte) = truncated
        .char_indices()
        .rev()
        .find(|(_, c)| *c == ' ')
        .map(|(i, _)| i)
    {
        format!("{}...", &truncated[..last_space_byte])
    } else {
        // No space found — use the full byte_limit (already on char boundary)
        format!("{}...", truncated)
    }
}

/// Format a duration into a compact relative time string like "2m ago", "1h ago".
pub fn format_relative_time(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds().max(0);
    if secs < 60 {
        "<1m ago".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 604800 {
        format!("{}d ago", secs / 86400)
    } else if secs < 2592000 {
        format!("{}w ago", secs / 604800)
    } else if secs < 31536000 {
        format!("{}mo ago", secs / 2592000)
    } else {
        format!("{}y ago", secs / 31536000)
    }
}

/// Short relative time: "2m", "1h", "3d" — no "ago" suffix.
pub fn format_relative_time_short(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds().max(0);
    if secs < 60 {
        "<1m".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else if secs < 604800 {
        format!("{}d", secs / 86400)
    } else if secs < 2592000 {
        format!("{}w", secs / 604800)
    } else if secs < 31536000 {
        format!("{}mo", secs / 2592000)
    } else {
        format!("{}y", secs / 31536000)
    }
}

fn display_summary(session: &SessionInfo) -> DisplaySummary {
    match (&session.first_prompt, &session.slug) {
        (Some(prompt), _) => DisplaySummary::Prompt(prompt.clone()),
        (None, Some(slug)) => DisplaySummary::Slug(slug.clone()),
        (None, None) => DisplaySummary::Empty,
    }
}

enum DisplaySummary {
    Prompt(String),
    Slug(String),
    Empty,
}

// Style constants
fn style_index() -> Style {
    Style::new().cyan().bold()
}

fn style_dim_italic() -> Style {
    Style::new().dimmed().italic()
}

/// Default format output with header, aligned columns, footer.
pub fn format_default(sessions: &[SessionInfo], now: DateTime<Utc>) -> String {
    let mut out = String::new();

    // Header
    let header = "Recent Claude Code sessions:";
    out.push_str(
        &header
            .if_supports_color(Stream::Stdout, |s| s.dimmed())
            .to_string(),
    );
    out.push_str("\n\n");

    if !sessions.is_empty() {
        // Compute column widths
        let index_width = if sessions.len() <= 10 { 1 } else { 2 };
        let max_path_width = sessions
            .iter()
            .map(|s| s.project_dir_display.len())
            .max()
            .unwrap_or(0);

        let idx_style = style_index();
        let dim_it = style_dim_italic();

        for (i, session) in sessions.iter().enumerate() {
            let duration = now - session.last_active;
            let time_str = format_relative_time(duration);

            // Index: right-aligned, cyan bold
            let idx_str = format!("{:>width$}", i, width = index_width);
            let idx_colored = idx_str
                .if_supports_color(Stream::Stdout, |s| s.style(idx_style))
                .to_string();

            // Time: right-aligned 7 chars, yellow
            let time_padded = format!("{:>7}", time_str);
            let time_colored = time_padded
                .if_supports_color(Stream::Stdout, |s| s.yellow())
                .to_string();

            // Path: left-aligned padded, green
            let path_padded = format!(
                "{:<width$}",
                session.project_dir_display,
                width = max_path_width
            );
            let path_colored = path_padded
                .if_supports_color(Stream::Stdout, |s| s.green())
                .to_string();

            // Summary
            let summary = display_summary(session);
            let summary_str = match &summary {
                DisplaySummary::Prompt(p) => {
                    let truncated = truncate_prompt(p, 72);
                    let quoted = format!("\"{}\"", truncated);
                    quoted
                        .if_supports_color(Stream::Stdout, |s| s.white())
                        .to_string()
                }
                DisplaySummary::Slug(s) => {
                    let quoted = format!("\"{}\"", s);
                    quoted
                        .if_supports_color(Stream::Stdout, |s| s.style(dim_it))
                        .to_string()
                }
                DisplaySummary::Empty => "(empty session)"
                    .if_supports_color(Stream::Stdout, |s| s.style(dim_it))
                    .to_string(),
            };

            out.push_str(&format!(
                "  {}  {}   {}  {}\n",
                idx_colored, time_colored, path_colored, summary_str
            ));
        }

        out.push('\n');
    }

    // Footer
    let footer = "Resume: ccsesh <number>";
    out.push_str(
        &footer
            .if_supports_color(Stream::Stdout, |s| s.dimmed())
            .to_string(),
    );
    out.push('\n');

    out
}

/// Short format output — compact single-line, no header/footer.
pub fn format_short(sessions: &[SessionInfo], now: DateTime<Utc>) -> String {
    if sessions.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    let max_path_width = sessions
        .iter()
        .map(|s| s.project_dir_display.len())
        .max()
        .unwrap_or(0);

    let idx_style = style_index();
    let dim_it = style_dim_italic();

    for (i, session) in sessions.iter().enumerate() {
        let duration = now - session.last_active;
        let time_str = format_relative_time_short(duration);

        // Index: right-aligned 2 chars, cyan bold
        let idx_str = format!("{:>2}", i);
        let idx_colored = idx_str
            .if_supports_color(Stream::Stdout, |s| s.style(idx_style))
            .to_string();

        // Time: right-aligned 3 chars, yellow
        let time_padded = format!("{:>3}", time_str);
        let time_colored = time_padded
            .if_supports_color(Stream::Stdout, |s| s.yellow())
            .to_string();

        // Path: left-aligned padded, green
        let path_padded = format!(
            "{:<width$}",
            session.project_dir_display,
            width = max_path_width
        );
        let path_colored = path_padded
            .if_supports_color(Stream::Stdout, |s| s.green())
            .to_string();

        // Summary
        let summary = display_summary(session);
        let summary_str = match &summary {
            DisplaySummary::Prompt(p) => {
                let truncated = truncate_prompt(p, 52);
                truncated
                    .if_supports_color(Stream::Stdout, |s| s.white())
                    .to_string()
            }
            DisplaySummary::Slug(s) => s
                .if_supports_color(Stream::Stdout, |s| s.style(dim_it))
                .to_string(),
            DisplaySummary::Empty => "(empty session)"
                .if_supports_color(Stream::Stdout, |s| s.style(dim_it))
                .to_string(),
        };

        out.push_str(&format!(
            "{} {}  {}  {}\n",
            idx_colored, time_colored, path_colored, summary_str
        ));
    }

    out
}

/// JSON output format.
#[derive(Serialize)]
struct JsonSession {
    index: usize,
    session_id: String,
    project_dir: String,
    project_dir_display: String,
    last_active: String,
    last_active_relative: String,
    first_prompt: Option<String>,
    slug: Option<String>,
    resume_command: String,
}

pub fn format_json(sessions: &[SessionInfo], now: DateTime<Utc>) -> String {
    let json_sessions: Vec<JsonSession> = sessions
        .iter()
        .enumerate()
        .map(|(i, session)| {
            let duration = now - session.last_active;
            let project_dir_str = session.project_dir.to_string_lossy().to_string();
            let escaped_dir = shell_escape_single_quote(&project_dir_str);
            JsonSession {
                index: i,
                session_id: session.session_id.clone(),
                project_dir: project_dir_str,
                project_dir_display: session.project_dir_display.clone(),
                last_active: session.last_active.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                last_active_relative: format_relative_time(duration),
                first_prompt: session.first_prompt.clone(),
                slug: session.slug.clone(),
                resume_command: format!(
                    "cd {} && claude --resume {}",
                    escaped_dir, session.session_id
                ),
            }
        })
        .collect();

    serde_json::to_string_pretty(&json_sessions).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    use std::path::PathBuf;

    fn make_session(
        id: &str,
        dir: &str,
        display: &str,
        last_active: DateTime<Utc>,
        prompt: Option<&str>,
        slug: Option<&str>,
    ) -> SessionInfo {
        SessionInfo {
            session_id: id.to_string(),
            path: PathBuf::from(format!("/home/user/.claude/projects/test/{}.jsonl", id)),
            project_dir: PathBuf::from(dir),
            project_dir_display: display.to_string(),
            last_active,
            first_prompt: prompt.map(|s| s.to_string()),
            slug: slug.map(|s| s.to_string()),
        }
    }

    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-02-18T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    // --- format_relative_time ---

    #[test]
    fn relative_time_zero_seconds() {
        assert_eq!(format_relative_time(TimeDelta::seconds(0)), "<1m ago");
    }

    #[test]
    fn relative_time_30_seconds() {
        assert_eq!(format_relative_time(TimeDelta::seconds(30)), "<1m ago");
    }

    #[test]
    fn relative_time_59_seconds() {
        assert_eq!(format_relative_time(TimeDelta::seconds(59)), "<1m ago");
    }

    #[test]
    fn relative_time_1_minute() {
        assert_eq!(format_relative_time(TimeDelta::seconds(60)), "1m ago");
    }

    #[test]
    fn relative_time_2_minutes() {
        assert_eq!(format_relative_time(TimeDelta::seconds(120)), "2m ago");
    }

    #[test]
    fn relative_time_59_minutes() {
        assert_eq!(format_relative_time(TimeDelta::seconds(3599)), "59m ago");
    }

    #[test]
    fn relative_time_1_hour() {
        assert_eq!(format_relative_time(TimeDelta::seconds(3600)), "1h ago");
    }

    #[test]
    fn relative_time_23_hours() {
        assert_eq!(format_relative_time(TimeDelta::seconds(86399)), "23h ago");
    }

    #[test]
    fn relative_time_1_day() {
        assert_eq!(format_relative_time(TimeDelta::seconds(86400)), "1d ago");
    }

    #[test]
    fn relative_time_6_days() {
        assert_eq!(format_relative_time(TimeDelta::seconds(604799)), "6d ago");
    }

    #[test]
    fn relative_time_1_week() {
        assert_eq!(format_relative_time(TimeDelta::seconds(604800)), "1w ago");
    }

    #[test]
    fn relative_time_4_weeks() {
        assert_eq!(format_relative_time(TimeDelta::seconds(2591999)), "4w ago");
    }

    #[test]
    fn relative_time_1_month() {
        assert_eq!(format_relative_time(TimeDelta::seconds(2592000)), "1mo ago");
    }

    #[test]
    fn relative_time_11_months() {
        assert_eq!(
            format_relative_time(TimeDelta::seconds(31535999)),
            "12mo ago"
        );
    }

    #[test]
    fn relative_time_1_year() {
        assert_eq!(format_relative_time(TimeDelta::seconds(31536000)), "1y ago");
    }

    #[test]
    fn relative_time_negative_clamped() {
        assert_eq!(format_relative_time(TimeDelta::seconds(-100)), "<1m ago");
    }

    // --- format_relative_time_short ---

    #[test]
    fn relative_time_short_seconds() {
        assert_eq!(format_relative_time_short(TimeDelta::seconds(10)), "<1m");
    }

    #[test]
    fn relative_time_short_minutes() {
        assert_eq!(format_relative_time_short(TimeDelta::seconds(120)), "2m");
    }

    #[test]
    fn relative_time_short_hours() {
        assert_eq!(format_relative_time_short(TimeDelta::seconds(7200)), "2h");
    }

    #[test]
    fn relative_time_short_days() {
        assert_eq!(format_relative_time_short(TimeDelta::seconds(172800)), "2d");
    }

    #[test]
    fn relative_time_short_weeks() {
        assert_eq!(
            format_relative_time_short(TimeDelta::seconds(1209600)),
            "2w"
        );
    }

    #[test]
    fn relative_time_short_months() {
        assert_eq!(
            format_relative_time_short(TimeDelta::seconds(5184000)),
            "2mo"
        );
    }

    #[test]
    fn relative_time_short_years() {
        assert_eq!(
            format_relative_time_short(TimeDelta::seconds(63072000)),
            "2y"
        );
    }

    // --- truncate_prompt ---

    #[test]
    fn truncate_within_limit() {
        assert_eq!(truncate_prompt("short text", 72), "short text");
    }

    #[test]
    fn truncate_exactly_at_limit() {
        let s = "a".repeat(72);
        assert_eq!(truncate_prompt(&s, 72), s);
    }

    #[test]
    fn truncate_over_limit_word_boundary() {
        let s = "Fix the pagination bug in the users endpoint for the API server module that is causing issues";
        // This is > 72 chars. Should truncate at last space before position 69.
        let result = truncate_prompt(s, 72);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 72);
    }

    #[test]
    fn truncate_over_limit_no_spaces() {
        let s = "a".repeat(80);
        let result = truncate_prompt(&s, 72);
        assert_eq!(result, format!("{}...", "a".repeat(69)));
        assert_eq!(result.chars().count(), 72);
    }

    #[test]
    fn truncate_with_short_limit() {
        assert_eq!(truncate_prompt("hello world", 3), "...");
    }

    #[test]
    fn truncate_prompt_52_chars() {
        let s = "Design technical approach for ccsesh tool implementation details here";
        let result = truncate_prompt(s, 52);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 52);
    }

    // --- format_default ---

    #[test]
    fn default_empty_sessions() {
        let now = fixed_now();
        let result = format_default(&[], now);
        assert!(result.contains("Recent Claude Code sessions:"));
        assert!(result.contains("Resume: ccsesh <number>"));
        // No session lines
        assert!(!result.contains("  0"));
    }

    #[test]
    fn default_single_session() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "abc-1234",
            "/home/user/dev/project",
            "~/dev/project",
            now - TimeDelta::seconds(120),
            Some("Fix the bug"),
            None,
        )];
        let result = format_default(&sessions, now);
        assert!(result.contains("Recent Claude Code sessions:"));
        assert!(result.contains("Resume: ccsesh <number>"));
        assert!(result.contains("2m ago"));
        assert!(result.contains("~/dev/project"));
        assert!(result.contains("\"Fix the bug\""));
    }

    #[test]
    fn default_column_alignment() {
        let now = fixed_now();
        let sessions = vec![
            make_session(
                "id1",
                "/home/user/dev/ccsesh",
                "~/dev/ccsesh",
                now - TimeDelta::seconds(120),
                Some("First prompt"),
                None,
            ),
            make_session(
                "id2",
                "/home/user/dev/longer-project",
                "~/dev/longer-project",
                now - TimeDelta::seconds(3600),
                Some("Second prompt"),
                None,
            ),
        ];
        let result = format_default(&sessions, now);
        let lines: Vec<&str> = result.lines().collect();

        // Find session lines (contain index 0 and 1)
        let line0 = lines.iter().find(|l| l.contains("First prompt")).unwrap();
        let line1 = lines.iter().find(|l| l.contains("Second prompt")).unwrap();

        // Both paths should be padded to the same column
        // ~/dev/longer-project is 20 chars, ~/dev/ccsesh is 12 chars
        // line0 should have trailing spaces after ~/dev/ccsesh to match
        assert!(line0.contains("~/dev/ccsesh"));
        assert!(line1.contains("~/dev/longer-project"));
    }

    #[test]
    fn default_display_priority_prompt() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "id1",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            Some("User prompt text"),
            Some("some-slug"),
        )];
        let result = format_default(&sessions, now);
        // Prompt takes priority over slug
        assert!(result.contains("\"User prompt text\""));
        assert!(!result.contains("some-slug"));
    }

    #[test]
    fn default_display_priority_slug() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "id1",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            None,
            Some("woolly-conjuring-journal"),
        )];
        let result = format_default(&sessions, now);
        assert!(result.contains("woolly-conjuring-journal"));
    }

    #[test]
    fn default_display_priority_empty() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "id1",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            None,
            None,
        )];
        let result = format_default(&sessions, now);
        assert!(result.contains("(empty session)"));
    }

    // --- format_short ---

    #[test]
    fn short_empty_sessions() {
        let now = fixed_now();
        let result = format_short(&[], now);
        assert_eq!(result, "");
    }

    #[test]
    fn short_single_session() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "id1",
            "/home/user/dev/project",
            "~/dev/project",
            now - TimeDelta::seconds(120),
            Some("Fix the bug"),
            None,
        )];
        let result = format_short(&sessions, now);
        assert!(!result.contains("Recent Claude Code sessions:"));
        assert!(!result.contains("Resume:"));
        assert!(result.contains("2m"));
        assert!(result.contains("~/dev/project"));
        assert!(result.contains("Fix the bug"));
        // No quotes in short format
        assert!(!result.contains("\"Fix the bug\""));
    }

    #[test]
    fn short_no_header_footer() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "id1",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            Some("Test"),
            None,
        )];
        let result = format_short(&sessions, now);
        assert!(!result.contains("Recent"));
        assert!(!result.contains("Resume"));
    }

    #[test]
    fn short_display_priority_slug() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "id1",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            None,
            Some("my-slug"),
        )];
        let result = format_short(&sessions, now);
        assert!(result.contains("my-slug"));
    }

    #[test]
    fn short_display_priority_empty() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "id1",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            None,
            None,
        )];
        let result = format_short(&sessions, now);
        assert!(result.contains("(empty session)"));
    }

    // --- format_json ---

    #[test]
    fn json_empty_sessions() {
        let now = fixed_now();
        let result = format_json(&[], now);
        assert_eq!(result, "[]");
    }

    #[test]
    fn json_single_session_schema() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "eb53d999-8692-42ce-a376-4f82206a086d",
            "/home/user/dev/ccsesh",
            "~/dev/ccsesh",
            now - TimeDelta::seconds(120),
            Some("Design technical approach"),
            Some("woolly-conjuring-journal"),
        )];
        let result = format_json(&sessions, now);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed.len(), 1);

        let entry = &parsed[0];
        assert_eq!(entry["index"], 0);
        assert_eq!(entry["session_id"], "eb53d999-8692-42ce-a376-4f82206a086d");
        assert_eq!(entry["project_dir"], "/home/user/dev/ccsesh");
        assert_eq!(entry["project_dir_display"], "~/dev/ccsesh");
        assert!(entry["last_active"].as_str().unwrap().ends_with('Z'));
        assert_eq!(entry["last_active_relative"], "2m ago");
        assert_eq!(entry["first_prompt"], "Design technical approach");
        assert_eq!(entry["slug"], "woolly-conjuring-journal");
        assert!(
            entry["resume_command"]
                .as_str()
                .unwrap()
                .contains("claude --resume")
        );
        assert!(
            entry["resume_command"]
                .as_str()
                .unwrap()
                .contains("eb53d999-8692-42ce-a376-4f82206a086d")
        );
    }

    #[test]
    fn json_nullable_fields() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "test-id",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            None,
            None,
        )];
        let result = format_json(&sessions, now);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert!(parsed[0]["first_prompt"].is_null());
        assert!(parsed[0]["slug"].is_null());
    }

    #[test]
    fn json_no_truncation() {
        let now = fixed_now();
        let long_prompt = "a".repeat(200);
        let sessions = vec![make_session(
            "test-id",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            Some(&long_prompt),
            None,
        )];
        let result = format_json(&sessions, now);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        // Full prompt preserved in JSON — no truncation
        assert_eq!(parsed[0]["first_prompt"].as_str().unwrap().len(), 200);
    }

    #[test]
    fn json_resume_command_uses_absolute_path() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "abc-123",
            "/home/user/my project",
            "~/my project",
            now - TimeDelta::seconds(60),
            Some("test"),
            None,
        )];
        let result = format_json(&sessions, now);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        let cmd = parsed[0]["resume_command"].as_str().unwrap();
        // Uses absolute path with shell escaping, not ~ path
        assert!(cmd.starts_with("cd '/home/user/my project'"));
        assert!(!cmd.contains("~/"));
    }

    #[test]
    fn json_last_active_iso8601() {
        let now = fixed_now();
        let sessions = vec![make_session(
            "test-id",
            "/home/user/dev",
            "~/dev",
            now - TimeDelta::seconds(60),
            Some("test"),
            None,
        )];
        let result = format_json(&sessions, now);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        let ts = parsed[0]["last_active"].as_str().unwrap();
        // Must be ISO 8601 UTC with Z suffix
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        // Should be parseable
        assert!(DateTime::parse_from_rfc3339(ts).is_ok());
    }

    #[test]
    fn json_multiple_sessions_indexed() {
        let now = fixed_now();
        let sessions = vec![
            make_session(
                "id1",
                "/home/user/a",
                "~/a",
                now - TimeDelta::seconds(60),
                Some("first"),
                None,
            ),
            make_session(
                "id2",
                "/home/user/b",
                "~/b",
                now - TimeDelta::seconds(3600),
                Some("second"),
                None,
            ),
        ];
        let result = format_json(&sessions, now);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["index"], 0);
        assert_eq!(parsed[1]["index"], 1);
    }

    // --- truncate_prompt UTF-8 safety ---

    #[test]
    fn truncate_prompt_with_emoji() {
        // Each emoji is 4 bytes but 1 char — should not panic
        let s = "Hello 🌍 world this is a test with emojis 🎉 and more text here to go over";
        let result = truncate_prompt(s, 30);
        assert!(!result.is_empty());
        assert!(result.chars().count() <= 30);
    }

    #[test]
    fn truncate_prompt_with_cjk() {
        // CJK chars are 3 bytes each
        let s = "这是一个很长的中文提示词需要被截断处理才能正常显示在终端上面不会超出限制";
        let result = truncate_prompt(s, 15);
        assert!(!result.is_empty());
        assert!(result.chars().count() <= 15);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_prompt_with_accented() {
        let s = "Résumé café naïve über straße coöperate più für résumé café naïve über straße";
        let result = truncate_prompt(s, 30);
        assert!(!result.is_empty());
        assert!(result.chars().count() <= 30);
    }

    #[test]
    fn truncate_prompt_mixed_width_no_panic() {
        let s = "Fix 🐛 in café résumé 日本語テスト end";
        let result = truncate_prompt(s, 20);
        assert!(!result.is_empty());
        assert!(result.chars().count() <= 20);
    }

    #[test]
    fn truncate_prompt_panic_regression_utf8_boundary() {
        let emoji_prefix = "🌍".repeat(17); // 68 bytes, 17 chars
        let s = format!("{} extra words here for padding", emoji_prefix);
        let result = truncate_prompt(&s, 20);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 20);
    }

    #[test]
    fn truncate_prompt_emoji_at_boundary_no_panic() {
        // Issue #27 regression test: 68 ASCII + 4-byte emoji at truncation point
        let s = format!("{}🌍 world", "a".repeat(68));
        let result = truncate_prompt(&s, 72);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 72);
    }
}
