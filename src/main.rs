//! Glossa — offline multi-edition dictionary.

// Hide the console window on Windows for release builds (GUI app, no terminal).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;

use iced::alignment::{Horizontal, Vertical};
use iced::border::Radius;
use iced::event::Event;
use iced::event::{self, Status};
use iced::keyboard::key::Named;
use iced::keyboard::Key;
use iced::widget::{
    button, column, combo_box, container, operation, progress_bar, rich_text, row, scrollable,
    span, stack, text, text_input, Space,
};
use iced::{
    color, font, keyboard, window, Border, Color, Element, Font, Length, Padding, Subscription,
    Task, Theme,
};
use rusqlite::Connection;

use glossa::db::{self, LanguageInfo};
use glossa::importer::{self, Progress};
use glossa::model::catalog;
use glossa::model::entry::Entry;
use glossa::model::library::Library;
use glossa::paths;

use glossa::model::catalog::EDITIONS;

const MUTED: Color = color!(0x888888);
const ACCENT: Color = color!(0x2a9d8f); // links / related words — the only colored thing

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Glossa")
        .theme(App::theme)
        .window(window::Settings {
            // decorations: false,
            // transparent: true,
            icon: window::icon::from_file_data(
                include_bytes!("../assets/icons/glossa-icon-2.png"),
                None,
            )
            .ok(),
            ..Default::default()
        })
        .subscription(subscription)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Search,
    Result,
    Settings,
}

/// Transient per-edition install state (shown on the Settings page).
#[derive(Debug, Clone)]
pub enum InstallState {
    Downloading { received: u64, total: Option<u64> },
    Importing { entries: u64 },
    Failed(String),
}

