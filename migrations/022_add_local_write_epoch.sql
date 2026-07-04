-- A background sync upserts a snapshot taken at fetch time; if the user marks a
-- notification read (or archives it) while the sync is in flight, the stale
-- snapshot would flip the flags back. Record when the last local mutation
-- happened so the sync upsert can keep local unread/archived state whenever the
-- local write is newer than the sync's start time. NULL = no pending local write.
ALTER TABLE notifications ADD COLUMN local_write_epoch INTEGER;
