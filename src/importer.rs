//! In-app importer: download an edition's `.jsonl.gz` dump and build its SQLite
//! database, reporting [`Progress`] as it goes. Runs off the UI thread.

use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;

use flate2::read::MultiGzDecoder;

use crate::db;
use crate::model::catalog::{self, Edition};
use crate::model::entry::Entry;

/// Progress updates emitted during an install.
#[derive(Debug, Clone)]
pub enum Progress {
    /// Bytes downloaded so far (and total, if the server reported it).
    Downloading { received: u64, total: Option<u64> },
    /// Entries imported into the database so far.
    Importing { entries: u64 },
    /// Install finished successfully.
    Done { entries: u64 },
    /// Install failed.
    Failed(String),
}

/// Download + import an edition, reporting progress. Always ends by reporting
/// either [`Progress::Done`] or [`Progress::Failed`].
pub fn install(edition: &Edition, data_dir: &Path, mut on_progress: impl FnMut(Progress)) {
    match run(edition, data_dir, &mut on_progress) {
        Ok(entries) => on_progress(Progress::Done { entries }),
        Err(e) => on_progress(Progress::Failed(e.to_string())),
    }
}

fn run(
    edition: &Edition,
    data_dir: &Path,
    on_progress: &mut impl FnMut(Progress),
) -> Result<u64, Box<dyn Error>> {
    fs::create_dir_all(data_dir)?;
    let tmp = data_dir.join(format!("{}.gz.tmp", edition.code));
    let db_path = data_dir.join(format!("{}.db", edition.code));

    download(edition.url, &tmp, on_progress)?;
    let count = import_file(&tmp, &db_path, on_progress)?;
    let _ = fs::remove_file(&tmp);
    Ok(count)
}

/// Stream the URL to `dest`, emitting download progress roughly every 4 MB.
fn download(
    url: &str,
    dest: &Path,
    on_progress: &mut impl FnMut(Progress),
) -> Result<(), Box<dyn Error>> {
    let resp = ureq::get(url).call()?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let mut reader = resp.into_body().into_reader();
    let mut file = BufWriter::new(File::create(dest)?);

    let mut buf = vec![0u8; 1 << 16];
    let mut received = 0u64;
    let mut last_emit = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        received += n as u64;
        if received - last_emit >= 4_000_000 {
            on_progress(Progress::Downloading { received, total });
            last_emit = received;
        }
    }
    file.flush()?;
    on_progress(Progress::Downloading { received, total });
    Ok(())
}

/// Parse the gzipped JSONL at `src` into a fresh database at `db_path`,
/// keeping all headword languages and recording per-language counts.
fn import_file(
    src: &Path,
    db_path: &Path,
    on_progress: &mut impl FnMut(Progress),
) -> Result<u64, Box<dyn Error>> {
    let reader = BufReader::with_capacity(1 << 20, MultiGzDecoder::new(File::open(src)?));

    let _ = fs::remove_file(db_path);
    let mut conn = db::open(db_path)?;
    db::init_schema(&conn)?;
    conn.pragma_update(None, "journal_mode", "OFF")?;
    conn.pragma_update(None, "synchronous", "OFF")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;

    let mut counts: HashMap<String, u64> = HashMap::new();
    let mut count = 0u64;

    // Rows are batched into multi-row INSERTs rather than executed one at a
    // time: preparing/stepping the SQLite VM has fixed per-statement
    // overhead, so one statement covering `BATCH_SIZE` rows is noticeably
    // cheaper than `BATCH_SIZE` separate single-row statements, even though
    // both run inside the same transaction. 100 rows * 5 columns = 500 bound
    // parameters, comfortably under SQLite's (even old, pre-3.32) default
    // limit of 999.
    const BATCH_SIZE: usize = 100;

    let tx = conn.transaction()?;
    {
        let single_sql = "INSERT INTO entries (word, word_norm, lang_code, pos, data)
             VALUES (?1, ?2, ?3, ?4, ?5)";
        let mut single_stmt = tx.prepare(single_sql)?;

        let batch_sql = format!(
            "INSERT INTO entries (word, word_norm, lang_code, pos, data) VALUES {}",
            std::iter::repeat("(?,?,?,?,?)")
                .take(BATCH_SIZE)
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut batch_stmt = tx.prepare(&batch_sql)?;

        let mut batch: Vec<[String; 5]> = Vec::with_capacity(BATCH_SIZE);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: Entry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.word.is_empty() || entry.lang_code.is_empty() {
                continue;
            }

            let norm = entry.word.to_lowercase();
            let data = serde_json::to_string(&entry)?;
            *counts.entry(entry.lang_code.clone()).or_default() += 1;
            batch.push([entry.word, norm, entry.lang_code, entry.pos, data]);

            if batch.len() == BATCH_SIZE {
                let flat = batch.iter().flat_map(|row| row.iter().map(String::as_str));
                batch_stmt.execute(rusqlite::params_from_iter(flat))?;
                batch.clear();
            }

            count += 1;
            if count % 50_000 == 0 {
                on_progress(Progress::Importing { entries: count });
            }
        }

        // Flush the remaining rows (fewer than `BATCH_SIZE`) one at a time.
        for row in &batch {
            single_stmt.execute(rusqlite::params![row[0], row[1], row[2], row[3], row[4]])?;
        }
    }
    tx.commit()?;

    // Populate the languages metadata table.
    let tx = conn.transaction()?;
    {
        let mut stmt =
            tx.prepare("INSERT OR REPLACE INTO languages (code, name, count) VALUES (?1, ?2, ?3)")?;
        for (code, c) in &counts {
            stmt.execute(rusqlite::params![code, catalog::lang_name(code), *c as i64])?;
        }
    }
    tx.commit()?;

    db::build_index(&conn)?;
    on_progress(Progress::Importing { entries: count });
    Ok(count)
}
