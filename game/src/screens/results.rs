use mirage_engine::egui::{Align2, Pos2, Rect, Vec2};
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use probe_protocol::Lobby;
use probe_sim::state::State;
use probe_sim::state::standings::{Standings, Team};
use probe_sim::{SeatId, TeamId};

use crate::display::camera::BeltCamera;
use crate::display::glyph::{self, Frame, Glyph, Size};
use crate::display::glyph_quad::{GlyphQuad, seat_color32};
use crate::display::scene::{Fill, Scene};
use crate::display::stencil::Stencil;
use crate::display::viewport::Viewport;
use crate::display::{belt, hud};
use crate::screens::control::{Controls, Rule};
use crate::screens::panel::{self, Panel};

const WIDTH: f32 = 520.0;

const ACTION_WIDTH: f32 = 160.0;

const ROCK_HELD: Glyph = Glyph {
    frame: Frame::Square,
    marks: Vec::new(),
    size: Size::Medium,
};

const ROCK_STEP: f32 = 2.5 * glyph::HALF;

const WINNER_FROM: f32 = 84.0;

const ROCKS_FROM: f32 = 176.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked {
    Rematch,
    Leave,
}

pub struct Results {
    lobby: Lobby,
    scene: Scene,
    camera: BeltCamera,
    standings: Standings,
    teams: Vec<TeamId>,
}

impl Results {
    pub fn of(lobby: Lobby, scene: Scene, camera: BeltCamera, state: &State) -> Results {
        Results {
            lobby,
            scene,
            camera,
            standings: state.standings(),
            teams: state.seats().iter().map(|seat| seat.team()).collect(),
        }
    }

    pub fn frame<G: crate::screens::Playable>(
        &mut self,
        ctx: &mut FrameCtx<'_, G>,
        rematch: &Rule,
    ) -> Option<Picked>
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());
        let size = ctx.window_size();
        let viewport = Viewport::of(&self.camera, size, points_per_pixel);
        belt::draw(&self.scene, &viewport, ctx);

        let pointer = viewport.point_at(ctx.pointer());
        let clicked = ctx.pressed(crate::controls::Button::Select);
        let window = panel::window_of(size, points_per_pixel);
        let scene = &self.scene;
        let standings = &self.standings;
        let teams = &self.teams;
        let mut picked = None;
        ctx.ui(|ui| {
            hud::paint(scene, &viewport, None, ui.painter());
            let panel = Panel::new(ui.painter(), window, pointer, clicked);
            picked = paint(&panel, standings, teams, rematch);
        });
        picked
    }

    pub fn lobby(&self) -> &Lobby {
        &self.lobby
    }
}

fn paint_team(panel: &Panel<'_>, rect: Rect, team: &Team, colour: SeatId, winner: bool) {
    let ink = match winner {
        true => panel::INK,
        false => panel::DIM_INK,
    };
    panel.label(
        &format!("Team {}", team.team.0 as u16 + 1),
        Pos2::new(rect.left(), rect.center().y),
        seat_color32(colour),
    );
    if winner {
        panel.text(
            "Winner",
            Pos2::new(rect.left() + WINNER_FROM, rect.center().y),
            Align2::LEFT_CENTER,
            ink,
            panel::BODY_SIZE,
        );
    }
    for at in 0..team.rocks {
        Stencil {
            glyph: &ROCK_HELD,
            centre: Pos2::new(
                rect.left() + ROCKS_FROM + at as f32 * ROCK_STEP,
                rect.center().y,
            ),
            half: glyph::HALF,
            colour: seat_color32(colour),
            fill: Fill::Solid,
            dim: false,
            starved: None,
        }
        .paint(panel.painter());
    }
    panel.text(
        &format!("Value {}", team.value as u64),
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        ink,
        panel::BODY_SIZE,
    );
}

fn paint(
    panel: &Panel<'_>,
    standings: &Standings,
    teams: &[TeamId],
    rematch: &Rule,
) -> Option<Picked> {
    let window = panel.window();
    let rows = standings.teams().len();
    let height = panel::ROW_HEIGHT * (rows as f32 * 1.25 + 6.0);
    let over = Rect::from_center_size(window.center(), Vec2::new(WIDTH, height));
    panel.scrim(over);
    panel.outline(over);
    panel.heading(
        "Results",
        Pos2::new(over.center().x, over.top() + panel::MARGIN),
    );

    let leaders = standings.leaders();
    let top = Pos2::new(
        over.left() + panel::MARGIN,
        over.top() + panel::MARGIN * 2.0,
    );
    let width = over.width() - 2.0 * panel::MARGIN;
    for (rect, team) in panel::column(top, width, rows).zip(standings.teams()) {
        let colour = teams
            .iter()
            .position(|seat| *seat == team.team)
            .map_or(SeatId(0), |at| SeatId(at as u8));
        paint_team(panel, rect, team, colour, leaders.contains(&team.team));
    }

    let actions = Pos2::new(
        over.center().x - ACTION_WIDTH - panel::ROW_HEIGHT / 2.0,
        over.bottom() - panel::MARGIN - panel::ROW_HEIGHT,
    );
    let again = Rect::from_min_size(actions, Vec2::new(ACTION_WIDTH, panel::ROW_HEIGHT));
    let leave = again.translate(Vec2::new(ACTION_WIDTH + panel::ROW_HEIGHT, 0.0));
    let mut controls = Controls::over(panel);
    let mut picked = None;
    if controls.action(again, "Rematch", rematch) {
        picked = Some(Picked::Rematch);
    }
    if controls.action(leave, "Leave", &Rule::Allows) {
        picked = Some(Picked::Leave);
    }
    controls.finish();
    picked
}