/// A selectable headword language for the pick_list (name only).
#[derive(Debug, Clone, PartialEq)]
pub struct Lang {
    pub code: String,
    label: String,
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

pub struct App {
    screen: Screen,
    library: Library,
    search_word: String,
    /// Read-only connection to the active edition's DB.
    conn: Option<Connection>,
    languages: Vec<LanguageInfo>,
    active_lang: Option<String>,
    /// Searchable language switcher state + its currently selected option.
    lang_state: combo_box::State<Lang>,
    lang_selected: Option<Lang>,
    /// Live autocomplete matches for the text currently in the search box.
    suggestions: Vec<String>,
    /// Index into `suggestions` highlighted by arrow-key navigation; the
    /// first match is always selected by default.
    selected_suggestion: usize,
    results: Vec<Entry>,
    status: Option<String>,
    /// The (word, language) currently displayed (last successful lookup).
    current_view: Option<(String, String)>,
    /// Previously viewed (word, language) pairs, for the Back button.
    history: Vec<(String, String)>,
    /// In-flight installs keyed by edition code.
    installs: HashMap<String, InstallState>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SearchInputChanged(String),
    Navigate(Screen),
    Back,
    GoToCrumb(usize),
    // Search page
    LangSelected(String),
    LinkClicked(String),
    SuggestionClicked(String),
    SuggestionUp,
    SuggestionDown,
    // Settings page
    Install(String),
    Uninstall(String),
    SetActiveEdition(String),
    InstallProgress(String, Progress),
    SearchSubmitted,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            screen: Screen::Search,
            search_word: String::new(),
            library: Library::load(),
            conn: None,
            languages: Vec::new(),
            active_lang: None,
            lang_state: combo_box::State::new(Vec::new()),
            lang_selected: None,
            suggestions: Vec::new(),
            selected_suggestion: 0,
            results: Vec::new(),
            status: None,
            current_view: None,
            history: Vec::new(),
            installs: HashMap::new(),
        };
        app.open_active();
        app
    }

    /// Open the active edition's database read-only and load its languages.
    fn open_active(&mut self) {
        self.conn = None;
        self.languages.clear();
        self.results.clear();
        self.suggestions.clear();
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
                self.lang_state = combo_box::State::new(lang_options(&self.languages));
                self.lang_selected = self
                    .active_lang
                    .as_deref()
                    .and_then(|c| find_lang(&self.languages, c));
                self.conn = Some(conn);
                self.status = None;
            }
            Err(e) => self.status = Some(format!("Failed to open dictionary: {e}")),
        }
    }

    /// Run a lookup for `word` in `lang` and populate `results`/`status`.
    fn run_lookup(&mut self, word: &str, lang: &str) {
        let Some(conn) = &self.conn else {
            return;
        };
        match db::lookup(conn, lang, word) {
            Ok(r) if r.is_empty() => {
                self.results.clear();
                self.status = Some(format!("No results for \u{201C}{word}\u{201D}"));
            }
            Ok(r) => {
                self.results = r;
                self.status = None;
            }
            Err(e) => {
                self.results.clear();
                self.status = Some(format!("Lookup error: {e}"));
            }
        }
    }

    /// Look up the word typed on the Search screen, in the active headword
    /// language. This starts a fresh navigation trail.
    fn lookup(&mut self) {
        let w = self.search_word.trim().to_string();
        if w.is_empty() {
            self.results.clear();
            self.status = None;
            return;
        }
        if self.conn.is_none() {
            return;
        }
        let lang = self.active_lang.clone().unwrap_or_else(|| "en".to_string());
        self.history.clear();
        self.run_lookup(&w, &lang);
        self.current_view = Some((w, lang));
    }

    /// Refresh the live autocomplete list shown under the search box.
    fn refresh_suggestions(&mut self) {
        let word = self.search_word.trim();
        let Some(conn) = (if word.is_empty() { None } else { self.conn.as_ref() }) else {
            self.suggestions.clear();
            self.selected_suggestion = 0;
            return;
        };
        let lang = self.active_lang.clone().unwrap_or_else(|| "en".to_string());
        self.suggestions = db::search_words(conn, &lang, word, 8).unwrap_or_default();
        self.selected_suggestion = 0;
    }

    /// Navigate to `word` in `lang` from within the result view (e.g. a
    /// related-word link), pushing the word currently on display onto the
    /// back history.
    fn navigate(&mut self, word: String, lang: String) {
        if self.conn.is_none() {
            return;
        }
        if let Some(prev) = self.current_view.take() {
            self.history.push(prev);
        }
        self.run_lookup(&word, &lang);
        self.current_view = Some((word, lang));
    }

    fn theme(&self) -> Theme {
        Theme::Dracula
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SearchInputChanged(input) => {
                self.search_word = input;
                // Installed-dictionary hints aside, don't let a stale result
                // message linger while the user is typing a new query.
                if self.conn.is_some() {
                    self.status = None;
                }
                self.refresh_suggestions();
            }
            Message::SearchSubmitted => {
                // With suggestions showing, submit always picks the
                // highlighted one (the first match by default) rather than
                // whatever partial text is still in the box.
                if let Some(word) = self.suggestions.get(self.selected_suggestion).cloned() {
                    self.search_word = word;
                }
                self.screen = Screen::Result;
                self.lookup();
                self.search_word.clear();
                self.suggestions.clear();
            }
            Message::SuggestionClicked(word) => {
                self.search_word = word;
                self.screen = Screen::Result;
                self.lookup();
                self.search_word.clear();
                self.suggestions.clear();
            }
            Message::SuggestionUp => {
                self.selected_suggestion = self.selected_suggestion.saturating_sub(1);
            }
            Message::SuggestionDown => {
                if self.selected_suggestion + 1 < self.suggestions.len() {
                    self.selected_suggestion += 1;
                }
            }
            Message::Navigate(s) => {
                if s == self.screen {
                    return Task::none();
                }
                self.screen = s;
                if s == Screen::Search {
                    return operation::focus("search_input");
                }
            }
            Message::LangSelected(l) => {
                self.library.set_active_lang(&l);
                self.active_lang = Some(l.clone());
                self.lang_selected = find_lang(&self.languages, &l);
                // Re-run whatever's currently on display in the new language.
                if let Some((word, _)) = self.current_view.clone() {
                    self.run_lookup(&word, &l);
                    self.current_view = Some((word, l));
                }
            }
            Message::LinkClicked(word) => {
                // Links inside explanations always point to words in the edition's
                // gloss language, so look them up there — without changing the user's
                // selected headword language.
                let gloss = self
                    .library
                    .active_edition()
                    .map(|e| e.code.to_string())
                    .unwrap_or_else(|| "en".to_string());
                self.navigate(word, gloss);
            }
            Message::Back => {
                if let Some((word, lang)) = self.history.pop() {
                    self.current_view = Some((word.clone(), lang.clone()));
                    self.run_lookup(&word, &lang);
                } else {
                    self.screen = Screen::Search;
                    return operation::focus("search_input");
                }
            }
            Message::GoToCrumb(i) => {
                if i < self.history.len() {
                    let (word, lang) = self.history[i].clone();
                    self.history.truncate(i);
                    self.current_view = Some((word.clone(), lang.clone()));
                    self.run_lookup(&word, &lang);
                }
            }
            Message::Install(code) => {
                self.installs.insert(
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
                let was_active =
                    self.library.active_edition().map(|e| e.code) == Some(code.as_str());
                self.installs.remove(&code);
                self.library.rescan();
                if was_active {
                    self.library.clear_active_edition();
                    self.open_active();
                }
            }
            Message::SetActiveEdition(code) => {
                self.library.set_active_edition(&code);
                self.open_active();
            }
            Message::InstallProgress(code, prog) => match prog {
                Progress::Downloading { received, total } => {
                    self.installs
                        .insert(code, InstallState::Downloading { received, total });
                }
                Progress::Importing { entries } => {
                    self.installs
                        .insert(code, InstallState::Importing { entries });
                }
                Progress::Failed(e) => {
                    self.installs.insert(code, InstallState::Failed(e));
                }
                Progress::Done { .. } => {
                    self.installs.remove(&code);
                    self.library.rescan();
                    // Auto-activate if nothing is active yet.
                    if self.library.active_edition().is_none() {
                        self.library.set_active_edition(&code);
                        self.open_active();
                    }
                }
            },
        }
        Task::none()
    }

    fn search_view(&self) -> Element<'_, Message> {
        let mut col = column![].spacing(25).max_width(600);

        // Only worth showing when the active edition actually has more than
        // one headword language to choose between.
        if self.languages.len() > 1 {
            col = col.push(
                container(
                    combo_box(
                        &self.lang_state,
                        "Language",
                        self.lang_selected.as_ref(),
                        |l: Lang| Message::LangSelected(l.code),
                    )
                    .size(13)
                    .padding(4)
                    .input_style(input_style)
                    .width(140),
                )
                .align_x(Horizontal::Center)
                .width(Length::Fill),
            );
        }

        col = col.push(
            text_input("type a word", &self.search_word)
                .id("search_input")
                .padding(10)
                .on_input(Message::SearchInputChanged)
                .on_submit(Message::SearchSubmitted),
        );

        if !self.suggestions.is_empty() {
            let mut sug_col = column![];
            for (i, word) in self.suggestions.iter().enumerate() {
                if i > 0 {
                    sug_col = sug_col.push(divider());
                }
                let is_selected = i == self.selected_suggestion;
                sug_col = sug_col.push(
                    button(text(word.clone()).size(14))
                        .style(move |theme: &Theme, status: button::Status| {
                            let mut style = button::text(theme, status);
                            if is_selected {
                                style.background =
                                    Some(theme.extended_palette().background.weak.color.into());
                            }
                            style
                        })
                        .padding(Padding::from([6, 10]))
                        .width(Length::Fill)
                        .on_press(Message::SuggestionClicked(word.clone())),
                );
            }
            col = col.push(sug_col);
        }

        if let Some(status) = &self.status {
            col = col.push(text(status.clone()).size(13).color(MUTED));
        }

        let settings_button = container(
            button(text("\u{2699}").size(24).color(MUTED))
                .style(button::text)
                .on_press(Message::Navigate(Screen::Settings)),
        )
        .padding(16)
        .align_x(Horizontal::Right)
        .width(Length::Fill)
        .height(Length::Fill);

        // Anchored from the top with a fixed offset rather than vertically
        // centered: a column lays out children top-down from that fixed
        // point, so the input's position stays put as the suggestion list
        // grows or shrinks below it. True vertical centering would recompute
        // the offset from the total (variable) height on every keystroke.
        let anchored = container(col)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .padding(Padding::from([160, 0]));

        stack![
            container(anchored).width(Length::Fill).height(Length::Fill),
            settings_button
        ]
        .into()
    }

    fn result_view(&self) -> Element<'_, Message> {
        let header = row![
            button(text("\u{2039} Back").size(13))
                .style(button::text)
                .padding(0)
                .on_press(Message::Back),
            breadcrumb(self),
        ]
        .spacing(12)
        .align_y(Vertical::Center)
        .padding(Padding::from([12, 20]));

        let body = if self.results.is_empty() {
            let message = self
                .status
                .clone()
                .unwrap_or_else(|| "No matching words found".to_string());
            centered_hint(message)
        } else {
            let mut col = column![].spacing(28).padding(Padding::from([0, 20]));
            for (i, entry) in self.results.iter().enumerate() {
                if i > 0 {
                    col = col.push(divider());
                }
                col = col.push(entry_view(entry));
            }
            scrollable(col.padding(24)).height(Length::Fill).into()
        };

        container(column![header, body])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let mut col = column![].padding(24).spacing(24);

        let active = self.library.active_edition().map(|e| e.code);

        for edition in EDITIONS {
            let installed = self.library.is_installed(edition.code);
            let installing = self.installs.get(edition.code);

            // Left: name + size/status.
            let title = text(edition.name).size(18);
            let subtitle = text(if installed {
                "Installed".to_string()
            } else {
                format!("{} download", edition.size)
            })
            .size(13);

            // Right: action area depends on state.
            let action: Element<Message> = if let Some(state) = installing {
                install_status(state)
            } else if installed {
                let mut r = row![].spacing(8);
                if active == Some(edition.code) {
                    r = r.push(text("Active").size(14));
                } else {
                    r = r.push(
                        button("Set active")
                            .on_press(Message::SetActiveEdition(edition.code.to_string())),
                    );
                }
                r = r.push(
                    button("Uninstall").on_press(Message::Uninstall(edition.code.to_string())),
                );
                r.into()
            } else {
                button("Install")
                    .on_press(Message::Install(edition.code.to_string()))
                    .into()
            };

            let entry_row = row![
                column![title, subtitle].spacing(2).width(Length::Fill),
                action,
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center);

            col = col.push(entry_row);
        }

        let header = row![
            button(text("\u{2039}").size(24))
                .style(button::text)
                .padding(5)
                .on_press(Message::Navigate(Screen::Search)),
            text("Dictionaries").size(24),
        ]
        .spacing(12)
        .align_y(Vertical::Center)
        .padding(24);

        column![header, scrollable(col)].spacing(24).into()
    }

    fn view(&self) -> Element<'_, Message> {
        let body = match self.screen {
            Screen::Search => self.search_view(),
            Screen::Result => self.result_view(),
            Screen::Settings => self.settings_view(),
        };
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
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

