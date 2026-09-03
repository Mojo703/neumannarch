//! The lobby: the belt the match will be played on, behind the seats
//! grouped by team and the host's shape.

use mirage_engine::egui::{Align2, Color32, Pos2, Rect, Vec2};
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use probe_agents::Personality;
use probe_protocol::{Bot, Control, Lobby, LobbyEdit, MAX_SLOTS, PlayerId, SeatSlot};
use probe_sim::belt::Belt;
use probe_sim::{TICKS_PER_SECOND, TeamId, Tick};

use crate::controls::Button;
use crate::display::camera::BeltCamera;
use crate::display::glyph_quad::{GlyphQuad, seat_color32};
use crate::display::scene::Scene;
use crate::display::screen::Screen;
use crate::display::{belt, hud};
use crate::screens::Playable;
use crate::screens::flow::Step;
use crate::screens::loading::Loading;
use crate::screens::panel::{self, Panel};

/// The eye-to-focus distance the lobby and loading screens show the belt
/// from, in meters.
///
/// A ring holds a fixed screen radius at every zoom, so this is the widest
/// view whose rings stand apart, which is a region of the belt and not all
/// of it.
pub const PREVIEW_ZOOM: f64 = 12_000.0;

/// The clocks the lobby offers, in ticks, which its clock action cycles.
const CLOCKS: [Tick; 4] = [
    Tick(60 * TICKS_PER_SECOND as u64),
    Tick(5 * 60 * TICKS_PER_SECOND as u64),
    Tick(15 * 60 * TICKS_PER_SECOND as u64),
    Tick(30 * 60 * TICKS_PER_SECOND as u64),
];

/// The personality Add Bot seats, which clicking the holder then cycles.
const FIRST_BOT: Bot = Bot::Turtle;

/// How wide a seat row stands, in points.
const SEAT_WIDTH: f32 = 380.0;

/// How wide the shape's own rows stand, in points.
const SHAPE_WIDTH: f32 = 200.0;

/// How wide a seat's colour swatch stands, in points.
const SWATCH: f32 = 14.0;

/// How wide a seat's readiness mark stands, in points.
const READY_WIDTH: f32 = 76.0;

/// How wide a bottom action stands, in points.
const ACTION_WIDTH: f32 = 150.0;

/// The lobby screen: the lobby itself, and the belt its seed lays.
pub struct LobbyScreen {
    lobby: Lobby,
    me: PlayerId,
    scene: Scene,
    camera: BeltCamera,
    /// The seed the belt was laid from, so the belt is rebuilt the instant
    /// the seed changes.
    laid: u64,
}

impl LobbyScreen {
    /// `lobby` as `me` sees it.
    pub fn of(lobby: Lobby, me: PlayerId) -> LobbyScreen {
        let laid = lobby.seed();
        let scene = belt_of(laid);
        let camera = BeltCamera::new(scene.centre(), PREVIEW_ZOOM);
        LobbyScreen {
            lobby,
            me,
            scene,
            camera,
            laid,
        }
    }

    /// Paints the belt, the seats and the shape, and answers what the
    /// player picked.
    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>) -> Option<Step>
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        if self.laid != self.lobby.seed() {
            self.laid = self.lobby.seed();
            self.scene = belt_of(self.laid);
        }

        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());
        let size = ctx.window_size();
        let screen = Screen::of(&self.camera, size, points_per_pixel);
        belt::draw(&self.scene, &screen, ctx);

        let pointer = screen.point_at(ctx.pointer());
        let clicked = ctx.pressed(Button::Select);
        let window = panel::window_of(size, points_per_pixel);
        let scene = &self.scene;
        let lobby = &self.lobby;
        let me = self.me;
        let mut picked = None;
        let mut edit = None;
        ctx.ui(|ui| {
            hud::paint(scene, &screen, None, ui.painter());
            let panel = Panel::new(ui.painter(), window, pointer, clicked);
            let (step, asked) = paint(&panel, lobby, me);
            picked = step;
            edit = asked;
        });

        match edit {
            Some(Asked::Edits(edits)) => {
                for edit in edits {
                    // Every control the screen draws as an action is one
                    // this viewer owns, so the lobby takes it.
                    self.lobby
                        .edit(me, edit)
                        .expect("a control the screen offers is an edit its viewer owns");
                }
            }
            Some(Asked::Regenerate) => self.lobby.regenerate_seed(),
            None => {}
        }
        picked
    }

    /// The lobby as it stands, which a match is frozen from.
    pub fn lobby(&self) -> &Lobby {
        &self.lobby
    }
}

