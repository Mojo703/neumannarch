//! The pause screen, over the match.

use mirage_engine::egui::{Pos2, Rect, Vec2};

use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};

/// How wide the pause screen's actions stand, in points.
const WIDTH: f32 = 220.0;

/// The pause screen. It holds nothing: the match under it holds whether it
/// is open.
pub struct Pause;

impl Pause {
    /// Paints the pause screen over the belt and answers what the player
    /// picked.
    ///
    /// Surrender waits on a verb for conceding: DESIGN.md gives a player
    /// one verb, and no rule by which a seat gives up before the clock. It
    /// is drawn and does nothing until that lands.
    pub fn frame(&self, panel: &Panel<'_>) -> Option<Step> {
        let window = panel.window();
        let height = panel::ROW_HEIGHT * 8.0;
        let over = Rect::from_center_size(window.center(), Vec2::new(WIDTH * 1.6, height));
        panel.scrim(over);
        panel.outline(over);
        panel.heading(
            "Paused",
            Pos2::new(over.center().x, over.top() + panel::MARGIN),
        );

        let top = Pos2::new(
            over.center().x - WIDTH / 2.0,
            over.top() + panel::MARGIN * 2.0,
        );
        let mut picked = None;
        for (rect, (label, step)) in panel::column(top, WIDTH, 3).zip([
            ("Resume", Some(Step::Resume)),
            ("Surrender", None),
            ("Leave", Some(Step::Title)),
        ]) {
            if panel.action(rect, label, step.is_some()) {
                picked = step;
            }
        }
        picked
    }
}
