-- Resumable downloads (TSD §9.4): the object version a partial download
-- belongs to, and how many bytes of its temporary file are known to be on
-- disk (written and fsynced). A retry continues from resume_offset only if
-- the object's ETag still matches resume_etag.
ALTER TABLE transfers ADD COLUMN resume_etag TEXT;
ALTER TABLE transfers ADD COLUMN resume_offset INTEGER NOT NULL DEFAULT 0
    CHECK (resume_offset >= 0);
