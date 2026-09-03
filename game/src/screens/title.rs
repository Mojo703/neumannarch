//! The title: what a session of the game can start, and the address a
//! join is typed into.

use mirage_engine::egui::{Align2, Pos2, Rect, Vec2};

use crate::net::hosting::{Hosting, NoRoom};
use crate::screens::control::{Controls, Rule, Valued};
use crate::screens::field::{Allow, Field, MAX_ADDRESS, Typed};
use crate::screens::flow::Step;
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
    /// The socket is open and the welcome has not arrived.
    Connecting,
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

/// The title screen: the address a join would use, and the room this
/// machine serves while the title is on screen.
pub struct Title {
    address: Field,
    /// The room Host hands to the flow, or why none is served, which is
    /// what disables Host.
    hosting: Result<Hosting, NoRoom>,
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
    /// The one line the join field shows about it.
    pub fn sentence(self) -> &'static str {
        match self {
            Joining::Connecting => "Connecting",
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
    /// The title as the game opens on it, serving a room this machine can
    /// host and offering a join to it.
    ///
    /// The room is served here, so Host is offered exactly where a click
    /// on it will work.
    pub fn opening() -> Title {
        Title {
            address: Field::holding(
                &format!("{LOOPBACK}:{}", probe_protocol::DEFAULT_PORT),
                Allow::Text(MAX_ADDRESS),
            ),
            hosting: Hosting::opened(),
        }
    }

    /// Paints the title and answers what the player picked. `joining` is
    /// what the last join of an address did.
    pub fn frame(
        &mut self,
        panel: &Panel<'_>,
        typed: &Typed,
        joining: Option<Joining>,
    ) -> Option<Step> {
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
        let hosts = Rule::unless(
            self.hosting
                .as_ref()
                .err()
                .map(|why| why.reason().to_string()),
        );
        let mut controls = Controls::over(panel);
        let mut step = None;
        if controls.action(places.skirmish, "Skirmish", &Rule::Allows) {
            step = Some(Step::Skirmish);
        }
        if controls.action(places.host, "Host", &hosts)
            && let Ok(hosting) = core::mem::replace(&mut self.hosting, Err(NoRoom::Held))
        {
            step = Some(Step::Host(hosting));
        }
        let typing = controls.value(
            Valued {
                rect: places.join,
                rule: &Rule::Allows,
                inside: Some(("Join", &Rule::Allows)),
                state: joining.map(Joining::sentence),
            },
            &mut self.address,
            typed,
        );
        if typing.entered || typing.acted {
            step = Some(Step::Join(self.address.text().to_string()));
        }
        controls.action(places.settings, "Settings", &Rule::refuses(NO_SETTINGS));
        controls.action(places.quit, "Quit", &Rule::refuses(NO_QUIT));
        controls.finish();
        step
    }
}
