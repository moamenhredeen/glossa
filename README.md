# Glossa

An offline, multi-edition dictionary desktop app. Download a Wiktionary edition once, then look up words with no network access — fast, local, and searchable across every headword language that edition defines.

Built in Rust with [`iced`](https://iced.rs/), backed by per-edition SQLite databases sourced from [kaikki.org](https://kaikki.org/) Wiktextract dumps.

## Features

- **Offline lookups.** Each installed edition is a self-contained SQLite database. No network needed after install.
- **Multiple editions.** Install English, French, German, Russian, Chinese, Polish, Dutch, Spanish, Japanese, Italian, or Portuguese — and switch the active one in Settings.
- **In-app installer.** Downloads and builds an edition's database in the background, with live download and import progress.
- **Headword-language switcher.** Each edition contains entries for many languages; pick which one you're searching.
- **Command palette (`Ctrl+K`).** Jump between pages.
- **Word search (`Ctrl+P`).** Prefix autocomplete over the active language's headwords.
- **Cross-reference navigation.** Click links inside an entry to follow them, with Back and breadcrumb history.
- **Custom window chrome.** Frameless window with its own title bar, drag, resize, and controls.

## Requirements

- Rust toolchain (edition 2024 — Rust 1.85+).
- Disk space for editions. Compressed downloads range from ~33 MB (Portuguese) to 2.6 GB (English); the built database is larger.

`rusqlite` is built with the `bundled` feature, so no system SQLite is required.

## Build & Run

```sh
cargo run --release
```

Release mode is recommended — imports parse large JSONL dumps and run much faster optimized.

## Usage

1. Launch the app. On first run no dictionary is installed.
2. Open **Settings** (via `Ctrl+K` → Settings).
3. Pick an edition and **Install**. It downloads and builds the database; progress is shown inline. The first installed edition is activated automatically.
4. Return to **Search**. Use `Ctrl+P` to search words, or the language switcher to change the headword language.
5. Click links in an entry to follow cross-references; use Back / breadcrumbs to retrace.

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| `Ctrl+K` | Toggle command palette (navigate pages) |
| `Ctrl+P` | Toggle word search (autocomplete) |
| `↑` / `↓` | Move selection in an overlay |
| `Enter` | Run selected item |
| `Esc` | Close overlay |

> On macOS, `Cmd` substitutes for `Ctrl`.

## Data layout

App data lives under the OS data directory (`%APPDATA%/glossa` on Windows; the platform data dir elsewhere; falls back to `./data`):

- `<data_dir>/{code}.db` — one SQLite database per installed edition (e.g. `en.db`).
- `<data_dir>/settings.json` — active edition, active language.

Uninstalling an edition deletes its `.db` file.

## Architecture

The crate splits into a GUI binary and a UI-independent library.

- `src/main.rs` — the `iced` application: state, `Message` handling, views, key events, window chrome. The only part that depends on `iced`.
- `src/lib.rs` — library root re-exporting the modules below.
- `src/db.rs` — SQLite schema, read-only/writable open helpers, language-scoped lookup, prefix autocomplete, language list.
- `src/importer.rs` — downloads an edition's `.jsonl.gz` dump and builds its database, streaming `Progress` off the UI thread.
- `src/paths.rs` — on-disk locations for databases and settings.
- `src/model/` — `catalog` (installable editions + language names), `entry` (deserialized dictionary entry), `library` (installed-edition state + persisted settings).
- `src/ui/` — view code: `chrome` (window + title bar), `search`, `settings`, `palette`, `palette` colors.

### How an edition is stored

Each edition's dump is parsed line by line into an `entries` table (`word`, normalized `word_norm`, `lang_code`, `pos`, full JSON `data`). A `languages` table records per-language entry counts for the switcher. The lookup index `(lang_code, word_norm)` is built after bulk insert for speed, and prefix search uses a half-open range over it so cost is independent of how many words match.

## Tests

```sh
cargo test
```

See `tests/db_lookup.rs`.

## Data source & licensing

Dictionary data comes from [kaikki.org](https://kaikki.org/) Wiktextract extracts of Wiktionary. Wiktionary content is licensed under CC BY-SA and GFDL; review those terms for any redistribution.
