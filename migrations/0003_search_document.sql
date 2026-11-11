ALTER TABLE memory ADD COLUMN search_document tsvector;
CREATE FUNCTION hippo_search_document() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 NEW.search_document := to_tsvector('simple', concat_ws(' ', NEW.title, NEW.summary, NEW.content,
 (SELECT string_agg(t.canonical_name, ' ') FROM memory_tag mt JOIN tag t ON t.id=mt.tag_id WHERE mt.memory_id=NEW.id),
 (SELECT string_agg(e.canonical_name, ' ') FROM memory_entity me JOIN entity e ON e.id=me.entity_id WHERE me.memory_id=NEW.id)));
 RETURN NEW;
END $$;
CREATE TRIGGER memory_search_document BEFORE INSERT OR UPDATE OF title,summary,content ON memory FOR EACH ROW EXECUTE FUNCTION hippo_search_document();
CREATE FUNCTION hippo_search_links() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP != 'INSERT' THEN UPDATE memory SET title=title WHERE id=OLD.memory_id; END IF;
 IF TG_OP != 'DELETE' THEN UPDATE memory SET title=title WHERE id=NEW.memory_id; END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER search_tag_links AFTER INSERT OR UPDATE OR DELETE ON memory_tag FOR EACH ROW EXECUTE FUNCTION hippo_search_links();
CREATE TRIGGER search_entity_links AFTER INSERT OR UPDATE OR DELETE ON memory_entity FOR EACH ROW EXECUTE FUNCTION hippo_search_links();
CREATE FUNCTION hippo_search_names() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_TABLE_NAME='tag' THEN
 UPDATE memory SET title=title WHERE id IN (SELECT memory_id FROM memory_tag WHERE tag_id=NEW.id);
 ELSE
 UPDATE memory SET title=title WHERE id IN (SELECT memory_id FROM memory_entity WHERE entity_id=NEW.id);
 END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER search_tag_names AFTER UPDATE OF canonical_name ON tag FOR EACH ROW EXECUTE FUNCTION hippo_search_names();
CREATE TRIGGER search_entity_names AFTER UPDATE OF canonical_name ON entity FOR EACH ROW EXECUTE FUNCTION hippo_search_names();
UPDATE memory SET title=title;
CREATE INDEX ix_memory_search_document ON memory USING gin(search_document);
CREATE INDEX IF NOT EXISTS ix_memory_relationship_source_memory_id ON memory_relationship(source_memory_id);
CREATE INDEX IF NOT EXISTS ix_memory_relationship_target_memory_id ON memory_relationship(target_memory_id);
