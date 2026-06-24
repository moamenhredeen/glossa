//! Settings page: install / uninstall editions and pick the active one.

use iced::widget::{button, column, container, progress_bar, row, text};
use iced::{color, Element, Length};

use glossa::model::catalog::EDITIONS;

use crate::{App, InstallState, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    let mut col = column![text("Dictionaries").size(24)].spacing(12);

    let active = app.library.active_edition().map(|e| e.code);

    for edition in EDITIONS {
        let installed = app.library.is_installed(edition.code);
        let installing = app.installs.get(edition.code);

        // Left: name + size/status.
        let title = text(edition.name).size(18);
        let subtitle = text(if installed {
            "Installed".to_string()
        } else {
            format!("{} download", edition.size)
        })
        .size(13)
        .color(color!(0x888888));

        // Right: action area depends on state.
        let action: Element<Message> = if let Some(state) = installing {
            install_status(state)
        } else if installed {
            let mut r = row![].spacing(8);
            if active == Some(edition.code) {
                r = r.push(text("Active").size(14).color(color!(0x4a90d9)));
            } else {
                r = r.push(
                    button("Set active")
                        .on_press(Message::SetActiveEdition(edition.code.to_string())),
                );
            }
            r = r.push(button("Uninstall").on_press(Message::Uninstall(edition.code.to_string())));
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

    col.into()
}

fn install_status(state: &InstallState) -> Element<'_, Message> {
    match state {
        InstallState::Downloading { received, total } => {
            let label = match total {
                Some(t) if *t > 0 => format!(
                    "Downloading {:.0}%",
                    (*received as f64 / *t as f64) * 100.0
                ),
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
        InstallState::Failed(e) => text(format!("Failed: {e}"))
            .size(13)
            .color(color!(0xcc4444))
            .into(),
    }
}
