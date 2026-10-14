-- Initial schema. Source of truth: docs/database_design.md §4.
-- Runtime pragmas (foreign_keys, WAL, busy_timeout) are set on the pool, not here.

CREATE TABLE connections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN (
        'aws', 'supabase', 'minio', 'cloudflare_r2', 'wasabi',
        'digitalocean_spaces', 'custom'
    )),
    endpoint TEXT,
    region TEXT NOT NULL,
    access_key_id TEXT NOT NULL,
    credential_ref TEXT NOT NULL UNIQUE,
    force_path_style INTEGER NOT NULL DEFAULT 0
        CHECK (force_path_style IN (0, 1)),
    allow_insecure_http INTEGER NOT NULL DEFAULT 0
        CHECK (allow_insecure_http IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_connections_name
    ON connections(name COLLATE NOCASE);

CREATE TABLE connection_buckets (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL,
    bucket_name TEXT NOT NULL,
    display_name TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(connection_id, bucket_name),
    FOREIGN KEY (connection_id)
        REFERENCES connections(id)
        ON DELETE CASCADE
);

CREATE TABLE transfers (
    id TEXT PRIMARY KEY,
    connection_id TEXT,
    connection_name_snapshot TEXT NOT NULL,
    direction TEXT NOT NULL
        CHECK (direction IN ('UPLOAD', 'DOWNLOAD')),
    bucket TEXT NOT NULL,
    object_key TEXT NOT NULL,
    local_path TEXT NOT NULL,
    bytes_total INTEGER
        CHECK (bytes_total IS NULL OR bytes_total >= 0),
    bytes_transferred INTEGER NOT NULL DEFAULT 0
        CHECK (bytes_transferred >= 0),
    status TEXT NOT NULL DEFAULT 'QUEUED'
        CHECK (status IN (
            'QUEUED', 'RUNNING', 'COMPLETED', 'FAILED', 'CANCELED'
        )),
    retry_count INTEGER NOT NULL DEFAULT 0
        CHECK (retry_count >= 0),
    error_code TEXT,
    error_message TEXT,
    multipart_upload_id TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    FOREIGN KEY (connection_id)
        REFERENCES connections(id)
        ON DELETE SET NULL
);

CREATE INDEX idx_transfers_status_created
    ON transfers(status, created_at DESC);

CREATE INDEX idx_transfers_connection
    ON transfers(connection_id);

CREATE TABLE transfer_parts (
    id TEXT PRIMARY KEY,
    transfer_id TEXT NOT NULL,
    part_number INTEGER NOT NULL CHECK (part_number > 0),
    etag TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    UNIQUE(transfer_id, part_number),
    FOREIGN KEY (transfer_id)
        REFERENCES transfers(id)
        ON DELETE CASCADE
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
