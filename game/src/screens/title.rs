use mirage_engine::egui::{Align2, Pos2, Rect, Vec2};

use crate::screens::control::{Controls, Rule, Valued};
use crate::screens::field::{Allow, Field, MAX_ADDRESS, Typed};
use crate::screens::panel::{self, Panel};

const LOOPBACK: &str = "127.0.0.1";

const WIDTH: f32 = 420.0;

pub(crate) const TITLE: &str = "Probe";

pub(crate) const TAGLINE: &str = "Two to four players in a belt";

pub(crate) const NO_SETTINGS: &str = "Cannot open settings here";

pub(crate) const NO_QUIT: &str = "Cannot quit here";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NoRoom,
    Full,
    Version,
    Closed,
    HostLeft,
    Removed,
}

pub struct Title {
    address: Field,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Skirmish,
    Host,
    Join(String),
}

pub struct Places {
    pub skirmish: Rect,
    pub host: Rect,
    pub join: Rect,
    pub settings: Rect,
    pub quit: Rect,
}

impl Outcome {
    pub fn phrase(self) -> &'static str {
        match self {
            Outcome::NoRoom => "No room",
            Outcome::Full => "Room full",
            Outcome::Version => "Version differs",
            Outcome::Closed => "Room closed",
            Outcome::HostLeft => "Host left",
            Outcome::Removed => "Removed",
        }
    }

    pub fn every() -> [Outcome; 6] {
        [
            Outcome::NoRoom,
            Outcome::Full,
            Outcome::Version,
            Outcome::Closed,
            Outcome::HostLeft,
            Outcome::Removed,
        ]
    }
}

impl Places {
    pub fn over(window: Rect) -> Places {
        let top = Pos2::new(
            window.center().x - WIDTH / 2.0,
            window.center().y - WIDTH / 4.0,
        );
        let [skirmish, host, field, settings, quit] = panel::rows(top, WIDTH);

        let join = Rect::from_min_size(field.min, Vec2::new(WIDTH, panel::ROW_HEIGHT * 2.0));
        let below = Vec2::new(0.0, panel::ROW_HEIGHT);
        Places {
            skirmish,
            host,
            join,
            settings: settings.translate(below),
            quit: quit.translate(below),
        }
    }
}

impl Title {
    pub fn opening() -> Title {
        Title {
            address: Field::holding(
                &format!("{LOOPBACK}:{}", probe_protocol::DEFAULT_PORT),
                Allow::Text(MAX_ADDRESS),
            ),
        }
    }

    pub fn frame(
        &mut self,
        panel: &Panel<'_>,
        typed: &Typed,
        hosts: &Rule,
        outcome: Option<&'static str>,
    ) -> Option<Action> {
        panel.backdrop();
        let window = panel.window();
        let heading = Pos2::new(window.center().x, window.top() + panel::MARGIN * 2.0);
        panel.heading(TITLE, heading);
        panel.text(
            TAGLINE,
            heading + Vec2::new(0.0, panel::HEADING_SIZE),
            Align2::CENTER_CENTER,
            panel::DIM_INK,
            panel::BODY_SIZE,
        );

        let places = Places::over(window);
        let mut controls = Controls::over(panel);
        let mut picked = None;
        if controls.action(places.skirmish, "Skirmish", &Rule::Allows) {
            picked = Some(Action::Skirmish);
        }
        if controls.action(places.host, "Host", hosts) {
            picked = Some(Action::Host);
        }
        let typing = controls.value(
            Valued {
                rect: places.join,
                rule: &Rule::Allows,
                inside: Some(("Join", &Rule::Allows)),
                state: outcome,
            },
            &mut self.address,
            typed,
        );
        if typing.entered || typing.acted {
            picked = Some(Action::Join(self.address.text().to_string()));
        }
        controls.action(places.settings, "Settings", &Rule::refuses(NO_SETTINGS));
        controls.action(places.quit, "Quit", &Rule::refuses(NO_QUIT));
        controls.finish();
        picked
    }
}
