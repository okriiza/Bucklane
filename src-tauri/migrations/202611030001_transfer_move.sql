-- Moves between buckets or connections (TSD §9.5): a COPY transfer with
-- source_delete_after_copy = 1 deletes its source once the copy is verified
-- and the source is unchanged.
ALTER TABLE transfers ADD COLUMN source_delete_after_copy INTEGER NOT NULL DEFAULT 0
    CHECK (source_delete_after_copy IN (0, 1));