/// What a control on the screen asked of the lobby.
enum Asked {
    /// Edits to apply in order; every one is the viewer's to make.
    Edits(Vec<LobbyEdit>),
    /// The seed's own action, which steps the seed rather than setting it.
    Regenerate,
}

/// One line of the seat column: a team's heading, one of its seats, or one
/// of the host's actions under it.
enum Line {
    /// A team, by the number it is shown as.
    Heading(TeamId),
    /// The slot at this index.
    Seat(usize),
    /// Add Bot and Open Seat, on this team.
    Actions(TeamId),
}

/// Every team that holds a seat, in team order.
fn teams(lobby: &Lobby) -> Vec<TeamId> {
    let mut teams: Vec<TeamId> = lobby
        .slots()
        .iter()
        .filter(|slot| slot.control != Control::Closed)
        .map(|slot| slot.team)
        .collect();
    teams.sort_unstable();
    teams.dedup();
    teams
}

/// The seat column top to bottom: each team that holds a seat, its seats
/// under it, and the host's own actions under those. A closed seat is not
/// drawn, so a team with none of its own is not either.
fn lines(lobby: &Lobby, host: bool) -> Vec<Line> {
    let mut lines = Vec::new();
    for team in teams(lobby) {
        lines.push(Line::Heading(team));
        lines.extend(
            lobby
                .slots()
                .iter()
                .enumerate()
                .filter(|(_, slot)| slot.team == team && slot.control != Control::Closed)
                .map(|(at, _)| Line::Seat(at)),
        );
        if host {
            lines.push(Line::Actions(team));
        }
    }
    lines
}

/// The first closed slot, which is where a seat added to a team goes;
/// `None` once every slot is held.
fn spare(lobby: &Lobby) -> Option<usize> {
    lobby
        .slots()
        .iter()
        .position(|slot| slot.control == Control::Closed)
}

/// The edits that put `control` on `team` in the first spare slot.
fn added(lobby: &Lobby, team: TeamId, control: Control) -> Option<Asked> {
    let slot = spare(lobby)?;
    Some(Asked::Edits(vec![
        LobbyEdit::SetSlot { slot, control },
        LobbyEdit::SetTeam { slot, team },
    ]))
}

/// The next thing a click on a bot's holder puts in its slot: the
/// personalities in turn. A person's own holder does not cycle.
fn cycled(control: Control) -> Control {
    match control {
        Control::Bot(Bot::Turtle) => Control::Bot(Bot::Expand),
        Control::Bot(Bot::Expand) => Control::Bot(Bot::Turtle),
        Control::Open | Control::Closed | Control::Player { .. } => control,
    }
}

/// The team a click on a seat's Move action lands, cycling the teams a
/// slot may sit on, so a seat both changes team and opens a new one.
fn next_team(team: TeamId) -> TeamId {
    TeamId((usize::from(team.0) + 1) as u8 % MAX_SLOTS as u8)
}

/// `team` as the player reads it, numbered from one.
fn team_name(team: TeamId) -> String {
    format!("Team {}", team.0 as u16 + 1)
}

/// A seat's Move action, which names the team the click lands it on rather
/// than the team it sits on, since the heading above it already says that.
fn moved_name(team: TeamId) -> String {
    format!("To {}", team_name(next_team(team)))
}

/// What holds a slot, as the player reads it.
fn holder_name(slot: &SeatSlot, me: PlayerId) -> String {
    match slot.control {
        Control::Open => "Open".to_string(),
        // A closed seat is not drawn, so its own name is never read.
        Control::Closed => "Closed".to_string(),
        Control::Bot(bot) => format!("Bot · {}", titled(Personality::of(bot).name)),
        Control::Player { player, .. } if player == me => "You".to_string(),
        Control::Player { player, .. } => format!("Player {}", player.0 as u64 + 1),
    }
}

