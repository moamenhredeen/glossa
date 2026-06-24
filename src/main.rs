//! Glossa — offline multi-edition dictionary.
//!
//! Two pages: Search (within the active edition, with a headword-language
//! switcher) and Settings (install / uninstall editions, pick the active one).
//! Installs download + build a per-edition SQLite database in-app, with progress.

mod ui;

use std::collections::HashMap;

use iced::event::{self, Event, Status};
use iced::keyboard::{self, key::Named, Key};
use iced::widget::{column, combo_box, container, stack};
use iced::{window, Element, Length, Subscription, Task};
use rusqlite::Connection;

use glossa::db::{self, LanguageInfo};
use glossa::importer::{self, Progress};
use glossa::model::catalog;
use glossa::model::entry::Entry;
use glossa::model::library::Library;
use glossa::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Search,
    Settings,
}

/// Which overlay (if any) is open. Both share one query/selection/list UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    /// Command palette (Ctrl+K) — navigate between pages.
    Command,
    /// Word search (Ctrl+P) — autocomplete dictionary words.
    Word,
}

/// Transient per-edition install state (shown on the Settings page).
#[derive(Debug, Clone)]
pub enum InstallState {
    Downloading { received: u64, total: Option<u64> },
    Importing { entries: u64 },
    Failed(String),
}

