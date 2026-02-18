use std::path::Path;

use anyhow::Result;

use crate::errors::CcseshError;
use crate::types::SessionCandidate;

/// Discover JSONL session files under `{home_dir}/.claude/projects/`.
///
/// Returns up to `limit` candidates sorted by mtime descending (most recent first).
/// Returns `Ok(vec![])` for `limit=0` without doing any I/O, and also when
/// no JSONL files are found (caller decides whether to raise `NoSessionsFound`).
pub fn discover_sessions(home_dir: &str, limit: usize) -> Result<Vec<SessionCandidate>> {
    if limit == 0 {
        return Ok(vec![]);
    }

    let projects_dir = Path::new(home_dir).join(".claude").join("projects");

    if !projects_dir.is_dir() {
        return Err(CcseshError::ProjectsDirNotFound { path: projects_dir }.into());
    }

    let mut candidates = Vec::new();

    let project_entries = match std::fs::read_dir(&projects_dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(vec![]),
    };

    for project_entry in project_entries {
        let project_entry = match project_entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        let session_entries = match std::fs::read_dir(&project_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for session_entry in session_entries {
            let session_entry = match session_entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let file_path = session_entry.path();

            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            let metadata = match file_path.symlink_metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }

            let mtime = match metadata.modified() {
                Ok(t) => t,
                Err(_) => continue,
            };

            candidates.push(SessionCandidate {
                path: file_path,
                mtime,
            });
        }
    }

    candidates.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    candidates.truncate(limit);

    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime};

    /// Helper: create `.claude/projects/` under the given root dir.
    fn setup_projects_dir(root: &std::path::Path) -> std::path::PathBuf {
        let projects = root.join(".claude").join("projects");
        fs::create_dir_all(&projects).unwrap();
        projects
    }

    /// Helper: create a .jsonl file and set its mtime.
    fn create_jsonl_with_mtime(dir: &std::path::Path, name: &str, mtime: SystemTime) {
        let path = dir.join(name);
        fs::write(&path, "{}").unwrap();
        let times = fs::FileTimes::new().set_modified(mtime);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(times)
            .unwrap();
    }

    #[test]
    fn sorting_by_mtime_descending() {
        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());
        let project = projects.join("my-project");
        fs::create_dir_all(&project).unwrap();

        let now = SystemTime::now();
        create_jsonl_with_mtime(&project, "old.jsonl", now - Duration::from_secs(100));
        create_jsonl_with_mtime(&project, "newest.jsonl", now);
        create_jsonl_with_mtime(&project, "middle.jsonl", now - Duration::from_secs(50));

        let result = discover_sessions(tmp.path().to_str().unwrap(), 10).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].path.file_name().unwrap(), "newest.jsonl");
        assert_eq!(result[1].path.file_name().unwrap(), "middle.jsonl");
        assert_eq!(result[2].path.file_name().unwrap(), "old.jsonl");
    }

    #[test]
    fn limit_clamping() {
        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());
        let project = projects.join("my-project");
        fs::create_dir_all(&project).unwrap();

        fs::write(project.join("a.jsonl"), "{}").unwrap();
        fs::write(project.join("b.jsonl"), "{}").unwrap();

        let result = discover_sessions(tmp.path().to_str().unwrap(), 100).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn limit_actually_limits() {
        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());
        let project = projects.join("my-project");
        fs::create_dir_all(&project).unwrap();

        for i in 0..5 {
            fs::write(project.join(format!("{i}.jsonl")), "{}").unwrap();
        }

        let result = discover_sessions(tmp.path().to_str().unwrap(), 2).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn limit_zero_returns_empty_without_io() {
        // Even a nonexistent home dir should work with limit=0.
        let result = discover_sessions("/nonexistent/path/that/does/not/exist", 0).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn empty_projects_directory() {
        let tmp = assert_fs::TempDir::new().unwrap();
        setup_projects_dir(tmp.path());

        let result = discover_sessions(tmp.path().to_str().unwrap(), 10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn empty_project_subdirectory() {
        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());
        // Project dir exists but has no files.
        fs::create_dir_all(projects.join("empty-project")).unwrap();

        let result = discover_sessions(tmp.path().to_str().unwrap(), 10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn non_jsonl_files_ignored() {
        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());
        let project = projects.join("my-project");
        fs::create_dir_all(&project).unwrap();

        fs::write(project.join("session.jsonl"), "{}").unwrap();
        fs::write(project.join("notes.txt"), "hello").unwrap();
        fs::write(project.join("data.json"), "{}").unwrap();
        fs::write(project.join("readme"), "hi").unwrap();

        let result = discover_sessions(tmp.path().to_str().unwrap(), 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path.file_name().unwrap(), "session.jsonl");
    }

    #[test]
    fn nested_subdirectories_ignored() {
        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());
        let project = projects.join("my-project");
        fs::create_dir_all(&project).unwrap();

        // Top-level jsonl — should be found.
        fs::write(project.join("top.jsonl"), "{}").unwrap();

        // Nested jsonl in subdirectories — should be ignored.
        let memory = project.join("memory");
        fs::create_dir_all(&memory).unwrap();
        fs::write(memory.join("nested.jsonl"), "{}").unwrap();

        let uuid_dir = project.join("some-uuid-dir");
        fs::create_dir_all(&uuid_dir).unwrap();
        fs::write(uuid_dir.join("deep.jsonl"), "{}").unwrap();

        let result = discover_sessions(tmp.path().to_str().unwrap(), 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path.file_name().unwrap(), "top.jsonl");
    }

    #[test]
    fn projects_dir_not_found() {
        let tmp = assert_fs::TempDir::new().unwrap();
        // Don't create .claude/projects/.
        let result = discover_sessions(tmp.path().to_str().unwrap(), 5);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("No Claude Code session directory found")
        );
    }

    #[test]
    fn multiple_project_dirs() {
        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());

        let proj_a = projects.join("project-a");
        let proj_b = projects.join("project-b");
        fs::create_dir_all(&proj_a).unwrap();
        fs::create_dir_all(&proj_b).unwrap();

        let now = SystemTime::now();
        create_jsonl_with_mtime(&proj_a, "a1.jsonl", now - Duration::from_secs(10));
        create_jsonl_with_mtime(&proj_b, "b1.jsonl", now);

        let result = discover_sessions(tmp.path().to_str().unwrap(), 10).unwrap();
        assert_eq!(result.len(), 2);
        // Most recent first (b1 from project-b).
        assert_eq!(result[0].path.file_name().unwrap(), "b1.jsonl");
        assert_eq!(result[1].path.file_name().unwrap(), "a1.jsonl");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_to_jsonl_files_skipped() {
        use std::os::unix::fs as unix_fs;

        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());
        let project = projects.join("my-project");
        fs::create_dir_all(&project).unwrap();

        // Create a real JSONL file outside the project tree
        let outside = tmp.path().join("outside.jsonl");
        fs::write(&outside, "{}").unwrap();

        // Symlink from inside project to outside
        unix_fs::symlink(&outside, project.join("symlinked.jsonl")).unwrap();

        // Also create a real file to ensure discovery still works
        fs::write(project.join("real.jsonl"), "{}").unwrap();

        let result = discover_sessions(tmp.path().to_str().unwrap(), 10).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path.file_name().unwrap(), "real.jsonl");
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_files_skipped_silently() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = assert_fs::TempDir::new().unwrap();
        let projects = setup_projects_dir(tmp.path());
        let project = projects.join("my-project");
        fs::create_dir_all(&project).unwrap();

        fs::write(project.join("readable.jsonl"), "{}").unwrap();

        let unreadable = project.join("unreadable.jsonl");
        fs::write(&unreadable, "{}").unwrap();
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

        // Should not error — just skip the unreadable file.
        let result = discover_sessions(tmp.path().to_str().unwrap(), 10).unwrap();
        assert!(result.len() >= 1);

        // Restore permissions so temp dir cleanup succeeds.
        fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o644)).unwrap();
    }
}
