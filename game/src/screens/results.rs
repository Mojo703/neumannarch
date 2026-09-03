//! The results: the standings over the belt the match ended on.

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
use crate::display::screen::Screen;
use crate::display::stencil::Stencil;
use crate::display::{belt, hud};
use crate::screens::flow::Step;
use crate::screens::panel::{self, Panel};

/// How wide the standings panel stands, in points.
const WIDTH: f32 = 520.0;

/// How wide the bottom actions stand, in points.
const ACTION_WIDTH: f32 = 160.0;

/// A rock held is drawn as the glyph of the thing that holds it, which is a
/// structure, at the run's own size.
const ROCK_HELD: Glyph = Glyph {
    frame: Frame::Square,
    marks: Vec::new(),
    size: Size::Medium,
};

/// How far apart the squares of a team's rocks stand, in points.
const ROCK_STEP: f32 = 2.5 * glyph::HALF;

/// How far into a team's row its own mark stands, in points, clear of its
/// name.
const WINNER_FROM: f32 = 84.0;

/// How far into a team's row its rocks begin, in points, clear of its name
/// and its own mark.
const ROCKS_FROM: f32 = 176.0;

/// The end of a match: what each team held, the belt it ended on, and the
/// lobby it came from, which a rematch returns to unchanged.
pub struct Results {
    lobby: Lobby,
    /// The belt as the last frame drew it, still.
    scene: Scene,
    camera: BeltCamera,
    standings: Standings,
    /// Each seat's team, in seat order, so a team takes its lowest seat's
    /// colour.
    teams: Vec<TeamId>,
}

impl Results {
    /// The score of the match `state` has reached, over `scene`, the belt
    /// its last frame drew, seen from `camera`.
    pub fn of(lobby: Lobby, scene: Scene, camera: BeltCamera, state: &State) -> Results {
        Results {
            lobby,
            scene,
            camera,
            standings: state.standings(),
            teams: state.seats().iter().map(|seat| seat.team()).collect(),
        }
    }

    /// Paints the results over the still belt and answers what the player
    /// picked.
    pub fn frame<G: crate::screens::Playable>(&mut self, ctx: &mut FrameCtx<'_, G>) -> Option<Step>
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());
        let size = ctx.window_size();
        let screen = Screen::of(&self.camera, size, points_per_pixel);
        belt::draw(&self.scene, &screen, ctx);

        let pointer = screen.point_at(ctx.pointer());
        let clicked = ctx.pressed(crate::controls::Button::Select);
        let window = panel::window_of(size, points_per_pixel);
        let scene = &self.scene;
        let standings = &self.standings;
        let teams = &self.teams;
        let mut picked = None;
        ctx.ui(|ui| {
            hud::paint(scene, &screen, None, ui.painter());
            let panel = Panel::new(ui.painter(), window, pointer, clicked);
            picked = paint(&panel, standings, teams);
        });
        picked
    }

    /// The lobby the match was set up in, which a rematch keeps.
    pub fn lobby(&self) -> &Lobby {
        &self.lobby
    }
}

/// One team's row: its name in its own colour, a ring glyph per rock it
/// holds, and its army value; the winning row is marked.
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

/// The results panel, and what the player picked in it.
fn paint(panel: &Panel<'_>, standings: &Standings, teams: &[TeamId]) -> Option<Step> {
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
    let rematch = Rect::from_min_size(actions, Vec2::new(ACTION_WIDTH, panel::ROW_HEIGHT));
    let leave = rematch.translate(Vec2::new(ACTION_WIDTH + panel::ROW_HEIGHT, 0.0));
    if panel.action(rematch, "Rematch", true) {
        return Some(Step::Rematch);
    }
    if panel.action(leave, "Leave", true) {
        return Some(Step::Title);
    }
    None
}
