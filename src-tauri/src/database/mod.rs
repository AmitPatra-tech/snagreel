use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::models::{Download, DownloadRequest, LibraryQuery, Settings};

pub struct Db {
    conn: Mutex<Connection>,
}

const SELECT_DOWNLOAD: &str = "SELECT d.id, d.url, d.title, d.platform, d.thumbnail, d.filename, \
     d.file_path, d.format, d.resolution, d.audio_only, d.duration, d.file_size, d.status, \
     d.error, d.created_at, d.completed_at, IFNULL(q.priority, 0), IFNULL(q.position, 0), \
     d.clip_start, d.clip_end \
     FROM downloads d LEFT JOIN queue q ON q.download_id = d.id";

fn row_to_download(row: &Row) -> rusqlite::Result<Download> {
    Ok(Download {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        platform: row.get(3)?,
        thumbnail: row.get(4)?,
        filename: row.get(5)?,
        file_path: row.get(6)?,
        format: row.get(7)?,
        resolution: row.get(8)?,
        audio_only: row.get::<_, i64>(9)? != 0,
        duration: row.get(10)?,
        file_size: row.get(11)?,
        status: row.get(12)?,
        error: row.get(13)?,
        created_at: row.get(14)?,
        completed_at: row.get(15)?,
        priority: row.get(16)?,
        position: row.get(17)?,
        clip_start: row.get(18)?,
        clip_end: row.get(19)?,
    })
}

impl Db {
    pub fn init(db_path: &Path, default_download_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("failed to open database at {}", db_path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate(default_download_path)?;
        Ok(db)
    }

    fn migrate(&self, default_download_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

        if version < 1 {
            conn.execute_batch(
                "BEGIN;
                CREATE TABLE IF NOT EXISTS downloads (
                    id            INTEGER PRIMARY KEY AUTOINCREMENT,
                    url           TEXT NOT NULL,
                    title         TEXT NOT NULL DEFAULT '',
                    platform      TEXT NOT NULL DEFAULT '',
                    thumbnail     TEXT,
                    filename      TEXT,
                    file_path     TEXT,
                    format        TEXT NOT NULL DEFAULT '',
                    resolution    TEXT,
                    audio_only    INTEGER NOT NULL DEFAULT 0,
                    duration      REAL,
                    file_size     INTEGER,
                    status        TEXT NOT NULL DEFAULT 'queued',
                    error         TEXT,
                    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                    completed_at  TEXT
                );
                CREATE TABLE IF NOT EXISTS settings (
                    id                        INTEGER PRIMARY KEY CHECK (id = 1),
                    download_path             TEXT NOT NULL,
                    theme                     TEXT NOT NULL DEFAULT 'dark',
                    language                  TEXT NOT NULL DEFAULT 'en',
                    max_concurrent_downloads  INTEGER NOT NULL DEFAULT 3,
                    auto_update               INTEGER NOT NULL DEFAULT 1,
                    notifications             INTEGER NOT NULL DEFAULT 1,
                    filename_template         TEXT NOT NULL DEFAULT '%(title)s.%(ext)s',
                    organize_by_platform      INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE IF NOT EXISTS queue (
                    id           INTEGER PRIMARY KEY AUTOINCREMENT,
                    download_id  INTEGER NOT NULL UNIQUE REFERENCES downloads(id) ON DELETE CASCADE,
                    priority     INTEGER NOT NULL DEFAULT 0,
                    position     INTEGER NOT NULL DEFAULT 0,
                    status       TEXT NOT NULL DEFAULT 'queued'
                );
                CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status);
                CREATE INDEX IF NOT EXISTS idx_downloads_url ON downloads(url);
                CREATE INDEX IF NOT EXISTS idx_queue_order ON queue(priority DESC, position ASC);
                PRAGMA user_version = 1;
                COMMIT;",
            )?;
        }

        if version < 2 {
            // Pro activation: store the peppered hash of the accepted key
            // (never the key itself). NULL means the free tier.
            // Clip/"cutout": optional download-section bounds (seconds).
            conn.execute_batch(
                "BEGIN;
                ALTER TABLE settings ADD COLUMN pro_key_hash TEXT;
                ALTER TABLE downloads ADD COLUMN clip_start REAL;
                ALTER TABLE downloads ADD COLUMN clip_end REAL;
                PRAGMA user_version = 2;
                COMMIT;",
            )?;
        }

        if version < 3 {
            // Optional browser to pull cookies from (login-gated sites).
            conn.execute_batch(
                "BEGIN;
                ALTER TABLE settings ADD COLUMN cookies_browser TEXT NOT NULL DEFAULT 'none';
                PRAGMA user_version = 3;
                COMMIT;",
            )?;
        }

        if version < 4 {
            // Server-issued licensing: store the signed token, the key it came
            // from (so we can renew unattended), and a stable device id.
            // `pro_key_hash` is kept only to detect pre-1.1 Pro users, who have
            // to re-enter their key once — see `needs_reactivation`.
            conn.execute_batch(
                "BEGIN;
                ALTER TABLE settings ADD COLUMN pro_token TEXT;
                ALTER TABLE settings ADD COLUMN pro_key TEXT;
                ALTER TABLE settings ADD COLUMN device_id TEXT;
                PRAGMA user_version = 4;
                COMMIT;",
            )?;
        }

        conn.execute(
            "INSERT OR IGNORE INTO settings (id, download_path) VALUES (1, ?1)",
            params![default_download_path],
        )?;
        Ok(())
    }

    // ---------- activation ----------

    /// This installation's device id, generated on first use and then stable.
    pub fn device_id(&self) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let existing: Option<String> = conn.query_row(
            "SELECT device_id FROM settings WHERE id = 1",
            [],
            |row| row.get::<_, Option<String>>(0),
        )?;
        if let Some(id) = existing.filter(|id| id.len() == 64) {
            return Ok(id);
        }
        let fresh = crate::activation::new_device_id();
        conn.execute(
            "UPDATE settings SET device_id = ?1 WHERE id = 1",
            params![fresh],
        )?;
        Ok(fresh)
    }

