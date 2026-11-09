ALTER TABLE entity ADD COLUMN workspace_id text;
ALTER TABLE entity ADD COLUMN app_id text;
ALTER TABLE entity ADD COLUMN user_id text;
ALTER TABLE entity ADD COLUMN is_self boolean NOT NULL DEFAULT false;
ALTER TABLE entity DROP CONSTRAINT uq_entity_type_normalized_name;
CREATE TEMP TABLE entity_scopes ON COMMIT DROP AS
WITH scopes AS (
 SELECT DISTINCT e.id,m.workspace_id,m.app_id,NULLIF(m.user_id,'') AS user_id
 FROM entity e JOIN memory_entity me ON me.entity_id=e.id JOIN memory m ON m.id=me.memory_id
 UNION ALL
 SELECT e.id,'default','legacy',NULL FROM entity e WHERE NOT EXISTS(SELECT 1 FROM memory_entity WHERE entity_id=e.id)
), numbered AS (
 SELECT *,row_number() OVER (PARTITION BY id ORDER BY workspace_id,app_id,user_id NULLS FIRST) AS n FROM scopes
)
SELECT *,CASE WHEN n=1 THEN id ELSE md5(json_build_array('entity-scope',id,workspace_id,app_id,user_id)::text) END AS new_id FROM numbered;
INSERT INTO entity (id,entity_type,canonical_name,normalized_name,description,status,metadata_json,created_at,updated_at,workspace_id,app_id,user_id,is_self)
SELECT s.new_id,e.entity_type,e.canonical_name,e.normalized_name,e.description,e.status,e.metadata_json,e.created_at,e.updated_at,s.workspace_id,s.app_id,s.user_id,false
FROM entity_scopes s JOIN entity e ON e.id=s.id WHERE s.n>1;
INSERT INTO entity_alias(id,entity_id,alias,normalized_alias,source,confidence,created_at)
SELECT md5(json_build_array('alias-scope',a.id,s.new_id)::text),s.new_id,a.alias,a.normalized_alias,a.source,a.confidence,a.created_at
FROM entity_scopes s JOIN entity_alias a ON a.entity_id=s.id WHERE s.n>1;
UPDATE memory_entity me SET entity_id=s.new_id
FROM entity_scopes s,memory m WHERE me.entity_id=s.id AND me.memory_id=m.id
 AND m.workspace_id=s.workspace_id AND m.app_id=s.app_id AND NULLIF(m.user_id,'') IS NOT DISTINCT FROM s.user_id;
UPDATE entity e SET workspace_id=s.workspace_id,app_id=s.app_id,user_id=s.user_id
FROM entity_scopes s WHERE s.id=e.id AND s.n=1;
ALTER TABLE entity ALTER COLUMN workspace_id SET NOT NULL;
ALTER TABLE entity ALTER COLUMN app_id SET NOT NULL;
CREATE UNIQUE INDEX uq_entity_scope_name ON entity(workspace_id,app_id,coalesce(user_id,''),entity_type,normalized_name);
CREATE INDEX ix_entity_scope ON entity(workspace_id,app_id,user_id);
CREATE UNIQUE INDEX uq_entity_self_user ON entity(app_id,user_id) WHERE is_self AND user_id IS NOT NULL AND user_id!='';
CREATE UNIQUE INDEX uq_entity_self_a2a ON entity(workspace_id,app_id) WHERE is_self AND (user_id IS NULL OR user_id='');
