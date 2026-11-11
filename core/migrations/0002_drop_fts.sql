-- The FTS5 index was never queried: the default tokenizer cannot match inside
-- unspaced CJK text, and every autosave re-indexed the whole body. Search is a
-- case-insensitive substring match (see query.rs) until the note count justifies
-- a trigram-tokenized index.
DROP TRIGGER IF EXISTS note_ai;
DROP TRIGGER IF EXISTS note_ad;
DROP TRIGGER IF EXISTS note_au;
DROP TABLE IF EXISTS note_fts;