    /// The stored licence token and the key that produced it.
    pub fn get_license(&self) -> Result<(Option<String>, Option<String>)> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT pro_token, pro_key FROM settings WHERE id = 1",
            [],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
        )?)
    }

    pub fn set_license(&self, token: &str, key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE settings SET pro_token = ?1, pro_key = ?2, pro_key_hash = NULL WHERE id = 1",
            params![token, key.trim()],
        )?;
        Ok(())
    }

    pub fn clear_license(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE settings SET pro_token = NULL, pro_key = NULL, pro_key_hash = NULL WHERE id = 1",
            [],
        )?;
        Ok(())
    }

    /// True when a pre-1.1 licence is on record but no token has replaced it.
    pub fn needs_reactivation(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT pro_key_hash IS NOT NULL AND pro_token IS NULL FROM settings WHERE id = 1",
            [],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false)
    }

    /// The verified claims of the stored token, if it is currently valid.
    ///
    /// Every check re-verifies the Ed25519 signature and the expiry, so editing
    /// the database by hand cannot unlock Pro — the value has to be a token
    /// this device was actually issued.
    pub fn license_claims(&self) -> Option<crate::activation::TokenClaims> {
        let device_id = self.device_id().ok()?;
        let (token, _) = self.get_license().ok()?;
        crate::activation::verify_token(&token?, &device_id)
    }

    pub fn is_pro(&self) -> bool {
        self.license_claims().is_some()
    }

    /// Crash recovery: anything left as `downloading` goes back to the queue.
    pub fn reset_in_flight(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET status = 'queued' WHERE status = 'downloading'",
            [],
        )?;
        conn.execute(
            "UPDATE queue SET status = 'queued' WHERE status = 'downloading'",
            [],
        )?;
        Ok(())
    }

    // ---------- downloads ----------

    pub fn insert_download(&self, req: &DownloadRequest) -> Result<Download> {
        let resolution = req
            .quality
            .as_ref()
            .map(|q| format!("{}p", q.trim_end_matches('p')));
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads (url, title, platform, thumbnail, format, resolution, audio_only, duration, status, clip_start, clip_end)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9, ?10)",
            params![
                req.url,
                req.title,
                req.platform,
                req.thumbnail,
                req.container,
                resolution,
                req.audio_only as i64,
                req.duration,
                req.clip_start,
                req.clip_end,
            ],
        )?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO queue (download_id, priority, position)
             VALUES (?1, 0, IFNULL((SELECT MAX(position) FROM queue), 0) + 1)",
            params![id],
        )?;
        drop(conn);
        self.get_download(id)
    }

    pub fn get_download(&self, id: i64) -> Result<Download> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("{SELECT_DOWNLOAD} WHERE d.id = ?1");
        Ok(conn.query_row(&sql, params![id], row_to_download)?)
    }

    pub fn get_completed_by_url(&self, url: &str) -> Result<Option<Download>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "{SELECT_DOWNLOAD} WHERE d.url = ?1 AND d.status = 'completed' ORDER BY d.completed_at DESC LIMIT 1"
        );
        Ok(conn
            .query_row(&sql, params![url], row_to_download)
            .optional()?)
    }

    pub fn list_downloads(&self) -> Result<Vec<Download>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "{SELECT_DOWNLOAD} ORDER BY IFNULL(q.priority, 0) DESC, \
             CASE WHEN q.position IS NULL THEN 1 ELSE 0 END ASC, \
             q.position ASC, d.created_at DESC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_download)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_status(&self, id: i64, status: &str, error: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if status == "completed" {
            conn.execute(
                "UPDATE downloads SET status = ?1, error = ?2, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3",
                params![status, error, id],
            )?;
        } else {
            conn.execute(
                "UPDATE downloads SET status = ?1, error = ?2 WHERE id = ?3",
                params![status, error, id],
            )?;
        }
        Ok(())
    }

    pub fn mark_completed(
        &self,
        id: i64,
        filename: &str,
        file_path: &str,
        file_size: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET status = 'completed', error = NULL, filename = ?1, file_path = ?2, \
             file_size = ?3, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?4",
            params![filename, file_path, file_size, id],
        )?;
        Ok(())
    }

    /// Insert an already-produced local file (e.g. a Pro edit result) directly
    /// as a completed library item. Not enqueued.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_completed_local(
        &self,
        url: &str,
        title: &str,
        platform: &str,
        thumbnail: Option<&str>,
        filename: &str,
        file_path: &str,
        format: &str,
        audio_only: bool,
        duration: Option<f64>,
        file_size: Option<i64>,
    ) -> Result<Download> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads (url, title, platform, thumbnail, filename, file_path, format, \
             audio_only, duration, file_size, status, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'completed', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![
                url,
                title,
                platform,
                thumbnail,
                filename,
                file_path,
                format,
                audio_only as i64,
                duration,
                file_size,
            ],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get_download(id)
    }

    pub fn delete_download(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Remove all completed items from the library/history (files kept on disk).
    pub fn clear_completed(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM downloads WHERE status = 'completed'", [])?;
        Ok(())
    }

    /// File paths of all completed items (for optional on-disk deletion).
    pub fn list_completed_paths(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_path FROM downloads WHERE status = 'completed' AND file_path IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ---------- library ----------

    pub fn search_library(&self, q: &LibraryQuery) -> Result<Vec<Download>> {
        let mut sql = format!("{SELECT_DOWNLOAD} WHERE d.status = 'completed'");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        let needle = q.query.trim();
        if !needle.is_empty() {
            let pattern = format!("%{}%", needle.replace('%', "\\%").replace('_', "\\_"));
            sql.push_str(
                " AND (d.title LIKE ?1 ESCAPE '\\' OR d.filename LIKE ?1 ESCAPE '\\' \
                 OR d.platform LIKE ?1 ESCAPE '\\' OR d.format LIKE ?1 ESCAPE '\\' \
                 OR d.url LIKE ?1 ESCAPE '\\')",
            );
            args.push(Box::new(pattern));
        }
        if let Some(platform) = q.platform.as_ref().filter(|p| !p.is_empty()) {
            sql.push_str(&format!(" AND d.platform = ?{}", args.len() + 1));
            args.push(Box::new(platform.clone()));
        }
        match q.kind.as_str() {
            "audio" => sql.push_str(" AND d.audio_only = 1"),
            "video" => sql.push_str(" AND d.audio_only = 0"),
            _ => {}
        }

        let column = match q.sort.as_str() {
            "title" => "d.title COLLATE NOCASE",
            "size" => "d.file_size",
            "duration" => "d.duration",
            _ => "d.completed_at",
        };
        let direction = if q.descending { "DESC" } else { "ASC" };
        sql.push_str(&format!(" ORDER BY {column} {direction}"));
        if let Some(limit) = q.limit {
            sql.push_str(&format!(" LIMIT {}", limit.max(0)));
        }

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(params_ref.as_slice(), row_to_download)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn list_platforms(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT platform FROM downloads WHERE status = 'completed' AND platform <> '' ORDER BY platform",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ---------- queue ----------

    pub fn enqueue(&self, download_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO queue (download_id, priority, position)
             VALUES (?1, 0, IFNULL((SELECT MAX(position) FROM queue), 0) + 1)
             ON CONFLICT(download_id) DO UPDATE SET status = 'queued'",
            params![download_id],
        )?;
        Ok(())
    }

    pub fn set_queue_status(&self, download_id: i64, status: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE queue SET status = ?1 WHERE download_id = ?2",
            params![status, download_id],
        )?;
        Ok(())
    }

    pub fn remove_from_queue(&self, download_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM queue WHERE download_id = ?1",
            params![download_id],
        )?;
        Ok(())
    }

    /// Next item to schedule, honoring priority then user-defined position.
    pub fn next_queued(&self) -> Result<Option<Download>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "{SELECT_DOWNLOAD} WHERE d.status = 'queued' AND q.status = 'queued' \
             ORDER BY q.priority DESC, q.position ASC LIMIT 1"
        );
        Ok(conn.query_row(&sql, [], row_to_download).optional()?)
    }

    pub fn reorder_queue(&self, ordered_ids: &[i64]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (index, download_id) in ordered_ids.iter().enumerate() {
            tx.execute(
                "UPDATE queue SET position = ?1 WHERE download_id = ?2",
                params![(index as i64) + 1, download_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---------- settings ----------

    pub fn get_settings(&self) -> Result<Settings> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT download_path, theme, language, max_concurrent_downloads, auto_update, \
             notifications, filename_template, organize_by_platform, cookies_browser FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(Settings {
                    download_path: row.get(0)?,
                    theme: row.get(1)?,
                    language: row.get(2)?,
                    max_concurrent_downloads: row.get(3)?,
                    auto_update: row.get::<_, i64>(4)? != 0,
                    notifications: row.get::<_, i64>(5)? != 0,
                    filename_template: row.get(6)?,
                    organize_by_platform: row.get::<_, i64>(7)? != 0,
                    cookies_browser: row.get(8)?,
                })
            },
        )?)
    }

    pub fn update_settings(&self, s: &Settings) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE settings SET download_path = ?1, theme = ?2, language = ?3, \
             max_concurrent_downloads = ?4, auto_update = ?5, notifications = ?6, \
             filename_template = ?7, organize_by_platform = ?8, cookies_browser = ?9 WHERE id = 1",
            params![
                s.download_path,
                s.theme,
                s.language,
                s.max_concurrent_downloads.clamp(1, 10),
                s.auto_update as i64,
                s.notifications as i64,
                s.filename_template,
                s.organize_by_platform as i64,
                s.cookies_browser,
            ],
        )?;
        Ok(())
    }
}
