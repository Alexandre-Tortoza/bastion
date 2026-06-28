-- Core pages table. SQLite is the derived index; markdown on disk is truth.
CREATE TABLE IF NOT EXISTS pages (
    id            INTEGER PRIMARY KEY,
    path          TEXT    NOT NULL UNIQUE,
    title         TEXT    NOT NULL DEFAULT '',
    kind          TEXT,              -- paper|concept|method|decision|comparison|synthesis|review|consolidation-proposal
    tier          TEXT,              -- semantic|episodic|working
    updated_at    TEXT,              -- ISO date YYYY-MM-DD (from frontmatter)
    pinned        INTEGER NOT NULL DEFAULT 0,
    status        TEXT,              -- e.g. superseded, ingested, proposed, accepted
    frontmatter   TEXT    NOT NULL DEFAULT '{}',  -- full frontmatter JSON for rich queries
    body          TEXT    NOT NULL DEFAULT '',     -- markdown body (for FTS rebuild)
    indexed_mtime INTEGER NOT NULL DEFAULT 0       -- filesystem mtime at index time (seconds)
);

-- FTS5 full-text index over title + body.
CREATE VIRTUAL TABLE IF NOT EXISTS pages_fts USING fts5(
    title,
    body,
    content     = pages,
    content_rowid = id,
    tokenize    = "unicode61 remove_diacritics 2"
);

-- Keep FTS in sync via triggers.
CREATE TRIGGER IF NOT EXISTS pages_ai AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
END;

CREATE TRIGGER IF NOT EXISTS pages_ad AFTER DELETE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
END;

CREATE TRIGGER IF NOT EXISTS pages_au AFTER UPDATE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
    INSERT INTO pages_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
END;

-- Wikilinks extracted from page bodies.
CREATE TABLE IF NOT EXISTS page_links (
    source_path TEXT NOT NULL,
    target_path TEXT NOT NULL,
    label       TEXT,
    anchor      TEXT,
    PRIMARY KEY (source_path, target_path)
);

CREATE INDEX IF NOT EXISTS page_links_target ON page_links(target_path);

-- Embeddings table (schema only — populated in Phase 3).
CREATE TABLE IF NOT EXISTS embeddings (
    page_id     INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    provider    TEXT    NOT NULL,
    model       TEXT    NOT NULL,
    dim         INTEGER NOT NULL,
    vector      BLOB    NOT NULL,    -- raw f32 little-endian
    content_sha TEXT    NOT NULL,    -- sha256 of body at embed time
    PRIMARY KEY (page_id, provider, model)
);
