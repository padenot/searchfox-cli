use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const FRESH_SECS: u64 = 3600;
const PRUNE_SECS: u64 = 7 * 24 * 3600;

pub struct CacheEntry {
    pub content: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub cached_at: u64,
}

impl CacheEntry {
    pub fn is_fresh(&self) -> bool {
        now().saturating_sub(self.cached_at) < FRESH_SECS
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn cache_path() -> Option<PathBuf> {
    let base = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else {
        PathBuf::from(std::env::var("HOME").ok()?).join(".cache")
    };
    Some(base.join("searchfox-cli").join("cache.db"))
}

pub fn open() -> Option<Connection> {
    let path = cache_path()?;
    std::fs::create_dir_all(path.parent()?).ok()?;
    let conn = Connection::open(&path).ok()?;
    init(&conn)?;
    Some(conn)
}

pub fn get(conn: &Connection, url: &str) -> Option<CacheEntry> {
    conn.query_row(
        "SELECT content, etag, last_modified, cached_at FROM cache WHERE url = ?1",
        params![url],
        |row| {
            Ok(CacheEntry {
                content: row.get(0)?,
                etag: row.get(1)?,
                last_modified: row.get(2)?,
                cached_at: row.get::<_, i64>(3)? as u64,
            })
        },
    )
    .ok()
}

pub fn set(
    conn: &Connection,
    url: &str,
    content: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO cache (url, content, etag, last_modified, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![url, content, etag, last_modified, now() as i64],
    );
}

fn init(conn: &Connection) -> Option<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cache (
            url TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            etag TEXT,
            last_modified TEXT,
            cached_at INTEGER NOT NULL
        );",
    )
    .ok()
}

pub fn open_in_memory() -> anyhow::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    init(&conn).ok_or_else(|| anyhow::anyhow!("failed to init in-memory cache"))?;
    Ok(conn)
}

#[cfg(test)]
pub fn set_with_timestamp(
    conn: &Connection,
    url: &str,
    content: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    cached_at: i64,
) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO cache (url, content, etag, last_modified, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![url, content, etag, last_modified, cached_at],
    );
}

pub fn prune(conn: &Connection) {
    let cutoff = (now() - PRUNE_SECS) as i64;
    let _ = conn.execute("DELETE FROM cache WHERE cached_at < ?1", params![cutoff]);
}

pub fn clear() -> std::io::Result<bool> {
    let Some(path) = cache_path() else {
        return Ok(false);
    };

    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn db() -> Connection {
        open_in_memory().unwrap()
    }

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock poisoned")
    }

    fn temp_cache_home() -> PathBuf {
        std::env::temp_dir().join(format!(
            "searchfox-cli-cache-test-{}-{}",
            std::process::id(),
            now()
        ))
    }

    #[test]
    fn miss_returns_none() {
        let conn = db();
        assert!(get(&conn, "https://example.com/file").is_none());
    }

    #[test]
    fn set_get_roundtrip() {
        let conn = db();
        set(
            &conn,
            "https://example.com/file",
            "hello",
            Some("\"abc\""),
            Some("Thu, 01 Jan 2026 00:00:00 GMT"),
        );
        let entry = get(&conn, "https://example.com/file").unwrap();
        assert_eq!(entry.content, "hello");
        assert_eq!(entry.etag.as_deref(), Some("\"abc\""));
        assert_eq!(
            entry.last_modified.as_deref(),
            Some("Thu, 01 Jan 2026 00:00:00 GMT")
        );
    }

    #[test]
    fn set_without_etag() {
        let conn = db();
        set(&conn, "https://example.com/file", "content", None, None);
        let entry = get(&conn, "https://example.com/file").unwrap();
        assert_eq!(entry.content, "content");
        assert!(entry.etag.is_none());
        assert!(entry.last_modified.is_none());
    }

    #[test]
    fn fresh_entry_is_fresh() {
        let conn = db();
        set(&conn, "https://example.com/file", "x", None, None);
        assert!(get(&conn, "https://example.com/file").unwrap().is_fresh());
    }

    #[test]
    fn old_entry_is_not_fresh() {
        let conn = db();
        set_with_timestamp(&conn, "https://example.com/file", "x", None, None, 0);
        assert!(!get(&conn, "https://example.com/file").unwrap().is_fresh());
    }

    #[test]
    fn set_replaces_existing_entry() {
        let conn = db();
        set(
            &conn,
            "https://example.com/file",
            "v1",
            Some("\"etag1\""),
            None,
        );
        set(
            &conn,
            "https://example.com/file",
            "v2",
            Some("\"etag2\""),
            None,
        );
        let entry = get(&conn, "https://example.com/file").unwrap();
        assert_eq!(entry.content, "v2");
        assert_eq!(entry.etag.as_deref(), Some("\"etag2\""));
    }

    #[test]
    fn prune_removes_old_keeps_fresh() {
        let conn = db();
        set_with_timestamp(&conn, "https://example.com/old", "old", None, None, 0);
        set(&conn, "https://example.com/new", "new", None, None);
        prune(&conn);
        assert!(get(&conn, "https://example.com/old").is_none());
        assert!(get(&conn, "https://example.com/new").is_some());
    }

    #[test]
    fn different_urls_are_independent() {
        let conn = db();
        set(&conn, "https://example.com/a", "aaa", Some("\"e1\""), None);
        set(&conn, "https://example.com/b", "bbb", Some("\"e2\""), None);
        assert_eq!(get(&conn, "https://example.com/a").unwrap().content, "aaa");
        assert_eq!(get(&conn, "https://example.com/b").unwrap().content, "bbb");
    }

    #[test]
    fn clear_removes_database_file() {
        let _guard = env_lock();
        let dir = temp_cache_home();
        let previous = std::env::var_os("XDG_CACHE_HOME");
        std::env::set_var("XDG_CACHE_HOME", &dir);

        {
            let path = cache_path().expect("cache path");
            std::fs::create_dir_all(path.parent().expect("cache dir")).unwrap();
            std::fs::write(&path, b"cache").unwrap();

            assert!(path.exists());
            assert!(clear().unwrap());
            assert!(!path.exists());
            assert!(!clear().unwrap());
        }

        if let Some(value) = previous {
            std::env::set_var("XDG_CACHE_HOME", value);
        } else {
            std::env::remove_var("XDG_CACHE_HOME");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
