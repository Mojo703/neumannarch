//! The title: what a session of the game can start, and the address a
//! join is typed into.

use mirage_engine::egui::{Align2, Pos2, Rect, Vec2};

use crate::screens::field::{Field, Typed};
use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};

/// What the title offers, in the order it draws them.
pub const ACTIONS: [Choice; 5] = [
    Choice::Skirmish,
    Choice::Host,
    Choice::Join,
    Choice::Settings,
    Choice::Quit,
];

/// The address a join offers, which is a room served on this machine.
const LOOPBACK: &str = "127.0.0.1";

/// How wide the title's column of actions stands, in points.
const WIDTH: f32 = 220.0;

/// One of the title's actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Open a lobby whose every seat is this machine's.
    Skirmish,
    /// Serve a room on this machine and open its lobby as the host.
    Host,
    /// Join the room at the address in the field.
    Join,
    /// Edit the bindings.
    Settings,
    /// Close the window.
    Quit,
}

/// The title screen, holding the address a join would use.
pub struct Title {
    address: Field,
}

impl Choice {
    /// What the title's action reads.
    pub fn label(self) -> &'static str {
        match self {
            Choice::Skirmish => "Skirmish",
            Choice::Host => "Host",
            Choice::Join => "Join",
            Choice::Settings => "Settings",
            Choice::Quit => "Quit",
        }
    }

    /// Whether the player can act on it yet.
    ///
    /// Settings waits on the screen that edits the bindings, and Quit on
    /// the engine, whose loop offers no way to ask for the window to close.
    pub fn offered(self) -> bool {
        match self {
            Choice::Skirmish | Choice::Host | Choice::Join => true,
            Choice::Settings | Choice::Quit => false,
        }
    }
}

impl Title {
    /// The title as the game opens on it, offering a join to a room on this
    /// machine.
    pub fn opening() -> Title {
        Title {
            address: Field::holding(&format!("{LOOPBACK}:{}", probe_protocol::DEFAULT_PORT)),
        }
    }

    /// Paints the title and answers what the player picked. `note` is what
    /// the flow has to say under the actions, word for word: the room it is
    /// joining, or why it is not in one.
    pub fn frame(&mut self, panel: &Panel<'_>, typed: &Typed, note: Option<&str>) -> Option<Step> {
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

        let laid = actions(window);
        let entered = self.address.frame(panel, address_field(window), typed);
        let mut picked = None;
        for (rect, choice) in laid.into_iter().zip(ACTIONS) {
            if panel.action(rect, choice.label(), choice.offered()) {
                picked = self.takes(choice);
            }
        }
        if entered {
            picked = self.takes(Choice::Join);
        }
        if let Some(note) = note {
            panel.text(
                note,
                Pos2::new(window.center().x, window.bottom() - panel::MARGIN),
                Align2::CENTER_CENTER,
                panel::DIM_INK,
                panel::BODY_SIZE,
            );
        }
        picked
    }

    /// The screen `choice` opens, where it opens one.
    fn takes(&self, choice: Choice) -> Option<Step> {
        match choice {
            Choice::Skirmish => Some(Step::Skirmish),
            Choice::Host => Some(Step::Host),
            Choice::Join => Some(Step::Join(self.address.text().to_string())),
            Choice::Settings | Choice::Quit => None,
        }
    }
}

/// Where the address is typed, under the actions, in points.
pub fn address_field(window: Rect) -> Rect {
    let [_, _, join, ..] = actions(window);
    Rect::from_min_size(
        Pos2::new(join.right() + panel::ROW_HEIGHT / 2.0, join.top()),
        Vec2::new(WIDTH, panel::ROW_HEIGHT),
    )
}

/// Where each of [`ACTIONS`] is drawn over `window`, in points, in the same
/// order.
pub fn actions(window: Rect) -> [Rect; ACTIONS.len()] {
    let top = Pos2::new(
        window.center().x - WIDTH / 2.0,
        window.center().y - WIDTH / 4.0,
    );
    let mut rects = [Rect::ZERO; ACTIONS.len()];
    for (rect, laid) in rects
        .iter_mut()
        .zip(panel::column(top, WIDTH, ACTIONS.len()))
    {
        *rect = laid;
    }
    rects
}
