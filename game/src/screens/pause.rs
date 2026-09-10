use mirage_engine::egui::{Pos2, Rect, Vec2};

use crate::screens::control::{Controls, Rule};
use crate::screens::panel::{self, Panel};

const WIDTH: f32 = 220.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked {
    Resume,
    Leave,
}

pub struct Pause;

impl Pause {
    pub fn frame(&self, panel: &Panel<'_>) -> Option<Picked> {
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
        let [resume, leave] = panel::rows(top, WIDTH);
        if controls.action(resume, "Resume", &Rule::Allows) {
            picked = Some(Picked::Resume);
        }
        if controls.action(leave, "Leave", &Rule::Allows) {
            picked = Some(Picked::Leave);
        }
        controls.finish();
        picked
    }
}
