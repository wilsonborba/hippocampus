CREATE TABLE persona_schema (
 schema_name text NOT NULL CHECK(schema_name IN ('person','bot')),
 version bigint NOT NULL CHECK(version>0),
 status text NOT NULL CHECK(status IN ('draft','published')),
 fields jsonb NOT NULL,
 PRIMARY KEY(schema_name,version)
);
ALTER TABLE entity ADD COLUMN document_store text;
ALTER TABLE entity ADD COLUMN document_id text;
