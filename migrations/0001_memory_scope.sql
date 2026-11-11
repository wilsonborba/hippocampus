-- Apply only to the isolated test copy before the A25 cutover.
ALTER TABLE memory ADD COLUMN app_id VARCHAR(64);
ALTER TABLE memory ADD COLUMN user_id VARCHAR(128);
UPDATE memory SET app_id = 'legacy';
ALTER TABLE memory ALTER COLUMN app_id SET NOT NULL;
CREATE INDEX idx_memory_scope ON memory (workspace_id, app_id, user_id);
DROP TABLE memory_provenance;
ALTER TABLE memory DROP COLUMN created_by;
ALTER TABLE memory DROP COLUMN source_kind;