pub struct App {
    screen: Screen,
    library: Library,
    /// Read-only connection to the active edition's DB.
    conn: Option<Connection>,
    languages: Vec<LanguageInfo>,
    active_lang: Option<String>,
    /// Searchable language switcher state + its currently selected option.
    lang_state: combo_box::State<ui::search::Lang>,
    lang_selected: Option<ui::search::Lang>,
    query: String,
    results: Vec<Entry>,
    status: Option<String>,
    /// The (word, language) currently displayed (last successful lookup).
    current_view: Option<(String, String)>,
    /// Previously viewed (word, language) pairs, for the Back button.
    history: Vec<(String, String)>,
    /// In-flight installs keyed by edition code.
    installs: HashMap<String, InstallState>,
    // Overlay (command palette / word search)
    overlay: Overlay,
    palette_query: String,
    palette_selected: usize,
    /// Word-search autocomplete suggestions (Word overlay only).
    word_suggestions: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Screen),
    Back,
    GoToCrumb(usize),
    // Search page
    LangSelected(String),
    LinkClicked(String),
    // Settings page
    Install(String),
    Uninstall(String),
    SetActiveEdition(String),
    InstallProgress(String, Progress),
    // Overlays (command palette / word search)
    TogglePalette,
    ToggleWordSearch,
    ClosePalette,
    PaletteAppend(String),
    PaletteBackspace,
    PaletteMoveSelection(i32),
    PaletteRun,
    PaletteRunIndex(usize),
    // Window chrome
    WindowDrag,
    WindowResize(window::Direction),
    WindowMinimize,
    WindowMaximize,
    WindowClose,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            screen: Screen::Search,
            library: Library::load(),
            conn: None,
            languages: Vec::new(),
            active_lang: None,
            lang_state: combo_box::State::new(Vec::new()),
            lang_selected: None,
            query: String::new(),
            results: Vec::new(),
            status: None,
            current_view: None,
            history: Vec::new(),
            installs: HashMap::new(),
            overlay: Overlay::None,
            palette_query: String::new(),
            palette_selected: 0,
            word_suggestions: Vec::new(),
        };
        app.open_active();
        app
    }

    /// Open the active edition's database read-only and load its languages.
    fn open_active(&mut self) {
        self.conn = None;
        self.languages.clear();
        self.results.clear();
        self.lang_state = combo_box::State::new(Vec::new());
        self.lang_selected = None;

        let Some(edition) = self.library.active_edition() else {
            self.status = Some("No dictionary installed. Open Settings to install one.".into());
            return;
        };

        match db::open_read_only(&paths::db_path(edition.code)) {
            Ok(conn) => {
                self.languages = db::list_languages(&conn).unwrap_or_default();
                // Pick active language: saved (if still valid), else the edition's
                // own language, else the most populous.
                let saved = self.library.active_lang().map(str::to_string);
                self.active_lang = saved
                    .filter(|l| self.languages.iter().any(|li| &li.code == l))
                    .or_else(|| {
                        self.languages
                            .iter()
                            .find(|li| li.code == edition.code)
                            .map(|li| li.code.clone())
                    })
                    .or_else(|| self.languages.first().map(|li| li.code.clone()));
                self.lang_state = combo_box::State::new(ui::search::lang_options(&self.languages));
                self.lang_selected = self
                    .active_lang
                    .as_deref()
                    .and_then(|c| ui::search::find_lang(&self.languages, c));
                self.conn = Some(conn);
                self.status = None;
            }
            Err(e) => self.status = Some(format!("Failed to open dictionary: {e}")),
        }
    }

    /// Look a word up in an explicit language and show the results.
    /// Does not touch `active_lang` or the history.
    fn lookup(&mut self, word: &str, lang: &str) {
        let Some(conn) = &self.conn else {
            return;
        };
        let w = word.trim();
        if w.is_empty() {
            self.results.clear();
            self.status = None;
            return;
        }
        match db::lookup(conn, lang, w) {
            Ok(r) if r.is_empty() => {
                self.results.clear();
                self.status = Some(format!("No results for \u{201C}{w}\u{201D}"));
            }
            Ok(r) => {
                self.results = r;
                self.status = None;
                self.current_view = Some((w.to_string(), lang.to_string()));
            }
            Err(e) => {
                self.results.clear();
                self.status = Some(format!("Lookup error: {e}"));
            }
        }
    }

    /// Start a fresh word search (from Ctrl+P): clears the navigation trail.
    fn open_word(&mut self, word: String, lang: String) {
        self.history.clear();
        self.query = word.clone();
        self.screen = Screen::Search;
        self.lookup(&word, &lang);
    }

    /// Jump to a breadcrumb entry, dropping everything after it.
    fn go_to_crumb(&mut self, index: usize) {
        if index < self.history.len() {
            let (word, lang) = self.history[index].clone();
            self.history.truncate(index);
            self.query = word.clone();
            self.lookup(&word, &lang);
        }
    }

    /// Refresh word-search autocomplete from the current palette query.
    fn refresh_word_suggestions(&mut self) {
        self.word_suggestions.clear();
        let (Some(conn), Some(lang)) = (&self.conn, &self.active_lang) else {
            return;
        };
        self.word_suggestions =
            db::search_words(conn, lang, &self.palette_query, 25).unwrap_or_default();
    }

    /// Navigate to a word in a given language: remember the current view for
    /// Back, then look it up.
    fn navigate(&mut self, word: String, lang: String) {
        let trimmed = word.trim().to_string();
        if !trimmed.is_empty()
            && self.current_view.as_ref().map(|(w, _)| w.as_str()) != Some(trimmed.as_str())
        {
            if let Some(cur) = &self.current_view {
                self.history.push(cur.clone());
            }
        }
        self.query = word.clone();
        self.screen = Screen::Search;
        self.lookup(&word, &lang);
    }
}

/// Spawn the download+import on a worker thread and stream its progress as messages.
fn install_task(code: String) -> Task<Message> {
    let Some(edition) = catalog::edition(&code) else {
        return Task::none();
    };
    let data_dir = paths::data_dir();
    let (tx, rx) = iced::futures::channel::mpsc::unbounded();
    std::thread::spawn(move || {
        importer::install(edition, &data_dir, |p| {
            let _ = tx.unbounded_send(p);
        });
    });
    Task::run(rx, move |p| Message::InstallProgress(code.clone(), p))
}

