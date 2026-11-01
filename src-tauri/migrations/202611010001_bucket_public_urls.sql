-- Base URL (custom domain or CDN) that serves a bucket's public objects, used
-- by "Copy public URL" instead of the S3 endpoint address.
CREATE TABLE bucket_public_urls (
    connection_id TEXT NOT NULL,
    bucket_name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (connection_id, bucket_name),
    FOREIGN KEY (connection_id)
        REFERENCES connections(id)
        ON DELETE CASCADE
);
