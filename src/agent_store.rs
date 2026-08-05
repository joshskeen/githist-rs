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
        .replace("://", "_")
        .replace('/', "_")
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
    pub fn load(path: &Path) -> Self {
        let contents = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(_) => return Self::default(),
        };
        serde_json::from_str(&contents).unwrap_or_default()
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
    fn missing_file_loads_empty_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");
        let store = AgentStore::load(&path);
        assert_eq!(store.schema, 1);
        assert!(store.branches.is_empty());
        assert!(!store.has_any_links());
    }

    #[test]
    fn link_and_list_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");

        let mut store = AgentStore::load(&path);
        store.link(
            "feature/foo",
            "f61f674e-03fe-4c67-b1c6-d1538221a9d4",
            Some("my session".to_string()),
        );
        store.save(&path).unwrap();

        let loaded = AgentStore::load(&path);
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