/// Number of items in the currently open overlay's list.
fn overlay_len(app: &App) -> usize {
    match app.overlay {
        Overlay::Command => ui::palette::matches(&app.palette_query).len(),
        Overlay::Word => app.word_suggestions.len(),
        Overlay::None => 0,
    }
}

/// Run the selected item of the open overlay.
fn run_palette(app: &mut App, index: usize) {
    match app.overlay {
        Overlay::Command => {
            let results = ui::palette::matches(&app.palette_query);
            if let Some(cmd) = results.get(index) {
                app.screen = cmd.target;
                app.overlay = Overlay::None;
            }
        }
        Overlay::Word => {
            if let Some(word) = app.word_suggestions.get(index).cloned() {
                let lang = app.active_lang.clone().unwrap_or_default();
                app.overlay = Overlay::None;
                app.open_word(word, lang);
            }
        }
        Overlay::None => {}
    }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Navigate(s) => app.screen = s,
        Message::GoToCrumb(i) => app.go_to_crumb(i),
        Message::LangSelected(l) => {
            app.library.set_active_lang(&l);
            app.active_lang = Some(l.clone());
            app.lang_selected = ui::search::find_lang(&app.languages, &l);
            let q = app.query.clone();
            app.lookup(&q, &l);
        }
        Message::LinkClicked(w) => {
            // Links inside explanations always point to words in the edition's
            // gloss language, so look them up there — without changing the user's
            // selected headword language.
            if let Some(edition) = app.library.active_edition() {
                let gloss = edition.code.to_string();
                app.navigate(w, gloss);
            }
        }
        Message::Back => {
            if let Some((word, lang)) = app.history.pop() {
                app.query = word.clone();
                app.screen = Screen::Search;
                app.lookup(&word, &lang);
            }
        }
        Message::Install(code) => {
            app.installs.insert(
                code.clone(),
                InstallState::Downloading {
                    received: 0,
                    total: None,
                },
            );
            return install_task(code);
        }
        Message::Uninstall(code) => {
            let _ = std::fs::remove_file(paths::db_path(&code));
            let was_active = app.library.active_edition().map(|e| e.code) == Some(code.as_str());
            app.installs.remove(&code);
            app.library.rescan();
            if was_active {
                app.library.clear_active_edition();
                app.open_active();
            }
        }
        Message::SetActiveEdition(code) => {
            app.library.set_active_edition(&code);
            app.open_active();
        }
        Message::InstallProgress(code, prog) => match prog {
            Progress::Downloading { received, total } => {
                app.installs
                    .insert(code, InstallState::Downloading { received, total });
            }
            Progress::Importing { entries } => {
                app.installs.insert(code, InstallState::Importing { entries });
            }
            Progress::Failed(e) => {
                app.installs.insert(code, InstallState::Failed(e));
            }
            Progress::Done { .. } => {
                app.installs.remove(&code);
                app.library.rescan();
                // Auto-activate if nothing is active yet.
                if app.library.active_edition().is_none() {
                    app.library.set_active_edition(&code);
                    app.open_active();
                }
            }
        },
        Message::TogglePalette => {
            app.overlay = if app.overlay == Overlay::Command {
                Overlay::None
            } else {
                Overlay::Command
            };
            app.palette_query.clear();
            app.palette_selected = 0;
            app.word_suggestions.clear();
        }
        Message::ToggleWordSearch => {
            app.overlay = if app.overlay == Overlay::Word {
                Overlay::None
            } else {
                Overlay::Word
            };
            app.palette_query.clear();
            app.palette_selected = 0;
            app.word_suggestions.clear();
        }
        Message::ClosePalette => app.overlay = Overlay::None,
        Message::PaletteAppend(s) => {
            if app.overlay != Overlay::None {
                app.palette_query.push_str(&s);
                app.palette_selected = 0;
                if app.overlay == Overlay::Word {
                    app.refresh_word_suggestions();
                }
            }
        }
        Message::PaletteBackspace => {
            if app.overlay != Overlay::None {
                app.palette_query.pop();
                app.palette_selected = 0;
                if app.overlay == Overlay::Word {
                    app.refresh_word_suggestions();
                }
            }
        }
        Message::PaletteMoveSelection(d) => {
            let len = overlay_len(app);
            if len > 0 {
                app.palette_selected =
                    (app.palette_selected as i32 + d).rem_euclid(len as i32) as usize;
            }
        }
        Message::PaletteRun => run_palette(app, app.palette_selected),
        Message::PaletteRunIndex(i) => run_palette(app, i),
        Message::WindowDrag => {
            return window::latest().then(|id| match id {
                Some(id) => window::drag(id),
                None => Task::none(),
            });
        }
        Message::WindowResize(dir) => {
            return window::latest().then(move |id| match id {
                Some(id) => window::drag_resize(id, dir),
                None => Task::none(),
            });
        }
        Message::WindowMinimize => {
            return window::latest().then(|id| match id {
                Some(id) => window::minimize(id, true),
                None => Task::none(),
            });
        }
        Message::WindowMaximize => {
            return window::latest().then(|id| match id {
                Some(id) => window::toggle_maximize(id),
                None => Task::none(),
            });
        }
        Message::WindowClose => {
            return window::latest().then(|id| match id {
                Some(id) => window::close(id),
                None => Task::none(),
            });
        }
    }
    Task::none()
}