/// `word` with its first letter upper case, which is how a personality's
/// own lower-case name is shown.
fn titled(word: &str) -> String {
    let mut letters = word.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

/// The belt `seed` lays, at the start of the match.
///
/// Map generation is a later unit, so every seed lays the same rocks; the
/// seed changes nothing the screen draws until it lands.
fn belt_of(_seed: u64) -> Scene {
    Scene::of_belt(&Belt::fixed(Belt::GRAVITY), Belt::GRAVITY, Tick::ZERO)
}

/// The whole screen, and what it asked of the lobby.
fn paint(panel: &Panel<'_>, lobby: &Lobby, me: PlayerId) -> (Option<Step>, Option<Asked>) {
    let window = panel.window();
    let host = lobby.host() == me;
    let mut asked = None;

    let laid = lines(lobby, host);
    for (rect, line) in rows(window, laid.len()).zip(&laid) {
        if let Some(one) = paint_line(panel, rect, lobby, me, line) {
            asked = Some(one);
        }
    }

    if let Some(one) = paint_shape(panel, lobby, host) {
        asked = Some(one);
    }
    let (step, readied) = bottom(panel, lobby, me);
    (step, readied.or(asked))
}

/// One line of the seat column.
fn paint_line(
    panel: &Panel<'_>,
    rect: Rect,
    lobby: &Lobby,
    me: PlayerId,
    line: &Line,
) -> Option<Asked> {
    match line {
        Line::Heading(team) => {
            panel.text(
                &team_name(*team),
                Pos2::new(rect.left(), rect.center().y),
                Align2::LEFT_CENTER,
                panel::INK,
                panel::HEADING_SIZE * 0.7,
            );
            None
        }
        Line::Seat(at) => paint_seat(panel, rect, lobby, me, *at),
        Line::Actions(team) => paint_team_actions(panel, rect, lobby, *team),
    }
}

/// One seat: its colour, who holds it, the team it would move to, and its
/// readiness mark.
fn paint_seat(
    panel: &Panel<'_>,
    rect: Rect,
    lobby: &Lobby,
    me: PlayerId,
    at: usize,
) -> Option<Asked> {
    let slot = lobby.slots()[at];
    let host = lobby.host() == me;
    let mine = slot.holds(me);
    panel.painter().rect_filled(
        Rect::from_center_size(
            Pos2::new(rect.left() + SWATCH, rect.center().y),
            Vec2::splat(SWATCH),
        ),
        0.0,
        lobby
            .seat_of(at)
            .map_or(Color32::from_gray(60), seat_color32),
    );

    let cells = rect.width() - 2.0 * SWATCH - READY_WIDTH;
    let holder = Rect::from_min_size(
        Pos2::new(rect.left() + 2.0 * SWATCH, rect.top()),
        Vec2::new(cells * 0.54, rect.height()),
    );
    let moved = Rect::from_min_size(
        Pos2::new(holder.right() + 8.0, rect.top()),
        Vec2::new(cells * 0.42, rect.height()),
    );

    let mut asked = None;
    // Only a bot's holder has anything to cycle to; every other holder is
    // stated rather than offered.
    let name = holder_name(&slot, me);
    match host && matches!(slot.control, Control::Bot(_)) {
        true if panel.action(holder, &name, true) => {
            asked = Some(Asked::Edits(vec![LobbyEdit::SetSlot {
                slot: at,
                control: cycled(slot.control),
            }]));
        }
        true => {}
        false => panel.label(
            &name,
            Pos2::new(holder.left(), holder.center().y),
            panel::INK,
        ),
    }
    if panel.action(moved, &moved_name(slot.team), host || mine) {
        asked = Some(Asked::Edits(vec![LobbyEdit::SetTeam {
            slot: at,
            team: next_team(slot.team),
        }]));
    }
    panel.text(
        match slot.control {
            Control::Player { ready: true, .. } => "Ready",
            Control::Player { ready: false, .. } => "Waiting",
            Control::Open | Control::Closed | Control::Bot(_) => "",
        },
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        panel::DIM_INK,
        panel::BODY_SIZE,
    );
    asked
}

/// Add Bot and Open Seat under one team, which only the host is shown.
fn paint_team_actions(panel: &Panel<'_>, rect: Rect, lobby: &Lobby, team: TeamId) -> Option<Asked> {
    let room = spare(lobby).is_some();
    let width = (rect.width() - 2.0 * SWATCH) / 2.0 - 6.0;
    let add = Rect::from_min_size(
        Pos2::new(rect.left() + 2.0 * SWATCH, rect.top()),
        Vec2::new(width, rect.height()),
    );
    let open = add.translate(Vec2::new(width + 6.0, 0.0));
    if panel.action(add, "Add Bot", room) {
        return added(lobby, team, Control::Bot(FIRST_BOT));
    }
    if panel.action(open, "Open Seat", room) {
        return added(lobby, team, Control::Open);
    }
    None
}

/// The host's shape down the right: the seed with its regenerate action,
/// and the clock.
fn paint_shape(panel: &Panel<'_>, lobby: &Lobby, host: bool) -> Option<Asked> {
    let [seed, regenerate, clock] = shape_actions(panel.window());
    panel.text(
        &format!("Seed {}", lobby.seed()),
        Pos2::new(seed.right(), seed.center().y),
        Align2::RIGHT_CENTER,
        panel::INK,
        panel::BODY_SIZE,
    );
    if panel.action(regenerate, "Regenerate", host) {
        return Some(Asked::Regenerate);
    }
    if panel.action(
        clock,
        &format!("Clock · {} min", lobby.clock().seconds() as u64 / 60),
        host,
    ) {
        return Some(Asked::Edits(vec![LobbyEdit::SetClock(next_clock(
            lobby.clock(),
        ))]));
    }
    None
}

/// The actions across the bottom: Start for the host, Ready for a guest,
/// and Leave.
fn bottom(panel: &Panel<'_>, lobby: &Lobby, me: PlayerId) -> (Option<Step>, Option<Asked>) {
    let window = panel.window();
    let host = lobby.host() == me;
    let [start, leave] = bottom_actions(window);

    let frozen = lobby.freeze().ok();
    if !host {
        let asked = panel
            .action(start, "Ready", true)
            .then(|| Asked::Edits(vec![LobbyEdit::SetReady { ready: true }]));
        return (leaving(panel, leave), asked);
    }
    let picked = panel.action(start, "Start", frozen.is_some());
    match frozen {
        // The action answers true only where it was drawn enabled, which is
        // where the lobby froze.
        Some(setup) if picked => {
            return (
                Some(Step::Loading(Box::new(Loading::of(
                    lobby.clone(),
                    setup,
                    me,
                )))),
                None,
            );
        }
        Some(_) => {}
        None => panel.text(
            "Every seat held, every guest ready",
            Pos2::new(window.center().x, start.top() - panel::ROW_HEIGHT / 2.0),
            Align2::CENTER_CENTER,
            panel::DIM_INK,
            panel::BODY_SIZE,
        ),
    }
    (leaving(panel, leave), None)
}

/// The Leave action, which every viewer of a lobby has.
fn leaving(panel: &Panel<'_>, rect: Rect) -> Option<Step> {
    panel.action(rect, "Leave", true).then_some(Step::Title)
}

/// `count` lines down the left of `window`, in points.
pub fn rows(window: Rect, count: usize) -> impl Iterator<Item = Rect> {
    let top = Pos2::new(
        window.left() + panel::MARGIN,
        window.top() + panel::MARGIN * 1.5,
    );
    panel::column(top, SEAT_WIDTH, count)
}

/// The seed's own row, Regenerate and the clock, down the right of
/// `window`.
pub fn shape_actions(window: Rect) -> [Rect; 3] {
    let top = Pos2::new(
        window.right() - SHAPE_WIDTH - panel::MARGIN,
        window.top() + panel::MARGIN * 1.5,
    );
    let mut rects = [Rect::ZERO; 3];
    for (rect, laid) in rects.iter_mut().zip(panel::column(top, SHAPE_WIDTH, 3)) {
        *rect = laid;
    }
    rects
}

/// Start, or Ready for a guest, and Leave, across the bottom of `window`.
pub fn bottom_actions(window: Rect) -> [Rect; 2] {
    let start = Rect::from_min_size(
        Pos2::new(
            window.center().x - ACTION_WIDTH - panel::ROW_HEIGHT / 2.0,
            window.bottom() - panel::MARGIN - panel::ROW_HEIGHT,
        ),
        Vec2::new(ACTION_WIDTH, panel::ROW_HEIGHT),
    );
    [
        start,
        start.translate(Vec2::new(ACTION_WIDTH + panel::ROW_HEIGHT, 0.0)),
    ]
}

/// The clock a click on the clock action lands, cycling [`CLOCKS`].
fn next_clock(clock: Tick) -> Tick {
    let at = CLOCKS.iter().position(|held| *held == clock);
    CLOCKS[at.map_or(0, |at| (at + 1) % CLOCKS.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The skirmish lobby: the host on team one, a bot on team two.
    fn skirmish() -> Lobby {
        Lobby::skirmish(PlayerId::HOST)
    }

    #[test]
    fn the_seat_column_heads_every_team_that_holds_a_seat_and_draws_no_closed_seat() {
        let lobby = skirmish();

        let laid = lines(&lobby, true);

        let headings: Vec<TeamId> = laid
            .iter()
            .filter_map(|line| match line {
                Line::Heading(team) => Some(*team),
                Line::Seat(_) | Line::Actions(_) => None,
            })
            .collect();
        let seats: Vec<usize> = laid
            .iter()
            .filter_map(|line| match line {
                Line::Seat(at) => Some(*at),
                Line::Heading(_) | Line::Actions(_) => None,
            })
            .collect();

        assert_eq!(headings, [TeamId(0), TeamId(1)]);
        assert_eq!(seats, [0, 1], "the two closed slots are not drawn");
        assert_eq!(
            laid.iter()
                .filter(|line| matches!(line, Line::Actions(_)))
                .count(),
            2,
            "the host is shown its actions under each team"
        );
        assert_eq!(
            lines(&lobby, false)
                .iter()
                .filter(|line| matches!(line, Line::Actions(_)))
                .count(),
            0,
            "and a guest is shown none"
        );
    }

    #[test]
    fn adding_a_bot_to_a_team_seats_the_first_personality_in_the_first_spare_slot() {
        let mut lobby = skirmish();

        let Some(Asked::Edits(edits)) = added(&lobby, TeamId(0), Control::Bot(FIRST_BOT)) else {
            panic!("a skirmish has two spare slots");
        };
        for edit in edits {
            lobby
                .edit(PlayerId::HOST, edit)
                .expect("the host adds a bot");
        }

        assert_eq!(lobby.slots()[2].control, Control::Bot(FIRST_BOT));
        assert_eq!(lobby.slots()[2].team, TeamId(0));
        assert_eq!(
            lines(&lobby, false)
                .iter()
                .filter(|line| matches!(line, Line::Seat(_)))
                .count(),
            3
        );
    }

    #[test]
    fn a_full_lobby_has_nowhere_to_add_a_seat() {
        let mut lobby = skirmish();
        for slot in 2..MAX_SLOTS {
            lobby
                .edit(
                    PlayerId::HOST,
                    LobbyEdit::SetSlot {
                        slot,
                        control: Control::Open,
                    },
                )
                .expect("the host opens a seat");
        }

        assert_eq!(spare(&lobby), None);
        assert!(added(&lobby, TeamId(0), Control::Open).is_none());
    }

    #[test]
    fn a_bots_holder_cycles_the_personalities_and_nothing_else_cycles() {
        assert_eq!(cycled(Control::Bot(Bot::Turtle)), Control::Bot(Bot::Expand));
        assert_eq!(cycled(Control::Bot(Bot::Expand)), Control::Bot(Bot::Turtle));

        let person = Control::Player {
            player: PlayerId::HOST,
            ready: true,
        };
        assert_eq!(cycled(person), person);
        assert_eq!(cycled(Control::Open), Control::Open);
    }

    #[test]
    fn every_team_a_move_lands_is_one_a_slot_may_sit_on() {
        let mut team = TeamId(0);
        let mut walked = Vec::new();
        for _ in 0..MAX_SLOTS {
            team = next_team(team);
            assert!(usize::from(team.0) < MAX_SLOTS);
            walked.push(team);
        }

        walked.sort_unstable();
        walked.dedup();
        assert_eq!(walked.len(), MAX_SLOTS, "a seat can reach every team");
    }

    #[test]
    fn the_clock_action_walks_every_clock_the_lobby_offers_and_comes_back() {
        let mut clock = CLOCKS[0];
        let mut walked = vec![clock];
        for _ in 1..CLOCKS.len() {
            clock = next_clock(clock);
            walked.push(clock);
        }

        assert_eq!(walked, CLOCKS);
        assert_eq!(next_clock(clock), CLOCKS[0]);
        assert!(
            CLOCKS
                .iter()
                .all(|clock| probe_protocol::CLOCK_RANGE.contains(clock)),
            "a clock the lobby offers is one it takes"
        );
    }

    #[test]
    fn every_label_the_lobby_shows_is_words_and_numbers_from_one() {
        let lobby = skirmish();

        assert_eq!(team_name(TeamId(0)), "Team 1");
        assert_eq!(team_name(TeamId(3)), "Team 4");
        assert_eq!(holder_name(&lobby.slots()[0], PlayerId::HOST), "You");
        assert_eq!(
            holder_name(&lobby.slots()[1], PlayerId::HOST),
            "Bot · Expand"
        );
        assert_eq!(
            holder_name(&lobby.slots()[0], PlayerId(9)),
            "Player 1",
            "another person is numbered from one too"
        );
    }
}
