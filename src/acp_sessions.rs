//! Read Cursor Agent CLI session metadata from `~/.cursor/acp-sessions`.

use crate::agent_store::LinkedSession;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const DEFAULT_MAX_CANDIDATES: usize = 20;

#[derive(Debug, serde::Deserialize)]
struct SessionMeta {
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

fn acp_root() -> PathBuf {
    if let Ok(dir) = std::env::var("GITHIST_ACP_SESSIONS_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cursor").join("acp-sessions")
}

fn canonical_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Whether `session_cwd` refers to the same repo/worktree as `repo_cwd`.
pub fn cwd_matches_repo(session_cwd: &Path, repo_cwd: &Path) -> bool {
    let session = canonical_path(session_cwd);
    let repo = canonical_path(repo_cwd);
    session == repo || session.starts_with(&repo) || repo.starts_with(&session)
}

fn dir_mtime(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// Recent ACP sessions whose `cwd` matches `repo_cwd`, newest first.
pub fn list_candidates(repo_cwd: &Path, max: usize) -> Vec<LinkedSession> {
    let root = acp_root();
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };

    let mut candidates: Vec<(SystemTime, LinkedSession)> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let session_id = path.file_name()?.to_string_lossy().into_owned();
            let meta_path = path.join("meta.json");
            let meta: SessionMeta = fs::read_to_string(&meta_path)
                .ok()
                .and_then(|contents| serde_json::from_str(&contents).ok())?;
            let cwd = meta.cwd.as_deref()?;
            if !cwd_matches_repo(Path::new(cwd), repo_cwd) {
                return None;
            }
            let linked_at = chrono::Utc::now().to_rfc3339();
            Some((
                dir_mtime(&path),
                LinkedSession {
                    session_id,
                    title: meta.title,
                    linked_at,
                },
            ))
        })
        .collect();

    candidates.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
    candidates
        .into_iter()
        .take(max.min(DEFAULT_MAX_CANDIDATES))
        .map(|(_, session)| session)
        .collect()
}

/// Current agent session from `CURSOR_AGENT_SESSION_ID`, if set.
pub fn current_session_from_env() -> Option<LinkedSession> {
    let session_id = std::env::var("CURSOR_AGENT_SESSION_ID").ok()?;
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return None;
    }
    let meta_path = acp_root().join(session_id).join("meta.json");
    let title = fs::read_to_string(meta_path)
        .ok()
        .and_then(|contents| serde_json::from_str::<SessionMeta>(&contents).ok())
        .and_then(|meta| meta.title);
    Some(LinkedSession {
        session_id: session_id.to_string(),
        title,
        linked_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Persist a newly linked session for `branch_name`.
pub fn save_link(
    store: &mut crate::agent_store::AgentStore,
    repo_id: &str,
    branch_name: &str,
    session: &LinkedSession,
) -> io::Result<()> {
    store.link(branch_name, &session.session_id, session.title.clone());
    store.save(&crate::agent_store::store_path(repo_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_session(dir: &Path, id: &str, cwd: &str, title: &str) {
        let session_dir = dir.join(id);
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(
            session_dir.join("meta.json"),
            format!(r#"{{"cwd":"{cwd}","title":"{title}"}}"#),
        )
        .unwrap();
    }

    #[test]
    fn cwd_matches_repo_exact_and_prefix() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        let nested = repo.join("pkg");
        fs::create_dir_all(&nested).unwrap();

        assert!(cwd_matches_repo(&repo, &repo));
        assert!(cwd_matches_repo(&nested, &repo));
        assert!(cwd_matches_repo(&repo, &nested));
    }

    #[test]
    fn list_candidates_filters_by_repo_cwd() {
        let acp = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();

        write_session(
            acp.path(),
            "11111111-1111-1111-1111-111111111111",
            repo.path().to_str().unwrap(),
            "repo session",
        );
        write_session(
            acp.path(),
            "22222222-2222-2222-2222-222222222222",
            other.path().to_str().unwrap(),
            "other session",
        );

        let prev = std::env::var("GITHIST_ACP_SESSIONS_DIR").ok();
        // SAFETY: test-only env mutation; restored before return.
        unsafe { std::env::set_var("GITHIST_ACP_SESSIONS_DIR", acp.path()) };

        let candidates = list_candidates(repo.path(), 20);
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].session_id,
            "11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(candidates[0].title.as_deref(), Some("repo session"));

        match prev {
            Some(v) => unsafe { std::env::set_var("GITHIST_ACP_SESSIONS_DIR", v) },
            None => unsafe { std::env::remove_var("GITHIST_ACP_SESSIONS_DIR") },
        }
    }

    #[test]
    fn current_session_from_env_reads_title_from_meta() {
        let acp = TempDir::new().unwrap();
        write_session(
            acp.path(),
            "33333333-3333-3333-3333-333333333333",
            "/tmp",
            "env session",
        );

        let prev_acp = std::env::var("GITHIST_ACP_SESSIONS_DIR").ok();
        let prev_id = std::env::var("CURSOR_AGENT_SESSION_ID").ok();
        // SAFETY: test-only env mutation; restored before return.
        unsafe {
            std::env::set_var("GITHIST_ACP_SESSIONS_DIR", acp.path());
            std::env::set_var(
                "CURSOR_AGENT_SESSION_ID",
                "33333333-3333-3333-3333-333333333333",
            );
        }

        let session = current_session_from_env().unwrap();
        assert_eq!(session.session_id, "33333333-3333-3333-3333-333333333333");
        assert_eq!(session.title.as_deref(), Some("env session"));

        match prev_acp {
            Some(v) => unsafe { std::env::set_var("GITHIST_ACP_SESSIONS_DIR", v) },
            None => unsafe { std::env::remove_var("GITHIST_ACP_SESSIONS_DIR") },
        }
        match prev_id {
            Some(v) => unsafe { std::env::set_var("CURSOR_AGENT_SESSION_ID", v) },
            None => unsafe { std::env::remove_var("CURSOR_AGENT_SESSION_ID") },
        }
    }
}
