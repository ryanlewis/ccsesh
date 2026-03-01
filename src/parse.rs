use std::io::BufRead;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};

use crate::types::{JsonlLine, SessionCandidate, SessionInfo};

const MAX_LINES: usize = 50;

/// Parse a session JSONL file into a `SessionInfo` struct.
pub fn parse_session(candidate: &SessionCandidate, home_dir: &str) -> Result<SessionInfo> {
    let session_id = extract_session_id(&candidate.path)?;

    let file = std::fs::File::open(&candidate.path)?;
    let reader = std::io::BufReader::new(file);

    let mut cwd: Option<String> = None;
    let mut slug: Option<String> = None;
    let mut first_prompt: Option<String> = None;

    for line_result in reader.lines().take(MAX_LINES) {
        let line_str = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let parsed: JsonlLine = match serde_json::from_str(&line_str) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Bail on subagent sessions (spawned by Claude Code Teams)
        if parsed.agent_name.is_some() {
            return Err(anyhow!("Subagent session: {}", candidate.path.display()));
        }

        if cwd.is_none()
            && let Some(ref c) = parsed.cwd
            && !c.chars().any(|ch| ch.is_control())
        {
            cwd = Some(c.clone());
        }

        if slug.is_none()
            && let Some(ref s) = parsed.slug
        {
            slug = Some(s.clone());
        }

        if first_prompt.is_none()
            && let Some(prompt) = try_extract_prompt(&parsed)
        {
            first_prompt = Some(prompt);
        }

        if cwd.is_some() && slug.is_some() && first_prompt.is_some() {
            break;
        }
    }

    // Fall back to an empty PathBuf when cwd is absent or was rejected (e.g.
    // contained C0/C1 control characters or DEL). The session can still be
    // listed — it just cannot be meaningfully resumed via `cd`, and we prefer
    // that over crashing.
    let project_dir = cwd.map(PathBuf::from).unwrap_or_default();

    let project_dir_display = {
        let dir_str = project_dir.to_string_lossy();
        if dir_str.starts_with(home_dir) && !home_dir.is_empty() {
            format!("~{}", &dir_str[home_dir.len()..])
        } else {
            dir_str.into_owned()
        }
    };

    let last_active: DateTime<Utc> = DateTime::<Utc>::from(candidate.mtime);

    Ok(SessionInfo {
        session_id,
        path: candidate.path.clone(),
        project_dir,
        project_dir_display,
        last_active,
        first_prompt,
        slug,
    })
}

/// Extract text from a `serde_json::Value` that is either a string or an array
/// containing `{"type":"text","text":"..."}` items.
pub fn extract_text_from_content(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(obj) = item.as_object()
                    && obj.get("type").and_then(|v| v.as_str()) == Some("text")
                    && let Some(text) = obj.get("text").and_then(|v| v.as_str())
                {
                    return Some(text.to_string());
                }
            }
            None
        }
        _ => None,
    }
}

/// Strip XML-like tags from a string using a character scanner.
/// Bails out of tag-skipping if a newline is encountered before the closing `>`.
/// After stripping, collapses whitespace runs into single spaces and trims.
pub fn strip_xml_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            let mut skipped = Vec::new();
            let mut found_close = false;
            for inner in chars.by_ref() {
                if inner == '\n' {
                    // Bail out: `<` was literal content, not a tag
                    out.push('<');
                    for sc in &skipped {
                        out.push(*sc);
                    }
                    out.push('\n');
                    break;
                }
                if inner == '>' {
                    found_close = true;
                    break;
                }
                skipped.push(inner);
            }
            if !found_close && chars.peek().is_none() && !skipped.is_empty() {
                // Reached end of input without closing `>` and without newline bail-out
                out.push('<');
                for sc in &skipped {
                    out.push(*sc);
                }
            } else if !found_close && skipped.is_empty() && chars.peek().is_none() {
                // Lone `<` at end of input
                out.push('<');
            }
        } else {
            out.push(c);
        }
    }

    collapse_whitespace(&out)
}

fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut last_was_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !last_was_ws {
                result.push(' ');
            }
            last_was_ws = true;
        } else {
            result.push(c);
            last_was_ws = false;
        }
    }
    result.trim().to_string()
}

fn try_extract_prompt(line: &JsonlLine) -> Option<String> {
    if line.msg_type.as_deref() != Some("user") {
        return None;
    }

    if line.is_meta == Some(true) {
        return None;
    }

    if line.is_compact_summary == Some(true) {
        return None;
    }

    let content = line.message.as_ref()?.content.as_ref()?;
    let raw_text = extract_text_from_content(content)?;

    let stripped = strip_xml_tags(&raw_text);

    if stripped.is_empty() {
        return None;
    }

    // Slash command detection
    if stripped.starts_with('/')
        && stripped[1..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric())
    {
        return None;
    }

    Some(stripped)
}

fn extract_session_id(path: &std::path::Path) -> Result<String> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Invalid filename: {}", path.display()))?;

    if !is_valid_uuid(stem) {
        return Err(anyhow!("Filename is not a valid UUID: {}", path.display()));
    }

    Ok(stem.to_string())
}

pub(crate) fn is_valid_uuid(s: &str) -> bool {
    // [0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    let segments = [8, 4, 4, 4, 12];
    let mut pos = 0;
    for (i, &len) in segments.iter().enumerate() {
        if i > 0 {
            if bytes[pos] != b'-' {
                return false;
            }
            pos += 1;
        }
        for _ in 0..len {
            if pos >= bytes.len() {
                return false;
            }
            let b = bytes[pos];
            if !b.is_ascii_hexdigit() || b.is_ascii_uppercase() {
                return false;
            }
            pos += 1;
        }
    }
    pos == 36
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    const TEST_UUID: &str = "eb53d999-8692-42ce-a376-4f82206a086d";

    fn fixture_candidate(fixture_name: &str) -> SessionCandidate {
        // Copy fixture to a unique temp dir with a UUID filename so parse_session accepts it.
        // Uses thread ID to avoid races when cargo test runs tests in parallel.
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture_name);
        let tmp_dir = std::env::temp_dir().join(format!(
            "ccsesh_test_{}_{:?}",
            fixture_name,
            std::thread::current().id()
        ));
        let _ = std::fs::create_dir_all(&tmp_dir);
        let dest = tmp_dir.join(format!("{}.jsonl", TEST_UUID));
        std::fs::copy(&fixture_path, &dest).expect("failed to copy fixture");
        SessionCandidate {
            path: dest,
            mtime: SystemTime::now(),
        }
    }

    // ---- strip_xml_tags tests ----

    #[test]
    fn strip_xml_tags_removes_simple_tags() {
        assert_eq!(
            strip_xml_tags("<command-name>/commit</command-name>"),
            "/commit"
        );
    }

    #[test]
    fn strip_xml_tags_preserves_text_between_tags() {
        // Tags are stripped individually; text between open/close tags is preserved
        assert_eq!(
            strip_xml_tags("<system-reminder>context loaded</system-reminder> Fix the CSS"),
            "context loaded Fix the CSS"
        );
    }

    #[test]
    fn strip_xml_tags_newline_bailout() {
        // `<` followed by content then a newline before `>` → literal
        let input = "< 10 mins remaining\nPlease wrap up";
        let result = strip_xml_tags(input);
        assert!(result.contains("< 10 mins remaining"));
        assert!(result.contains("Please wrap up"));
    }

    #[test]
    fn strip_xml_tags_no_tags() {
        assert_eq!(strip_xml_tags("Hello world"), "Hello world");
    }

    #[test]
    fn strip_xml_tags_empty_input() {
        assert_eq!(strip_xml_tags(""), "");
    }

    #[test]
    fn strip_xml_tags_collapses_whitespace() {
        assert_eq!(
            strip_xml_tags("<tag>text</tag>   more   text"),
            "text more text"
        );
    }

    #[test]
    fn strip_xml_tags_lone_lt_at_end() {
        assert_eq!(strip_xml_tags("text <"), "text <");
    }

    #[test]
    fn strip_xml_tags_unclosed_tag_at_end() {
        assert_eq!(strip_xml_tags("text <unclosed"), "text <unclosed");
    }

    #[test]
    fn strip_xml_tags_multiple_tags() {
        assert_eq!(strip_xml_tags("<a>hello</a> <b>world</b>"), "hello world");
    }

    // ---- extract_text_from_content tests ----

    #[test]
    fn extract_text_string_value() {
        let val = serde_json::json!("hello world");
        assert_eq!(extract_text_from_content(&val), Some("hello world".into()));
    }

    #[test]
    fn extract_text_array_with_text_item() {
        let val = serde_json::json!([
            {"type": "tool_result", "content": "something"},
            {"type": "text", "text": "the prompt"}
        ]);
        assert_eq!(extract_text_from_content(&val), Some("the prompt".into()));
    }

    #[test]
    fn extract_text_array_no_text_item() {
        let val = serde_json::json!([
            {"type": "tool_result", "content": "something"},
            {"type": "image", "source": {"type": "base64"}}
        ]);
        assert_eq!(extract_text_from_content(&val), None);
    }

    #[test]
    fn extract_text_array_image_and_text() {
        let val = serde_json::json!([
            {"type": "image", "source": {"type": "base64", "data": "abc"}},
            {"type": "text", "text": "Describe this"}
        ]);
        assert_eq!(
            extract_text_from_content(&val),
            Some("Describe this".into())
        );
    }

    #[test]
    fn extract_text_null_value() {
        let val = serde_json::Value::Null;
        assert_eq!(extract_text_from_content(&val), None);
    }

    #[test]
    fn extract_text_number_value() {
        let val = serde_json::json!(42);
        assert_eq!(extract_text_from_content(&val), None);
    }

    // ---- is_valid_uuid tests ----

    #[test]
    fn valid_uuid() {
        assert!(is_valid_uuid("eb53d999-8692-42ce-a376-4f82206a086d"));
    }

    #[test]
    fn invalid_uuid_too_short() {
        assert!(!is_valid_uuid("eb53d999-8692-42ce-a376"));
    }

    #[test]
    fn invalid_uuid_uppercase() {
        assert!(!is_valid_uuid("EB53D999-8692-42CE-A376-4F82206A086D"));
    }

    #[test]
    fn invalid_uuid_bad_chars() {
        assert!(!is_valid_uuid("gb53d999-8692-42ce-a376-4f82206a086d"));
    }

    #[test]
    fn invalid_uuid_no_hyphens() {
        assert!(!is_valid_uuid("eb53d99986924f82206a086da376a376"));
    }

    // ---- try_extract_prompt tests ----

    #[test]
    fn prompt_from_normal_user_line() {
        let line: JsonlLine =
            serde_json::from_str(r#"{"type":"user","message":{"content":"Hello world"}}"#).unwrap();
        assert_eq!(try_extract_prompt(&line), Some("Hello world".into()));
    }

    #[test]
    fn prompt_skips_meta() {
        let line: JsonlLine = serde_json::from_str(
            r#"{"type":"user","isMeta":true,"message":{"content":"<caveat>stuff</caveat>"}}"#,
        )
        .unwrap();
        assert_eq!(try_extract_prompt(&line), None);
    }

    #[test]
    fn prompt_skips_compact_summary() {
        let line: JsonlLine = serde_json::from_str(
            r#"{"type":"user","isCompactSummary":true,"message":{"content":"This session is being continued..."}}"#,
        )
        .unwrap();
        assert_eq!(try_extract_prompt(&line), None);
    }

    #[test]
    fn prompt_skips_slash_command() {
        let line: JsonlLine =
            serde_json::from_str(r#"{"type":"user","message":{"content":"/clear"}}"#).unwrap();
        assert_eq!(try_extract_prompt(&line), None);
    }

    #[test]
    fn prompt_skips_assistant_lines() {
        let line: JsonlLine =
            serde_json::from_str(r#"{"type":"assistant","message":{"content":"Response text"}}"#)
                .unwrap();
        assert_eq!(try_extract_prompt(&line), None);
    }

    #[test]
    fn prompt_strips_xml_then_detects_slash_command() {
        let line: JsonlLine = serde_json::from_str(
            r#"{"type":"user","message":{"content":"<command-name>/add-dir</command-name>\n            /add-dir ../shared-lib"}}"#,
        )
        .unwrap();
        assert_eq!(try_extract_prompt(&line), None);
    }

    #[test]
    fn prompt_empty_after_xml_strip() {
        let line: JsonlLine =
            serde_json::from_str(r#"{"type":"user","message":{"content":"<tag>  </tag>"}}"#)
                .unwrap();
        assert_eq!(try_extract_prompt(&line), None);
    }

    #[test]
    fn prompt_from_array_content() {
        let line: JsonlLine = serde_json::from_str(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"x"},{"type":"text","text":"My prompt"}]}}"#,
        )
        .unwrap();
        assert_eq!(try_extract_prompt(&line), Some("My prompt".into()));
    }

    // ---- parse_session fixture tests ----

    #[test]
    fn parse_normal_session() {
        let candidate = fixture_candidate("normal.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(info.session_id, TEST_UUID);
        assert_eq!(
            info.project_dir,
            PathBuf::from("/Users/testuser/dev/myproject")
        );
        assert_eq!(info.project_dir_display, "~/dev/myproject");
        assert_eq!(info.slug.as_deref(), Some("woolly-conjuring-journal"));
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("Design technical approach for ccsesh")
        );
    }

    #[test]
    fn parse_meta_only_session() {
        let candidate = fixture_candidate("meta_only.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(info.first_prompt, None);
        assert_eq!(info.slug.as_deref(), Some("gentle-morning-breeze"));
    }

    #[test]
    fn parse_empty_session() {
        let candidate = fixture_candidate("empty.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(info.first_prompt, None);
        assert_eq!(info.slug, None);
        assert_eq!(info.project_dir, PathBuf::from(""));
    }

    #[test]
    fn parse_xml_markup_session() {
        let candidate = fixture_candidate("xml_markup.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        // Line 2: <command-name>/commit</command-name>\ncommit → "/commit commit" → slash command, skipped
        // Line 4: < 10 mins remaining\nPlease wrap up → newline bail-out, literal content
        // Line 5: <system-reminder>context loaded</system-reminder> Fix the login page CSS alignment issue → "Fix the login page CSS alignment issue"
        // But line 4 comes first. Let's check what it actually produces.
        // "< 10 mins remaining\nPlease wrap up the current task" → strip_xml_tags bails out → "< 10 mins remaining Please wrap up the current task" after whitespace collapse
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("< 10 mins remaining Please wrap up the current task")
        );
    }

    #[test]
    fn parse_slash_command_session() {
        let candidate = fixture_candidate("slash_command.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        // Slash commands are skipped; first real prompt is line 5
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("Refactor the argument parser to use clap derive macros")
        );
        assert_eq!(info.slug.as_deref(), Some("silver-winding-path"));
    }

    #[test]
    fn parse_array_content_session() {
        let candidate = fixture_candidate("array_content.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("Array content test prompt")
        );
        assert_eq!(info.slug.as_deref(), Some("quiet-autumn-leaf"));
    }

    #[test]
    fn parse_image_paste_session() {
        let candidate = fixture_candidate("image_paste.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(info.first_prompt.as_deref(), Some("Describe this image"));
    }

    #[test]
    fn parse_compact_summary_session() {
        let candidate = fixture_candidate("compact_summary.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        // isCompactSummary line is skipped; real prompt is on line 4
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("Add cursor-based pagination to the /users endpoint")
        );
        assert_eq!(info.slug.as_deref(), Some("warm-golden-sunset"));
    }

    #[test]
    fn parse_no_cwd_session() {
        let candidate = fixture_candidate("no_cwd.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(info.project_dir, PathBuf::from(""));
        assert_eq!(info.project_dir_display, "");
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("Explain how async/await works in Rust")
        );
    }

    #[test]
    fn parse_summary_only_session() {
        let candidate = fixture_candidate("summary_only.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(info.first_prompt, None);
        assert_eq!(info.slug, None);
    }

    #[test]
    fn parse_truncated_session() {
        let candidate = fixture_candidate("truncated.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        // Truncated last line is skipped gracefully; should still get prompt from valid lines
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("Optimize the database query performance")
        );
        assert_eq!(info.slug.as_deref(), Some("swift-running-river"));
    }

    #[test]
    fn parse_subagent_session_returns_err() {
        let candidate = fixture_candidate("team_subagent.jsonl");
        assert!(parse_session(&candidate, "/Users/testuser").is_err());
    }

    #[test]
    fn parse_nonexistent_file_returns_err() {
        let candidate = SessionCandidate {
            path: PathBuf::from("/tmp/does-not-exist/eb53d999-8692-42ce-a376-4f82206a086d.jsonl"),
            mtime: SystemTime::now(),
        };
        assert!(parse_session(&candidate, "/Users/testuser").is_err());
    }

    #[test]
    fn parse_invalid_uuid_filename_returns_err() {
        // Create a real file but with a non-UUID name
        let tmp = std::env::temp_dir().join("ccsesh_test_baduuid");
        let _ = std::fs::create_dir_all(&tmp);
        let path = tmp.join("not-a-uuid.jsonl");
        std::fs::write(&path, r#"{"type":"user","message":{"content":"test"}}"#).unwrap();
        let candidate = SessionCandidate {
            path,
            mtime: SystemTime::now(),
        };
        assert!(parse_session(&candidate, "/Users/testuser").is_err());
    }

    #[test]
    fn parse_home_dir_substitution() {
        let candidate = fixture_candidate("normal.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(info.project_dir_display, "~/dev/myproject");

        // With a different home dir that doesn't match
        let info2 = parse_session(&candidate, "/home/other").unwrap();
        assert_eq!(info2.project_dir_display, "/Users/testuser/dev/myproject");
    }

    #[test]
    fn parse_slug_from_first_occurrence() {
        // In normal.jsonl, slug appears first on line 3 (assistant line)
        let candidate = fixture_candidate("normal.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(info.slug.as_deref(), Some("woolly-conjuring-journal"));
    }

    #[test]
    fn summary_lines_dont_affect_prompt() {
        // normal.jsonl has a summary line at line 4 (mid-file)
        // It should not interfere with prompt extraction
        let candidate = fixture_candidate("normal.jsonl");
        let info = parse_session(&candidate, "/Users/testuser").unwrap();
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("Design technical approach for ccsesh")
        );
    }

    #[test]
    fn parse_cwd_with_newline_is_rejected() {
        let candidate = fixture_candidate("newline_cwd.jsonl");
        let info = parse_session(&candidate, "/tmp").unwrap();
        // cwd containing \n should be rejected, falling back to empty
        assert_eq!(info.project_dir, PathBuf::from(""));
        // The prompt should still be extracted
        assert_eq!(
            info.first_prompt.as_deref(),
            Some("Test prompt with newline cwd")
        );
    }

    #[test]
    fn parse_malformed_json_lines_skipped() {
        let tmp = std::env::temp_dir().join("ccsesh_test_malformed");
        let _ = std::fs::create_dir_all(&tmp);
        let path = tmp.join(format!("{}.jsonl", TEST_UUID));
        std::fs::write(
            &path,
            "this is not json\n{\"type\":\"user\",\"cwd\":\"/tmp/proj\",\"message\":{\"content\":\"Valid prompt\"}}\n{broken json\n",
        )
        .unwrap();
        let candidate = SessionCandidate {
            path,
            mtime: SystemTime::now(),
        };
        let info = parse_session(&candidate, "/tmp").unwrap();
        assert_eq!(info.first_prompt.as_deref(), Some("Valid prompt"));
        assert_eq!(info.project_dir, PathBuf::from("/tmp/proj"));
    }
}
