use crate::errors::CcseshError;
use crate::types::{SessionInfo, shell_escape_single_quote};

/// Outputs the shell wrapper function for the given shell type.
pub fn print_shell_init(shell: &str) -> anyhow::Result<()> {
    match shell {
        "fish" => print!("{}", FISH_TEMPLATE),
        "bash" => print!("{}", BASH_TEMPLATE),
        "zsh" => print!("{}", ZSH_TEMPLATE),
        _ => {
            return Err(CcseshError::UnknownShell {
                shell: shell.to_string(),
            }
            .into());
        }
    }
    Ok(())
}

/// Outputs the __CCSESH_EXEC__ protocol for shell wrapper eval.
pub fn print_exec_protocol(session: &SessionInfo) -> anyhow::Result<()> {
    if !is_valid_uuid(&session.session_id) {
        anyhow::bail!("Invalid session ID: {}", session.session_id);
    }
    let escaped_dir = shell_escape_single_quote(&session.project_dir.to_string_lossy());
    println!("__CCSESH_EXEC__");
    println!(
        "cd {} && claude --resume {}",
        escaped_dir, session.session_id
    );
    Ok(())
}

/// Formats human-readable resume instructions as a string.
///
/// Uses `session.project_dir` (the full path) rather than `project_dir_display`
/// because tilde expansion does not occur inside single-quoted strings.
pub fn format_resume_instructions(session: &SessionInfo) -> String {
    let escaped_dir = shell_escape_single_quote(&session.project_dir.to_string_lossy());
    format!(
        "To resume this session, run:\n  cd {} && claude --resume {}",
        escaped_dir, session.session_id
    )
}

/// Prints human-readable resume instructions (fallback when --shell-mode is not set).
pub fn print_resume_instructions(session: &SessionInfo) {
    println!("{}", format_resume_instructions(session));
}

pub(crate) fn is_valid_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if b != b'-' {
                    return false;
                }
            }
            _ => {
                if !b.is_ascii_hexdigit() || b.is_ascii_uppercase() {
                    return false;
                }
            }
        }
    }
    true
}

const FISH_TEMPLATE: &str = r#"function ccsesh
    set -l output (command ccsesh --shell-mode fish $argv)
    set -l rc $status
    set -l exec_idx 0
    for i in (seq (count $output))
        if test "$output[$i]" = "__CCSESH_EXEC__"
            set exec_idx $i
            break
        end
    end
    if test $exec_idx -gt 0
        for i in (seq (math $exec_idx + 1) (count $output))
            eval $output[$i]
        end
    else
        printf '%s\n' $output
        return $rc
    end
end
"#;

const BASH_TEMPLATE: &str = r#"ccsesh() {
    local output rc
    output=$(command ccsesh --shell-mode bash "$@")
    rc=$?
    if [[ "$output" == *"__CCSESH_EXEC__"* ]]; then
        eval "$(printf '%s\n' "$output" | sed -n '/^__CCSESH_EXEC__$/,$p' | tail -n +2)"
    else
        printf '%s\n' "$output"
        return $rc
    fi
}
"#;

