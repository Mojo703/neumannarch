//! The two states a match holds in, over the dimmed HUD.

use mirage_engine::egui::{Align2, Pos2, Rect, Vec2};
use probe_sim::{SeatId, Tick};

use crate::display::glyph_quad::seat_color32;
use crate::screens::control::{Controls, Rule};
use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};

/// How wide a waiting seat's colour stands, in points.
const SWATCH: f32 = 22.0;

/// How wide the desync's Leave action stands, in points.
const WIDTH: f32 = 220.0;

/// Why a match is holding.
pub enum Held {
    /// Two machines' histories differ at this tick, so the match cannot go
    /// on.
    Desynced(Tick),
    /// The match is waiting for the machines these seats belong to. It
    /// resumes by itself.
    Waiting(Vec<SeatId>),
}

impl Held {
    /// Paints the hold over the whole window, dimming what the HUD drew
    /// under it, and answers what the player picked.
    pub fn frame(&self, panel: &Panel<'_>) -> Option<Step> {
        let window = panel.window();
        panel.scrim(window);
        let middle = window.center();
        match self {
            Held::Waiting(seats) => {
                panel.text(
                    "Waiting for the other machines",
                    middle,
                    Align2::CENTER_CENTER,
                    panel::INK,
                    panel::BODY_SIZE,
                );
                paint_swatches(panel, middle, seats);
                None
            }
            Held::Desynced(tick) => {
                panel.heading("Desynced", middle);
                panel.text(
                    &format!("The machines parted at tick {}", tick.0),
                    middle + Vec2::new(0.0, panel::HEADING_SIZE),
                    Align2::CENTER_CENTER,
                    panel::DIM_INK,
                    panel::BODY_SIZE,
                );
                let leave = Rect::from_center_size(
                    middle + Vec2::new(0.0, panel::HEADING_SIZE * 3.0),
                    Vec2::new(WIDTH, panel::ROW_HEIGHT),
                );
                let mut controls = Controls::over(panel);
                let left = controls.action(leave, "Leave", &Rule::Allows);
                controls.finish();
                left.then_some(Step::Title)
            }
        }
    }
}

/// The colours of the seats the match is waiting on, in a row under `at`.
fn paint_swatches(panel: &Panel<'_>, at: Pos2, seats: &[SeatId]) {
    let step = SWATCH * 1.5;
    let left = at.x - step * (seats.len() as f32 - 1.0) / 2.0;
    for (which, seat) in seats.iter().enumerate() {
        panel.painter().rect_filled(
            Rect::from_center_size(
                Pos2::new(left + step * which as f32, at.y + panel::ROW_HEIGHT),
                Vec2::splat(SWATCH),
            ),
            0.0,
            seat_color32(*seat),
        );
    }
}
