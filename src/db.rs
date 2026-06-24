//! SQLite access: schema, open helpers, language-scoped word lookup, and the
//! language list that powers the search-page switcher.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::model::entry::Entry;

/// Open (creating if needed) a writable connection — used by the importer.
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open(path)
}

/// Open an existing database read-only — used by the GUI for lookups.
pub fn open_read_only(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

/// Create the `entries` table and the `languages` metadata table.
/// The lookup index is created separately, after bulk insert (see [`build_index`]).
pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS entries (
            id        INTEGER PRIMARY KEY,
            word      TEXT NOT NULL,
            word_norm TEXT NOT NULL,
            lang_code TEXT NOT NULL,
            pos       TEXT,
            data      TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS languages (
            code  TEXT PRIMARY KEY,
            name  TEXT NOT NULL,
            count INTEGER NOT NULL
         );",
    )
}

/// Build the lookup index. Run after bulk insert — much faster than maintaining
/// it per-row during import.
pub fn build_index(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_lang_word ON entries(lang_code, word_norm);",
    )
}

/// One language present in an edition, with its entry count.
#[derive(Debug, Clone)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
    pub count: i64,
}

/// Read the language list (most populous first) for the switcher.
pub fn list_languages(conn: &Connection) -> rusqlite::Result<Vec<LanguageInfo>> {
    let mut stmt =
        conn.prepare("SELECT code, name, count FROM languages ORDER BY count DESC, name ASC")?;
    let rows = stmt.query_map([], |row| {
        Ok(LanguageInfo {
            code: row.get(0)?,
            name: row.get(1)?,
            count: row.get(2)?,
        })
    })?;
    rows.collect()
}

/// Autocomplete: distinct headwords in a language whose normalized form starts
/// with `prefix`, alphabetical.
///
/// Uses a half-open range (`>=`/`<`) on `word_norm` rather than `LIKE`, so the
/// `(lang_code, word_norm)` index drives both the filter and the ordering and
/// SQLite stops as soon as `limit` rows are found — cost is independent of how
/// many words match overall.
pub fn search_words(
    conn: &Connection,
    lang_code: &str,
    prefix: &str,
    limit: usize,
) -> rusqlite::Result<Vec<String>> {
    let lower = prefix.trim().to_lowercase();
    if lower.is_empty() {
        return Ok(Vec::new());
    }
    // Exclusive upper bound: every string starting with `lower` sorts before
    // `lower` + the maximum code point.
    let upper = format!("{lower}\u{10FFFF}");

    let mut stmt = conn.prepare(
        "SELECT word FROM entries
         WHERE lang_code = ?1 AND word_norm >= ?2 AND word_norm < ?3
         GROUP BY word_norm
         ORDER BY word_norm
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![lang_code, lower, upper, limit as i64],
        |row| row.get::<_, String>(0),
    )?;
    rows.collect()
}

/// Look up all entries for a headword in a given language (case-insensitive).
pub fn lookup(conn: &Connection, lang_code: &str, word: &str) -> rusqlite::Result<Vec<Entry>> {
    let norm = word.trim().to_lowercase();
    let mut stmt =
        conn.prepare("SELECT data FROM entries WHERE lang_code = ?1 AND word_norm = ?2")?;
    let rows = stmt.query_map(rusqlite::params![lang_code, norm], |row| {
        row.get::<_, String>(0)
    })?;

    let mut out = Vec::new();
    for row in rows {
        let data = row?;
        // Skip rows that fail to parse rather than aborting the whole lookup.
        if let Ok(entry) = serde_json::from_str::<Entry>(&data) {
            out.push(entry);
        }
    }
    Ok(out)
}
