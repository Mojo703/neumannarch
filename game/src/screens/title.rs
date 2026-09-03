//! The title: what a session of the game can start.

use mirage_engine::egui::{Align2, Pos2, Rect, Vec2};

use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};

/// What the title offers, in the order it draws them, and whether the
/// player can act on it yet.
///
/// Host and Join wait on the match server, and Settings on the screen that
/// edits the bindings; Quit waits on the engine, whose loop offers no way
/// to ask for the window to close.
pub const ACTIONS: [(&str, bool); 5] = [
    ("Skirmish", true),
    ("Host", false),
    ("Join", false),
    ("Settings", false),
    ("Quit", false),
];

/// How wide the title's column of actions stands, in points.
const WIDTH: f32 = 220.0;

/// The title screen. It holds nothing: what it offers is fixed.
pub struct Title;

impl Title {
    /// Paints the title and answers what the player picked. Only Skirmish
    /// does anything; see [`ACTIONS`].
    pub fn frame(&self, panel: &Panel<'_>) -> Option<Step> {
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

        let mut picked = None;
        for (rect, (label, enabled)) in actions(window).into_iter().zip(ACTIONS) {
            if panel.action(rect, label, enabled) {
                picked = Some(Step::Skirmish);
            }
        }
        picked
    }
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
