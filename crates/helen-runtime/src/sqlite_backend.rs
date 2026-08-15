//! SQLite backend for transcript persistence (Task 8.1) —
//! port of `helen/runtime/transcript_store.py::SQLiteBackend`.
//!
//! Schema (byte-compatible with Python):
//! ```sql
//! CREATE TABLE transcript (
//!     id INTEGER PRIMARY KEY AUTOINCREMENT,
//!     uuid TEXT UNIQUE NOT NULL,
//!     type TEXT NOT NULL,
//!     data TEXT NOT NULL,
//!     timestamp REAL NOT NULL
//! );
//! CREATE INDEX idx_uuid ON transcript(uuid);
//! CREATE INDEX idx_timestamp ON transcript(timestamp);
//! ```
//! WAL mode, `synchronous=NORMAL`, `temp_store=MEMORY`.
//! The `data` column stores the same item dict JSON as the JSONL backend,
//! so SQLite and JSONL transcripts are byte-compatible with Python.

use crate::transcript::{Item, SessionMeta};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// SQLite backend for transcript persistence with WAL mode.
#[derive(Debug)]
pub struct SqliteBackend {
    pub path: PathBuf,
    conn: Connection,
}

impl SqliteBackend {
    /// Open (creating) the SQLite database with the Python schema.
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS transcript (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT UNIQUE NOT NULL,
                type TEXT NOT NULL,
                data TEXT NOT NULL,
                timestamp REAL NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_uuid ON transcript(uuid);
            CREATE INDEX IF NOT EXISTS idx_timestamp ON transcript(timestamp);",
        )?;
        Ok(Self { path, conn })
    }

    /// `append` — INSERT OR REPLACE on UNIQUE uuid (Python parity).
    pub fn append(&self, item: &Item) {
        let item_dict = item.to_dict();
        let uuid = item.uuid().to_string();
        let type_ = item_dict
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("message")
            .to_string();
        let data = serde_json::to_string(&item_dict).unwrap_or_default();
        let ts = crate::observability::now_ts();
        let r = self.conn.execute(
            "INSERT OR REPLACE INTO transcript (uuid, type, data, timestamp) VALUES (?1, ?2, ?3, ?4)",
            params![uuid, type_, data, ts],
        );
        if let Err(e) = r {
            eprintln!("SqliteBackend: failed to append: {e}");
        }
    }

    /// `load_all` — all items ordered by id.
    pub fn load_all(&self) -> Vec<Item> {
        let mut items = Vec::new();
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT data FROM transcript ORDER BY id ASC")
        else {
            return items;
        };
        let rows = stmt.query_map([], |row| row.get::<_, String>(0));
        if let Ok(rows) = rows {
            for row in rows.flatten() {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&row) {
                    if let Some(item) = Item::from_dict(&data) {
                        items.push(item);
                    }
                }
            }
        }
        items
    }

    /// `write_meta` — upsert the single-row session_meta table (v1.23.3).
    pub fn write_meta(&self, meta: &SessionMeta) {
        let _ = self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                argv TEXT,
                timestamp REAL,
                helen_version TEXT,
                python_version TEXT,
                platform TEXT,
                cwd TEXT,
                session_id TEXT,
                session_scope TEXT
            )",
        );
        let argv = serde_json::to_string(&meta.argv).unwrap_or_else(|_| "[]".into());
        let r = self.conn.execute(
            "INSERT OR REPLACE INTO session_meta
             (id, argv, timestamp, helen_version, python_version,
              platform, cwd, session_id, session_scope)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                argv,
                meta.timestamp,
                meta.helen_version,
                meta.python_version,
                meta.platform,
                meta.cwd,
                meta.session_id,
                meta.session_scope
            ],
        );
        if let Err(e) = r {
            eprintln!("SqliteBackend: failed to write meta: {e}");
        }
    }

    /// `read_meta` — None if table absent or empty (Python parity).
    pub fn read_meta(&self) -> Option<SessionMeta> {
        let _table_ok = self
            .conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='session_meta'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()?;
        let row = self
            .conn
            .query_row(
                "SELECT argv, timestamp, helen_version, python_version,
                        platform, cwd, session_id, session_scope
                 FROM session_meta WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, f64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()
            .ok()
            .flatten()?;
        let argv = serde_json::from_str::<Vec<String>>(&row.0).unwrap_or_default();
        Some(SessionMeta {
            argv,
            timestamp: row.1,
            helen_version: row.2,
            python_version: row.3,
            platform: row.4,
            cwd: row.5,
            session_id: row.6,
            session_scope: row.7,
            parent_session_id: String::new(),
        })
    }

    /// `update_pinned` — parse JSON data, update pinned, re-insert (v1.30.1).
    pub fn update_pinned(&self, uuid: &str, pinned: bool) {
        let Ok(data) = self
            .conn
            .query_row(
                "SELECT data FROM transcript WHERE uuid = ?1",
                params![uuid],
                |row| row.get::<_, String>(0),
            )
            .optional()
        else {
            return;
        };
        let Some(data) = data else { return };
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("pinned".into(), serde_json::Value::Bool(pinned));
            }
            let type_ = v
                .get("type")
                .and_then(|x| x.as_str())
                .unwrap_or("message")
                .to_string();
            let new_data = serde_json::to_string(&v).unwrap_or_default();
            let ts = crate::observability::now_ts();
            let _ = self.conn.execute(
                "INSERT OR REPLACE INTO transcript (uuid, type, data, timestamp) VALUES (?1, ?2, ?3, ?4)",
                params![uuid, type_, new_data, ts],
            );
        }
    }

    /// `query` — WHERE pushdown on timestamp + JSON_EXTRACT filters, then
    /// regex filtering and offset/limit in post-processing (Python parity).
    #[allow(clippy::too_many_arguments)]
    pub fn query(
        &self,
        roles: Option<&[String]>,
        agent_names: Option<&[String]>,
        invocation_ids: Option<&[String]>,
        since: Option<f64>,
        until: Option<f64>,
        content_regex: Option<&str>,
        message_types: Option<&[String]>,
        limit: Option<usize>,
        offset: usize,
    ) -> Vec<Item> {
        let mut where_clauses: Vec<String> = Vec::new();
        let mut params_list: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(since) = since {
            where_clauses.push("timestamp >= ?".into());
            params_list.push(rusqlite::types::Value::Real(since));
        }
        if let Some(until) = until {
            where_clauses.push("timestamp <= ?".into());
            params_list.push(rusqlite::types::Value::Real(until));
        }
        if let Some(roles) = roles {
            if !roles.is_empty() {
                let placeholders = vec!["?"; roles.len()].join(",");
                where_clauses.push(format!("json_extract(data, '$.role') IN ({placeholders})"));
                for r in roles {
                    params_list.push(rusqlite::types::Value::Text(r.clone()));
                }
            }
        }
        if let Some(agents) = agent_names {
            if !agents.is_empty() {
                let placeholders = vec!["?"; agents.len()].join(",");
                where_clauses
                    .push(format!("json_extract(data, '$.agent_name') IN ({placeholders})"));
                for a in agents {
                    params_list.push(rusqlite::types::Value::Text(a.clone()));
                }
            }
        }
        if let Some(inv) = invocation_ids {
            if !inv.is_empty() {
                let placeholders = vec!["?"; inv.len()].join(",");
                where_clauses.push(format!(
                    "json_extract(data, '$.invocation_id') IN ({placeholders})"
                ));
                for i in inv {
                    params_list.push(rusqlite::types::Value::Text(i.clone()));
                }
            }
        }
        if let Some(mtypes) = message_types {
            if !mtypes.is_empty() {
                let placeholders = vec!["?"; mtypes.len()].join(",");
                where_clauses
                    .push(format!("json_extract(data, '$.message_type') IN ({placeholders})"));
                for t in mtypes {
                    params_list.push(rusqlite::types::Value::Text(t.clone()));
                }
            }
        }

        let mut sql = "SELECT data FROM transcript".to_string();
        if !where_clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY timestamp ASC");

        let mut items = Vec::new();
        let Ok(mut stmt) = self.conn.prepare(&sql) else {
            return items;
        };
        let rows = stmt.query_map(rusqlite::params_from_iter(params_list), |row| {
            row.get::<_, String>(0)
        });
        if let Ok(rows) = rows {
            for row in rows.flatten() {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&row) {
                    let Some(item) = Item::from_dict(&data) else { continue };
                    // Content regex filter (post-processing, Python parity).
                    if let Some(re) = content_regex {
                        if let Item::Message(m) = &item {
                            let content = match &m.content {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            if regex::Regex::new(re)
                                .map(|r| !r.is_match(&content))
                                .unwrap_or(false)
                            {
                                continue;
                            }
                        }
                    }
                    items.push(item);
                }
            }
        }

        if offset > 0 {
            items = items.into_iter().skip(offset).collect();
        }
        if let Some(limit) = limit {
            items.truncate(limit);
        }
        items
    }

    pub fn close(&self) {
        // rusqlite Connection closes on drop; nothing to do.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_fixture_roundtrip() {
        // Fixture written by Python `helen/runtime/transcript_store.py`.
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/python_session.db");
        assert!(fixture.exists(), "fixture missing: {}", fixture.display());
        let b = SqliteBackend::open(&fixture).unwrap();
        let items = b.load_all();
        assert_eq!(items.len(), 3);
        let Item::Message(m1) = &items[0] else { panic!("first item must be message") };
        assert_eq!(m1.role, "user");
        assert_eq!(m1.uuid, "u1");
        assert_eq!(m1.agent_name.as_deref(), Some("agentA"));
        assert_eq!(m1.invocation_id, "inv1");
        assert_eq!(m1.priority, 90);
        let Item::Message(m2) = &items[1] else { panic!("second item must be message") };
        assert!(m2.pinned);
        let Item::Boundary(bm) = &items[2] else { panic!("third item must be boundary") };
        assert_eq!(bm.layer, "microcompact");
        assert_eq!(bm.summary, "[Compressed: 2 msgs]");

        // session_meta roundtrip.
        let meta = b.read_meta().expect("meta present");
        assert_eq!(meta.session_id, "s1");
        assert_eq!(meta.helen_version, "1.45.0");
        assert_eq!(meta.argv, vec!["helen", "-s"]);
        assert_eq!(meta.session_scope, "project");
        b.close();
    }

    #[test]
    fn append_write_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("helen_sqlite_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        let _ = std::fs::remove_file(&path);

        let b = SqliteBackend::open(&path).unwrap();
        let m = crate::transcript::Message::new(
            "user",
            serde_json::json!("content here"),
            vec![],
            None,
            "x1".into(),
            None,
            50,
            false,
            false,
            None,
            String::new(),
            String::new(),
            vec![],
        );
        let item = Item::Message(m.clone());
        b.append(&item);
        b.update_pinned("x1", true);
        let items = b.load_all();
        assert_eq!(items.len(), 1);
        let Item::Message(m2) = &items[0] else { panic!() };
        assert!(m2.pinned, "update_pinned must persist");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn query_filters() {
        let dir = std::env::temp_dir().join(format!("helen_sqlite_q_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("q.db");
        let _ = std::fs::remove_file(&path);
        let b = SqliteBackend::open(&path).unwrap();
        for (i, role) in ["user", "assistant", "user"].iter().enumerate() {
            let m = crate::transcript::Message::new(
                role,
                serde_json::json!(format!("msg {i}")),
                vec![],
                None,
                format!("m{i}"),
                None,
                50,
                false,
                false,
                Some(format!("agent{}", i % 2)),
                String::new(),
                String::new(),
                vec![],
            );
            let item = Item::Message(m.clone());
            b.append(&item);
        }
        let users = b.query(Some(&["user".to_string()]), None, None, None, None, None, None, None, 0);
        assert_eq!(users.len(), 2);
        let agent1 = b.query(None, Some(&["agent1".to_string()]), None, None, None, None, None, None, 0);
        assert_eq!(agent1.len(), 1);
        let regex = b.query(None, None, None, None, None, Some("msg 1"), None, None, 0);
        assert_eq!(regex.len(), 1);
        let limited = b.query(None, None, None, None, None, None, None, Some(1), 0);
        assert_eq!(limited.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_meta_returns_none() {
        let dir = std::env::temp_dir().join(format!("helen_sqlite_m_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.db");
        let _ = std::fs::remove_file(&path);
        let b = SqliteBackend::open(&path).unwrap();
        assert!(b.read_meta().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
