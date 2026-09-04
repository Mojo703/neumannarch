//! The title: what a session of the game can start, and the address a
//! join is typed into.

use mirage_engine::egui::{Align2, Pos2, Rect, Vec2};
use probe_protocol::Refusal;

use crate::screens::control::{Controls, Rule, Valued};
use crate::screens::field::{Allow, Field, MAX_ADDRESS, Typed};
use crate::screens::panel::{self, Panel};

/// The address a join offers, which is a room served on this machine.
const LOOPBACK: &str = "127.0.0.1";

/// How wide the title's column of controls stands, in points.
const WIDTH: f32 = 420.0;

/// Why Settings is disabled: the screen that edits the bindings is a later
/// unit.
const NO_SETTINGS: &str = "Settings are not available in this version";

/// Why Quit is disabled: the engine's loop offers no way to ask for the
/// window to close, so nothing the game can call closes it.
const NO_QUIT: &str = "Closing is not available in this version";

/// How a join of the room at the address went, shown inside the join
/// field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joining {
    /// The socket closed before any welcome, so nothing answers there.
    NoRoom,
    /// Every slot of the room is held.
    Full,
    /// The room speaks another version of the protocol.
    Version,
    /// The room this machine was in has closed.
    Closed,
    /// The host of the room this machine was in left it.
    HostLeft,
    /// The host opened the slot this machine held.
    Removed,
}

/// The title screen: the address a join would use, kept as it was typed
/// for as long as the game runs.
pub struct Title {
    address: Field,
}

/// What the title's actions ask for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Picked {
    /// A lobby of seats all on this machine.
    Skirmish,
    /// Serve a room here and join it.
    Host,
    /// Join the room at this address, as `host:port`.
    Join(String),
}

/// Where the title's controls stand, which its paint, its hit test and the
/// headless drive all read.
pub struct Places {
    pub skirmish: Rect,
    pub host: Rect,
    /// The join field, with the Join action inside its right edge, where
    /// [`crate::screens::control::inside_rect`] puts it.
    pub join: Rect,
    pub settings: Rect,
    pub quit: Rect,
}

impl Joining {
    /// What the join field says about a refusal, where it is one a joining
    /// machine hears; `None` for a refusal only a machine already in the
    /// room is sent, which the lobby the room broadcasts answers.
    pub fn refused(why: Refusal) -> Option<Joining> {
        match why {
            Refusal::Full => Some(Joining::Full),
            Refusal::Version => Some(Joining::Version),
            Refusal::Edit(_) | Refusal::NotReady(_) => None,
        }
    }

    /// The one line the join field shows about it.
    pub fn sentence(self) -> &'static str {
        match self {
            Joining::NoRoom => "No room at this address",
            Joining::Full => "The room is full",
            Joining::Version => "That version differs",
            Joining::Closed => "The room closed",
            Joining::HostLeft => "The host left",
            Joining::Removed => "Removed by the host",
        }
    }
}

impl Places {
    /// Where each control stands over `window`, in points.
    pub fn over(window: Rect) -> Places {
        let top = Pos2::new(
            window.center().x - WIDTH / 2.0,
            window.center().y - WIDTH / 4.0,
        );
        let [skirmish, host, field, settings, quit] = panel::rows(top, WIDTH);
        // The join field carries its state under its address, so it stands
        // a row taller and the rows under it move down by one.
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
    /// The title as the game opens on it, offering a join to the room this
    /// machine would serve.
    pub fn opening() -> Title {
        Title {
            address: Field::holding(
                &format!("{LOOPBACK}:{}", probe_protocol::DEFAULT_PORT),
                Allow::Text(MAX_ADDRESS),
            ),
        }
    }

    /// Paints the title and answers what the player picked. `hosts` is
    /// whether a room is served for Host to join, and `said` what the join
    /// field shows about the last join or the last room.
    pub fn frame(
        &mut self,
        panel: &Panel<'_>,
        typed: &Typed,
        hosts: &Rule,
        said: Option<&'static str>,
    ) -> Option<Picked> {
        panel.backdrop();
        let window = panel.window();
        let heading = Pos2::new(window.center().x, window.top() + panel::MARGIN * 2.0);
        panel.heading("Probe", heading);
        panel.text(
            "A two-to-four-player space RTS",
            heading + Vec2::new(0.0, panel::HEADING_SIZE),
            Align2::CENTER_CENTER,
            panel::DIM_INK,
            panel::BODY_SIZE,
        );

        let places = Places::over(window);
        let mut controls = Controls::over(panel);
        let mut picked = None;
        if controls.action(places.skirmish, "Skirmish", &Rule::Allows) {
            picked = Some(Picked::Skirmish);
        }
        if controls.action(places.host, "Host", hosts) {
            picked = Some(Picked::Host);
        }
        let typing = controls.value(
            Valued {
                rect: places.join,
                rule: &Rule::Allows,
                inside: Some(("Join", &Rule::Allows)),
                state: said,
            },
            &mut self.address,
            typed,
        );
        if typing.entered || typing.acted {
            picked = Some(Picked::Join(self.address.text().to_string()));
        }
        controls.action(places.settings, "Settings", &Rule::refuses(NO_SETTINGS));
        controls.action(places.quit, "Quit", &Rule::refuses(NO_QUIT));
        controls.finish();
        picked
    }
}
