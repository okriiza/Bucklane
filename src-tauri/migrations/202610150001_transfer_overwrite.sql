-- Records whether the user chose to overwrite an existing destination, so the
-- decision is honored at download finalization and on retry (TSD §6.5.1).
ALTER TABLE transfers ADD COLUMN overwrite INTEGER NOT NULL DEFAULT 0
    CHECK (overwrite IN (0, 1));
