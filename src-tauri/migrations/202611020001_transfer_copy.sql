-- Copies between buckets or connections (TSD §9.5): a COPY transfer streams
-- an object from its source (source_* columns) to the destination in
-- connection_id / bucket / object_key. local_path is unused ('') for copies.
--
-- SQLite cannot change a CHECK constraint, so the table is rebuilt. Dropping
-- it cascades to transfer_parts, which is saved and restored around the swap.

CREATE TABLE transfer_parts_backup AS SELECT * FROM transfer_parts;

CREATE TABLE transfers_new (
    id TEXT PRIMARY KEY,
    connection_id TEXT,
    connection_name_snapshot TEXT NOT NULL,
    direction TEXT NOT NULL
        CHECK (direction IN ('UPLOAD', 'DOWNLOAD', 'COPY')),
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
    overwrite INTEGER NOT NULL DEFAULT 0
        CHECK (overwrite IN (0, 1)),
    resume_etag TEXT,
    resume_offset INTEGER NOT NULL DEFAULT 0
        CHECK (resume_offset >= 0),
    source_connection_id TEXT,
    source_connection_name_snapshot TEXT,
    source_bucket TEXT,
    source_key TEXT,
    CHECK (direction <> 'COPY' OR (
        source_connection_name_snapshot IS NOT NULL
        AND source_bucket IS NOT NULL
        AND source_key IS NOT NULL
    )),
    FOREIGN KEY (connection_id)
        REFERENCES connections(id)
        ON DELETE SET NULL,
    FOREIGN KEY (source_connection_id)
        REFERENCES connections(id)
        ON DELETE SET NULL
);

INSERT INTO transfers_new (
    id, connection_id, connection_name_snapshot, direction, bucket, object_key,
    local_path, bytes_total, bytes_transferred, status, retry_count, error_code,
    error_message, multipart_upload_id, created_at, started_at, completed_at,
    overwrite, resume_etag, resume_offset
)
SELECT
    id, connection_id, connection_name_snapshot, direction, bucket, object_key,
    local_path, bytes_total, bytes_transferred, status, retry_count, error_code,
    error_message, multipart_upload_id, created_at, started_at, completed_at,
    overwrite, resume_etag, resume_offset
FROM transfers;

DROP TABLE transfers;
ALTER TABLE transfers_new RENAME TO transfers;

CREATE INDEX idx_transfers_status_created
    ON transfers(status, created_at DESC);

CREATE INDEX idx_transfers_connection
    ON transfers(connection_id);

CREATE INDEX idx_transfers_source_connection
    ON transfers(source_connection_id);

INSERT INTO transfer_parts SELECT * FROM transfer_parts_backup;
DROP TABLE transfer_parts_backup;
