//! The pause screen, over the match.

use mirage_engine::egui::{Pos2, Rect, Vec2};

use crate::screens::control::{Controls, Rule};
use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};

/// How wide the pause screen's actions stand, in points.
const WIDTH: f32 = 220.0;

/// Why Surrender is disabled: DESIGN.md gives a player one verb, and no
/// rule by which a seat gives up before the clock.
const NO_SURRENDER: &str = "Surrendering is not available in this version";

/// The pause screen. It holds nothing: the match under it holds whether it
/// is open.
pub struct Pause;

impl Pause {
    /// Paints the pause screen over the belt and answers what the player
    /// picked.
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
        let mut controls = Controls::over(panel);
        let mut picked = None;
        let [resume, surrender, leave] = panel::rows(top, WIDTH);
        if controls.action(resume, "Resume", &Rule::Allows) {
            picked = Some(Step::Resume);
        }
        controls.action(surrender, "Surrender", &Rule::refuses(NO_SURRENDER));
        if controls.action(leave, "Leave", &Rule::Allows) {
            picked = Some(Step::Title);
        }
        controls.finish();
        picked
    }
}
