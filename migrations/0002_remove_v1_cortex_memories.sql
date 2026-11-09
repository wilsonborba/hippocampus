-- The v2 test migration tool rejects the production database.
-- UUID workspace names identify Cortex v1 user memories (area 6.9).
-- Resource rows and FSM bytes stay intact.
CREATE TEMP TABLE v1_cortex_memory_ids ON COMMIT DROP AS
SELECT id FROM memory
WHERE workspace_id ~* '^([0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$';
DELETE FROM memory_relationship
WHERE source_memory_id IN (SELECT id FROM v1_cortex_memory_ids)
   OR target_memory_id IN (SELECT id FROM v1_cortex_memory_ids);
DELETE FROM memory_embedding WHERE memory_id IN (SELECT id FROM v1_cortex_memory_ids);
DELETE FROM memory_entity WHERE memory_id IN (SELECT id FROM v1_cortex_memory_ids);
DELETE FROM memory_event WHERE memory_id IN (SELECT id FROM v1_cortex_memory_ids);
DELETE FROM memory_resource WHERE memory_id IN (SELECT id FROM v1_cortex_memory_ids);
DELETE FROM memory_tag WHERE memory_id IN (SELECT id FROM v1_cortex_memory_ids);
DELETE FROM memory WHERE id IN (SELECT id FROM v1_cortex_memory_ids);
