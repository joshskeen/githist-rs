//! Persisted links between git branches and Cursor Agent CLI sessions.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkedSession {
    pub session_id: String,
    pub title: Option<String>,
    pub linked_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentStore {
    pub schema: u32,
    pub branches: HashMap<String, Vec<LinkedSession>>,
}

impl Default for AgentStore {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            branches: HashMap::new(),
        }
    }
}

/// Normalize a git remote URL for stable repo identification.
///
/// Lowercases the scheme (`HTTPS` → `https`), strips a trailing `.git`, and
/// removes a trailing slash.
pub fn normalize_remote_url(url: &str) -> String {
    let mut result = url.trim().to_string();
    if let Some(idx) = result.find("://") {
        let (scheme, rest) = result.split_at(idx);
        result = format!("{}{}", scheme.to_ascii_lowercase(), rest);
    }

    if result.ends_with('/') {
        result.pop();
    }
    if result.ends_with(".git") {
        result.truncate(result.len() - 4);
    }
    result
}

/// Filesystem-safe repo identifier derived from a normalized remote URL.
pub fn repo_id_from_remote(url: &str) -> String {
    normalize_remote_url(url)
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '@' | '?' | '*' | '<' | '>' | '|' | '"' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

/// Stable FNV-1a 64-bit hash (independent of Rust's `DefaultHasher`).
fn fnv1a64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for &b in data {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Filesystem-safe repo identifier derived from a local toplevel/workdir path.
pub fn repo_id_from_path(path: &Path) -> String {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let normalized = canonical.components().as_path();
    let bytes = normalized.as_os_str().as_encoded_bytes();
    format!("path_{:016x}", fnv1a64(bytes))
}

fn config_root() -> PathBuf {
    if let Ok(dir) = std::env::var("GITHIST_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config")
}

pub fn store_path(repo_id: &str) -> PathBuf {
    config_root()
        .join("githist")
        .join(repo_id)
        .join("agents.json")
}

impl AgentStore {
    pub fn load(path: &Path) -> io::Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        serde_json::from_str(&contents).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, json)
    }

    pub fn link(&mut self, branch: &str, session_id: &str, title: Option<String>) {
        let linked_at = chrono::Utc::now().to_rfc3339();
        let session = LinkedSession {
            session_id: session_id.to_string(),
            title,
            linked_at,
        };
        let sessions = self.branches.entry(branch.to_string()).or_default();
        sessions.retain(|s| s.session_id != session_id);
        sessions.insert(0, session);
    }

    pub fn unlink(&mut self, branch: &str, session_id: &str) {
        if let Some(sessions) = self.branches.get_mut(branch) {
            sessions.retain(|s| s.session_id != session_id);
            if sessions.is_empty() {
                self.branches.remove(branch);
            }
        }
    }

    pub fn sessions_for(&self, branch: &str) -> Option<&[LinkedSession]> {
        self.branches
            .get(branch)
            .map(|sessions| sessions.as_slice())
    }

    pub fn has_any_links(&self) -> bool {
        self.branches.values().any(|sessions| !sessions.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn normalize_remote_url_strips_suffix_and_lowercases_scheme() {
        assert_eq!(
            normalize_remote_url("HTTPS://GitHub.com/user/Repo.git"),
            "https://GitHub.com/user/Repo"
        );
        assert_eq!(
            normalize_remote_url("git@github.com:user/repo.git"),
            "git@github.com:user/repo"
        );
        assert_eq!(
            normalize_remote_url("https://example.com/foo/"),
            "https://example.com/foo"
        );
    }

    #[test]
    fn repo_id_from_path_is_stable_and_filesystem_safe() {
        let dir = TempDir::new().unwrap();
        let id = repo_id_from_path(dir.path());
        assert_eq!(id, repo_id_from_path(dir.path()));
        assert!(id.starts_with("path_"));
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn repo_id_from_remote_https_and_ssh() {
        assert_eq!(
            repo_id_from_remote("https://github.com/user/repo.git"),
            "https___github.com_user_repo"
        );
        assert_eq!(
            repo_id_from_remote("git@github.com:user/repo.git"),
            "git_github.com_user_repo"
        );
    }

    #[test]
    fn missing_file_loads_empty_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");
        let store = AgentStore::load(&path).unwrap();
        assert_eq!(store.schema, 1);
        assert!(store.branches.is_empty());
        assert!(!store.has_any_links());
    }

    #[test]
    fn corrupt_json_returns_err() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");
        fs::write(&path, "{ not valid json").unwrap();

        let err = AgentStore::load(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn link_and_list_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");

        let mut store = AgentStore::load(&path).unwrap();
        store.link(
            "feature/foo",
            "f61f674e-03fe-4c67-b1c6-d1538221a9d4",
            Some("my session".to_string()),
        );
        store.save(&path).unwrap();

        let loaded = AgentStore::load(&path).unwrap();
        let sessions = loaded.sessions_for("feature/foo").unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].session_id,
            "f61f674e-03fe-4c67-b1c6-d1538221a9d4"
        );
        assert_eq!(sessions[0].title.as_deref(), Some("my session"));
        assert!(!sessions[0].linked_at.is_empty());

        store.unlink("feature/foo", "f61f674e-03fe-4c67-b1c6-d1538221a9d4");
        assert!(store.sessions_for("feature/foo").is_none());

        assert!(loaded.has_any_links());
    }

    #[test]
    fn unlink_save_load_roundtrip_clears_links() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");

        let mut store = AgentStore::load(&path).unwrap();
        store.link("main", "session-a", None);
        store.save(&path).unwrap();

        store.unlink("main", "session-a");
        store.save(&path).unwrap();

        let loaded = AgentStore::load(&path).unwrap();
        assert!(loaded.sessions_for("main").is_none());
        assert!(!loaded.has_any_links());
    }

    #[test]
    fn relink_same_session_updates_order_and_linked_at() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");

        let mut store = AgentStore::load(&path).unwrap();
        store.link("main", "session-a", Some("first".to_string()));
        let original_linked_at = store.sessions_for("main").unwrap()[0].linked_at.clone();
        store.link("main", "session-b", Some("second".to_string()));
        assert_eq!(
            store.sessions_for("main").unwrap()[0].session_id,
            "session-b"
        );

        std::thread::sleep(std::time::Duration::from_millis(10));
        store.link("main", "session-a", Some("first again".to_string()));

        let sessions = store.sessions_for("main").unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "session-a");
        assert_eq!(sessions[0].title.as_deref(), Some("first again"));
        assert_eq!(sessions[1].session_id, "session-b");
        assert_ne!(sessions[0].linked_at, original_linked_at);
    }

    #[test]
    fn store_path_uses_githist_config_dir() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config");
        fs::create_dir_all(&config).unwrap();
        let prev = std::env::var("GITHIST_CONFIG_DIR").ok();
        // SAFETY: test-only env mutation; restored before return.
        unsafe { std::env::set_var("GITHIST_CONFIG_DIR", &config) };

        let path = store_path("my-repo-id");
        assert_eq!(
            path,
            config.join("githist").join("my-repo-id").join("agents.json")
        );

        match prev {
            Some(v) => unsafe { std::env::set_var("GITHIST_CONFIG_DIR", v) },
            None => unsafe { std::env::remove_var("GITHIST_CONFIG_DIR") },
        }
    }
}