fn subscription(_app: &App) -> Subscription<Message> {
    event::listen_with(|event: Event, _status: Status, _id: window::Id| {
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
                    "," => return Some(Message::Navigate(Screen::Settings)),
                    _ => {}
                }
            }
            return None;
        }

        match key {
            Key::Named(Named::Escape) => Some(Message::Navigate(Screen::Search)),
            Key::Named(Named::ArrowUp) => Some(Message::SuggestionUp),
            Key::Named(Named::ArrowDown) => Some(Message::SuggestionDown),
            _ => None,
        }
    })
}

fn install_status(state: &InstallState) -> Element<'_, Message> {
    match state {
        InstallState::Downloading { received, total } => {
            let label = match total {
                Some(t) if *t > 0 => {
                    format!("Downloading {:.0}%", (*received as f64 / *t as f64) * 100.0)
                }
                _ => format!("Downloading {:.1} MB", *received as f64 / 1_000_000.0),
            };
            let bar = match total {
                Some(t) if *t > 0 => progress_bar(0.0..=*t as f32, *received as f32),
                _ => progress_bar(0.0..=1.0, 0.0),
            };
            column![text(label).size(13), container(bar).width(200)]
                .spacing(4)
                .into()
        }
        InstallState::Importing { entries } => column![
            text(format!("Importing… {entries} entries")).size(13),
            container(progress_bar(0.0..=1.0, 0.5)).width(200),
        ]
        .spacing(4)
        .into(),
        InstallState::Failed(e) => text(format!("Failed: {e}")).size(13).into(),
    }
}