const ZSH_TEMPLATE: &str = r#"ccsesh() {
    local output rc
    output=$(command ccsesh --shell-mode zsh "$@")
    rc=$?
    if [[ "$output" == *"__CCSESH_EXEC__"* ]]; then
        eval "$(print -r -- "$output" | sed -n '/^__CCSESH_EXEC__$/,$p' | tail -n +2)"
    else
        print -r -- "$output"
        return $rc
    fi
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_session(session_id: &str, project_dir: &str, display: &str) -> SessionInfo {
        SessionInfo {
            session_id: session_id.to_string(),
            path: PathBuf::from("/tmp/test.jsonl"),
            project_dir: PathBuf::from(project_dir),
            project_dir_display: display.to_string(),
            last_active: Utc::now(),
            first_prompt: Some("test prompt".to_string()),
            slug: None,
        }
    }

    #[test]
    fn test_is_valid_uuid() {
        assert!(is_valid_uuid("eb53d999-8692-42ce-a376-4f82206a086d"));
        assert!(is_valid_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(!is_valid_uuid("not-a-uuid"));
        assert!(!is_valid_uuid("EB53D999-8692-42CE-A376-4F82206A086D"));
        assert!(!is_valid_uuid("eb53d999-8692-42ce-a376-4f82206a086"));
        assert!(!is_valid_uuid("eb53d999-8692-42ce-a376-4f82206a086da"));
        assert!(!is_valid_uuid("eb53d999_8692_42ce_a376_4f82206a086d"));
        assert!(!is_valid_uuid("gb53d999-8692-42ce-a376-4f82206a086d"));
    }

    #[test]
    fn test_print_shell_init_fish() {
        assert!(print_shell_init("fish").is_ok());
    }

    #[test]
    fn test_print_shell_init_bash() {
        assert!(print_shell_init("bash").is_ok());
    }

    #[test]
    fn test_print_shell_init_zsh() {
        assert!(print_shell_init("zsh").is_ok());
    }

    #[test]
    fn test_print_shell_init_unknown() {
        let result = print_shell_init("nushell");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("nushell"));
    }

    #[test]
    fn test_fish_template_content() {
        assert!(FISH_TEMPLATE.contains("function ccsesh"));
        assert!(FISH_TEMPLATE.contains("__CCSESH_EXEC__"));
        assert!(FISH_TEMPLATE.contains("--shell-mode fish"));
    }

    #[test]
    fn test_bash_template_content() {
        assert!(BASH_TEMPLATE.contains("ccsesh()"));
        assert!(BASH_TEMPLATE.contains("__CCSESH_EXEC__"));
        assert!(BASH_TEMPLATE.contains("--shell-mode bash"));
    }

    #[test]
    fn test_zsh_template_content() {
        assert!(ZSH_TEMPLATE.contains("ccsesh()"));
        assert!(ZSH_TEMPLATE.contains("__CCSESH_EXEC__"));
        assert!(ZSH_TEMPLATE.contains("--shell-mode zsh"));
        assert!(ZSH_TEMPLATE.contains("print -r --"));
    }

    #[test]
    fn test_exec_protocol_valid_uuid() {
        let session = make_session(
            "eb53d999-8692-42ce-a376-4f82206a086d",
            "/home/user/project",
            "~/project",
        );
        assert!(print_exec_protocol(&session).is_ok());
    }

    #[test]
    fn test_exec_protocol_invalid_uuid() {
        let session = make_session("not-a-uuid", "/home/user/project", "~/project");
        assert!(print_exec_protocol(&session).is_err());
    }

    #[test]
    fn test_exec_protocol_path_with_spaces() {
        let session = make_session(
            "eb53d999-8692-42ce-a376-4f82206a086d",
            "/home/user/my project",
            "~/my project",
        );
        assert!(print_exec_protocol(&session).is_ok());
    }

    #[test]
    fn test_exec_protocol_path_with_single_quotes() {
        let session = make_session(
            "eb53d999-8692-42ce-a376-4f82206a086d",
            "/tmp/it's here",
            "~/it's here",
        );
        assert!(print_exec_protocol(&session).is_ok());
    }

    #[test]
    fn test_resume_instructions_uses_full_path() {
        let session = make_session(
            "eb53d999-8692-42ce-a376-4f82206a086d",
            "/home/user/project",
            "~/project",
        );
        let output = format_resume_instructions(&session);
        // Must use the full path, not the tilde-abbreviated display path,
        // because tilde expansion does not occur inside single quotes.
        assert!(
            output.contains("/home/user/project"),
            "expected full path in output, got: {output}"
        );
        assert!(
            !output.contains("~/project"),
            "must not use tilde-abbreviated path: {output}"
        );
        assert!(output.contains("claude --resume eb53d999-8692-42ce-a376-4f82206a086d"));
    }

    #[test]
    fn test_resume_instructions_path_with_spaces() {
        let session = make_session(
            "eb53d999-8692-42ce-a376-4f82206a086d",
            "/home/user/my project",
            "~/my project",
        );
        let output = format_resume_instructions(&session);
        // Path with spaces must be wrapped in single quotes
        assert!(
            output.contains("'/home/user/my project'"),
            "expected single-quoted path with spaces, got: {output}"
        );
    }

    #[test]
    fn test_resume_instructions_path_with_single_quotes() {
        let session = make_session(
            "eb53d999-8692-42ce-a376-4f82206a086d",
            "/tmp/it's here",
            "~/it's here",
        );
        let output = format_resume_instructions(&session);
        // Internal single quotes must be escaped as '\''
        assert!(
            output.contains("'\\''"),
            "expected escaped single quote in output, got: {output}"
        );
        assert!(output.contains("/tmp/it"));
    }
}
