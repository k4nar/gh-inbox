-- Archiving marks the thread done on GitHub, so GitHub stops returning it and
-- full-sync reconciliation (archive_stale) would silently re-archive any row the
-- user unarchived locally. Flag locally-unarchived rows so reconciliation skips
-- them; the flag clears when GitHub returns the thread again (new activity) or
-- when the user archives it again.
ALTER TABLE notifications ADD COLUMN locally_unarchived INTEGER NOT NULL DEFAULT 0;