/// Navigation trail: each previous word is a clickable crumb; the current word
/// is shown plain at the end.
fn breadcrumb(app: &App) -> Element<'_, Message> {
    let mut trail = row![].spacing(6).align_y(Vertical::Center);
    for (i, (word, _lang)) in app.history.iter().enumerate() {
        trail = trail.push(
            button(text(word.clone()).size(13).color(ACCENT))
                .style(button::text)
                .padding(0)
                .on_press(Message::GoToCrumb(i)),
        );
        trail = trail.push(text("\u{203A}").size(13).color(MUTED)); // ›
    }
    if let Some((word, _)) = &app.current_view {
        trail = trail.push(text(word.clone()).size(13).color(MUTED));
    }
    trail.into()
}

fn centered_hint(message: String) -> Element<'static, Message> {
    container(text(message).size(15).color(MUTED))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
}

/// Render a single entry: headword + POS, IPA, numbered senses, etymology.
fn entry_view(entry: &Entry) -> Element<'static, Message> {
    let bold = Font {
        weight: font::Weight::Bold,
        ..Font::default()
    };
    let italic = Font {
        style: font::Style::Italic,
        ..Font::default()
    };

    let mut col = column![].spacing(10);

    // Headword (left, large+bold) with POS pushed to the right (muted).
    col = col.push(
        row![
            text(entry.word.clone())
                .size(30)
                .font(bold)
                .width(Length::Fill),
            text(entry.pos.clone()).size(14).color(MUTED),
        ]
        .align_y(Vertical::Center),
    );

    // IPA, muted monospace.
    let ipas: Vec<String> = entry.sounds.iter().filter_map(|s| s.ipa.clone()).collect();
    if !ipas.is_empty() {
        col = col.push(
            text(ipas.join("   "))
                .size(14)
                .font(Font::MONOSPACE)
                .color(MUTED),
        );
    }

    // Senses.
    let mut n = 0;
    for sense in &entry.senses {
        if sense.glosses.is_empty() {
            continue;
        }
        n += 1;

        let mut sense_col =
            column![text(format!("{n}.  {}", sense.glosses.join("; "))).size(16)].spacing(6);

        // Examples: plain italic muted, indented.
        for ex in &sense.examples {
            if ex.text.is_empty() {
                continue;
            }
            sense_col = sense_col.push(
                container(
                    text(format!("\u{201C}{}\u{201D}", ex.text))
                        .size(14)
                        .font(italic)
                        .color(MUTED),
                )
                .padding(Padding::ZERO.left(18)),
            );
        }

        // Related words: own line, accent-colored clickable links.
        let words: Vec<String> = sense
            .links
            .iter()
            .filter_map(|l| l.first().cloned())
            .filter(|w| !w.is_empty())
            .collect();
        if !words.is_empty() {
            let mut spans = vec![span("related   ").size(13).color(MUTED)];
            for (j, word) in words.iter().enumerate() {
                if j > 0 {
                    spans.push(span(" · ").size(13).color(MUTED));
                }
                spans.push(span(word.clone()).size(13).color(ACCENT).link(word.clone()));
            }
            sense_col = sense_col.push(
                container(rich_text(spans).on_link_click(Message::LinkClicked))
                    .padding(Padding::ZERO.left(18)),
            );
        }

        col = col.push(sense_col);
    }

    // Etymology: small muted italic, no label.
    if let Some(ety) = &entry.etymology_text {
        if !ety.is_empty() {
            col = col.push(text(ety.clone()).size(13).font(italic).color(MUTED));
        }
    }

    col.into()
}

/// A thin horizontal divider between entries.
fn divider() -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(theme.extended_palette().background.strong.color.into()),
            ..container::Style::default()
        })
        .into()
}

fn input_style(theme: &Theme, _status: text_input::Status) -> text_input::Style {
    let palette = theme.extended_palette();
    text_input::Style {
        // A subtle lift off the background rather than a saturated
        // secondary fill — the switcher should recede, not pop.
        background: palette.background.weak.color.into(),
        border: Border {
            color: Default::default(),
            width: 0.0,
            radius: Radius::new(4.0),
        },
        icon: palette.background.weak.text,
        placeholder: MUTED,
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
    }
}

/// Build the selectable language options from an edition's language list.
pub fn lang_options(langs: &[LanguageInfo]) -> Vec<Lang> {
    langs
        .iter()
        .map(|li| Lang {
            code: li.code.clone(),
            label: li.name.clone(),
        })
        .collect()
}

/// Find the option matching a language code.
pub fn find_lang(langs: &[LanguageInfo], code: &str) -> Option<Lang> {
    lang_options(langs).into_iter().find(|l| l.code == code)
}