fn view(app: &App) -> Element<'_, Message> {
    let body = match app.screen {
        Screen::Search => ui::search::view(app),
        Screen::Settings => ui::settings::view(app),
    };
    let content = container(body)
        .width(Length::Fill)
        .height(Length::Fill);

    let lang_selector = ui::search::language_selector(app);
    let inner = column![
        ui::chrome::title_bar(app.screen, !app.history.is_empty(), lang_selector),
        content
    ];
    let base = ui::chrome::window(inner.into());

    match app.overlay {
        Overlay::None => base,
        Overlay::Command => {
            let items: Vec<String> = ui::palette::matches(&app.palette_query)
                .iter()
                .map(|c| c.label.to_string())
                .collect();
            let overlay =
                ui::palette::view(&app.palette_query, app.palette_selected, &items, "Type a command…");
            stack![base, overlay].into()
        }
        Overlay::Word => {
            let overlay = ui::palette::view(
                &app.palette_query,
                app.palette_selected,
                &app.word_suggestions,
                "Search a word…",
            );
            stack![base, overlay].into()
        }
    }
}

/// Map raw key events to palette messages. Ctrl/Cmd+K toggles the palette;
/// other keys drive it (the `update` handlers ignore them unless it's open).
fn on_event(event: Event, _status: Status, _id: window::Id) -> Option<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modifiers,
        text,
        ..
    }) = event
    else {
        return None;
    };

    if modifiers.command() {
        if let Key::Character(c) = &key {
            match c.as_str().to_ascii_lowercase().as_str() {
                "k" => return Some(Message::TogglePalette),
                "p" => return Some(Message::ToggleWordSearch),
                _ => {}
            }
        }
        return None;
    }

    match key {
        Key::Named(Named::Escape) => Some(Message::ClosePalette),
        Key::Named(Named::Enter) => Some(Message::PaletteRun),
        Key::Named(Named::ArrowDown) => Some(Message::PaletteMoveSelection(1)),
        Key::Named(Named::ArrowUp) => Some(Message::PaletteMoveSelection(-1)),
        Key::Named(Named::Backspace) => Some(Message::PaletteBackspace),
        _ => text.map(|t| Message::PaletteAppend(t.to_string())),
    }
}

fn subscription(_app: &App) -> Subscription<Message> {
    event::listen_with(on_event)
}

fn main() -> iced::Result {
    iced::application(App::new, update, view)
        .title("Glossa")
        .decorations(false)
        .transparent(true)
        .subscription(subscription)
        .run()
}
