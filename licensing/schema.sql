-- Snagreel licensing schema (already applied to the snagreel-licensing D1 database).
-- Kept here so the database can be rebuilt from scratch.

CREATE TABLE IF NOT EXISTS keys (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  key_hash    TEXT    NOT NULL UNIQUE,
  seats       INTEGER NOT NULL DEFAULT 3,
  revoked     INTEGER NOT NULL DEFAULT 0,
  note        TEXT,
  created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS activations (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  key_id      INTEGER NOT NULL REFERENCES keys(id) ON DELETE CASCADE,
  device_id   TEXT    NOT NULL,
  app_version TEXT,
  first_seen  TEXT    NOT NULL DEFAULT (datetime('now')),
  last_seen   TEXT    NOT NULL DEFAULT (datetime('now')),
  UNIQUE (key_id, device_id)
);

CREATE INDEX IF NOT EXISTS idx_activations_key ON activations(key_id);
CREATE INDEX IF NOT EXISTS idx_activations_last_seen ON activations(last_seen);
