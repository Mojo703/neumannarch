use mirage_engine::egui::{Align2, Color32, Pos2, Rect, Vec2};
use mirage_engine::mesh::{Holds, Sphere};
use mirage_engine::prelude::FrameCtx;
use probe_agents::Personality;
use probe_protocol::{Bot, Holder, Lobby, LobbyEdit, MAX_SLOTS, PlayerId, Refused};
use probe_sim::belt::Belt;
use probe_sim::{TICKS_PER_SECOND, TeamId, Tick};

use crate::controls::Button;
use crate::display::camera::BeltCamera;
use crate::display::glyph_quad::{GlyphQuad, seat_color32};
use crate::display::label::titled;
use crate::display::scene::Scene;
use crate::display::viewport::Viewport;
use crate::display::{belt, hud};
use crate::screens::Playable;
use crate::screens::control::{Chose, Controls, Rule, Value, Valued};
use crate::screens::field::{Allow, Field, MAX_SEED, Typed};
use crate::screens::panel::{self, Panel};
use crate::screens::panning::Panning;

pub const PREVIEW_ZOOM: f64 = 12_000.0;

const CLOCKS: [Tick; 4] = [
    Tick(60 * TICKS_PER_SECOND as u64),
    Tick(5 * 60 * TICKS_PER_SECOND as u64),
    Tick(15 * 60 * TICKS_PER_SECOND as u64),
    Tick(30 * 60 * TICKS_PER_SECOND as u64),
];

const BOTS: [Bot; 2] = [Bot::Turtle, Bot::Expand];

const SEAT_WIDTH: f32 = 52.0;

const SETTINGS_WIDTH: f32 = 320.0;

const LABEL_SHARE: f32 = 0.3;

const HOLDER_WIDTH: f32 = 150.0;

const KICK_WIDTH: f32 = 60.0;

const TEAM_WIDTH: f32 = 104.0;

const READY_WIDTH: f32 = 62.0;

const CELL_GAP: f32 = 8.0;

const ACTION_WIDTH: f32 = 150.0;

const TABLE_WIDTH: f32 =
    SEAT_WIDTH + HOLDER_WIDTH + KICK_WIDTH + TEAM_WIDTH + READY_WIDTH + 4.0 * CELL_GAP;

pub struct LobbyScreen {
    lobby: Lobby,
    me: PlayerId,
    scene: Scene,
    camera: BeltCamera,
    laid: u64,
    seed: Field,
    open: Option<Open>,
    panning: Panning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    Holder(usize),
    Team(usize),
    Clock,
}

#[derive(Default)]
pub struct Asked {
    pub edits: Vec<LobbyEdit>,
    pub picked: Option<Picked>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked {
    Leave,
    Start,
}

pub struct Places {
    pub head: Row,
    pub rows: Vec<Row>,
    pub seed: Rect,
    pub clock: Rect,
    pub act: Rect,
    pub leave: Rect,
}

pub struct Row {
    pub slot: usize,
    pub seat: Rect,
    pub holder: Rect,
    pub kick: Rect,
    pub team: Rect,
    pub ready: Rect,
}

impl LobbyScreen {
    pub fn of(lobby: Lobby, me: PlayerId) -> LobbyScreen {
        let laid = lobby.seed();
        let scene = belt_from(laid);
        let camera = BeltCamera::new(scene.centre(), PREVIEW_ZOOM);
        LobbyScreen {
            lobby,
            me,
            scene,
            camera,
            laid,
            seed: Field::holding(&laid.to_string(), Allow::Digits(MAX_SEED)),
            open: None,
            panning: Panning::still(),
        }
    }

    pub fn frame<G: Playable>(&mut self, ctx: &mut FrameCtx<'_, G>) -> Asked
    where
        G::Meshes: Holds<GlyphQuad> + Holds<Sphere>,
    {
        if self.laid != self.lobby.seed() {
            self.laid = self.lobby.seed();
            self.scene = belt_from(self.laid);
        }

        let mut points_per_pixel = 1.0;
        ctx.ui(|ui| points_per_pixel = 1.0 / ui.ctx().pixels_per_point());
        let size = ctx.window_size();
        let window = panel::window_of(size, points_per_pixel);
        self.panning.drag(ctx, &mut self.camera, size, true);
        let viewport = Viewport::of(&self.camera, size, points_per_pixel);
        belt::draw(&self.scene, &viewport, ctx);

        let pointer = viewport.point_at(ctx.pointer());
        let clicked = ctx.pressed(Button::Select);
        let mut asked = Asked::default();
        ctx.ui(|ui| {
            hud::paint(&self.scene, &viewport, None, ui.painter());
            let panel = Panel::new(ui.painter(), window, pointer, clicked);
            asked = self.paint(&panel, &Typed::this_frame(ui.ctx()));
        });
        asked
    }

    pub fn lobby(&self) -> &Lobby {
        &self.lobby
    }

    pub fn takes(&mut self, lobby: Lobby) {
        self.lobby = lobby;
    }

    fn paint(&mut self, panel: &Panel<'_>, typed: &Typed) -> Asked {
        let places = Places::over(panel.window());
        let mut controls = Controls::over(panel);
        let mut asked = Asked::default();

        for (label, cell) in [
            ("Seat", places.head.seat),
            ("Holder", places.head.holder),
            ("Team", places.head.team),
            ("Ready", places.head.ready),
        ] {
            panel.label(
                label,
                Pos2::new(cell.left(), cell.center().y),
                panel::DIM_INK,
            );
        }
        for row in &places.rows {
            self.paint_row(panel, &mut controls, row, &mut asked.edits);
        }
        self.paint_shape(panel, &mut controls, &places, typed, &mut asked.edits);
        self.paint_bottom(&mut controls, &places, &mut asked);

        if controls.finish() {
            self.open = None;
        }
        asked
    }

    fn paint_row(
        &mut self,
        panel: &Panel<'_>,
        controls: &mut Controls<'_>,
        row: &Row,
        edits: &mut Vec<LobbyEdit>,
    ) {
        let slot = self.lobby.slots()[row.slot];
        let square = Rect::from_center_size(
            Pos2::new(
                row.seat.left() + panel::ROW_HEIGHT / 2.0,
                row.seat.center().y,
            ),
            Vec2::splat(panel::ROW_HEIGHT * 0.6),
        );
        panel.painter().rect_filled(
            square,
            0.0,
            self.lobby
                .seat_of(row.slot)
                .map_or(Color32::from_gray(50), seat_color32),
        );
        panel.text(
            &format!("{}", row.slot + 1),
            square.center(),
            Align2::CENTER_CENTER,
            panel::INK,
            panel::BODY_SIZE,
        );

        let holds = self.rule(LobbyEdit::SetSlot {
            slot: row.slot,
            holder: slot.holder,
        });
        let chosen = controls.choice(
            row.holder,
            &self.holder_name(slot.holder),
            &self.holder_choices(row.slot),
            &holds,
            self.open == Some(Open::Holder(row.slot)),
        );
        match chosen {
            Some(Chose::Toggled) => self.toggle(Open::Holder(row.slot)),
            Some(Chose::Value(holder)) => {
                self.open = None;
                edits.push(LobbyEdit::SetSlot {
                    slot: row.slot,
                    holder,
                });
            }
            None => {}
        }

        if let Holder::Player { player, .. } = slot.holder
            && player != self.lobby.host()
            && controls.action(row.kick, "Kick", &self.rule(LobbyEdit::Kick(player)))
        {
            edits.push(LobbyEdit::Kick(player));
        }

        if slot.holder == Holder::Closed {
            return;
        }

        let sits = self.rule(LobbyEdit::SetTeam {
            slot: row.slot,
            team: slot.team,
        });
        let chosen = controls.choice(
            row.team,
            &team_name(slot.team),
            &self.team_choices(row.slot),
            &sits,
            self.open == Some(Open::Team(row.slot)),
        );
        match chosen {
            Some(Chose::Toggled) => self.toggle(Open::Team(row.slot)),
            Some(Chose::Value(team)) => {
                self.open = None;
                edits.push(LobbyEdit::SetTeam {
                    slot: row.slot,
                    team,
                });
            }
            None => {}
        }

        panel.label(
            match slot.holder {
                Holder::Player { ready: true, .. } => "Ready",
                Holder::Player { ready: false, .. } => "Waiting",
                Holder::Open | Holder::Closed | Holder::Bot(_) => "",
            },
            Pos2::new(row.ready.left(), row.ready.center().y),
            panel::DIM_INK,
        );
    }

    fn paint_shape(
        &mut self,
        panel: &Panel<'_>,
        controls: &mut Controls<'_>,
        places: &Places,
        typed: &Typed,
        edits: &mut Vec<LobbyEdit>,
    ) {
        for (label, control) in [("Seed", places.seed), ("Clock", places.clock)] {
            panel.label(
                label,
                Pos2::new(
                    control.left() - SETTINGS_WIDTH * LABEL_SHARE,
                    control.center().y,
                ),
                panel::DIM_INK,
            );
        }
        let seeds = self.rule(LobbyEdit::SetSeed(self.lobby.seed()));
        self.seed.shows(&self.lobby.seed().to_string());
        let typing = controls.value(
            Valued {
                rect: places.seed,
                rule: &seeds,
                inside: Some(("Random", &seeds)),
                state: None,
            },
            &mut self.seed,
            typed,
        );
        if typing.acted {
            edits.push(LobbyEdit::SetSeed(self.lobby.next_seed()));
        } else if let Ok(seed) = self.seed.text().parse::<u64>()
            && seed != self.lobby.seed()
        {
            edits.push(LobbyEdit::SetSeed(seed));
        }

        let clocks = self.rule(LobbyEdit::SetClock(self.lobby.clock()));
        let chosen = controls.choice(
            places.clock,
            &clock_name(self.lobby.clock()),
            &self.clock_choices(),
            &clocks,
            self.open == Some(Open::Clock),
        );
        match chosen {
            Some(Chose::Toggled) => self.toggle(Open::Clock),
            Some(Chose::Value(clock)) => {
                self.open = None;
                edits.push(LobbyEdit::SetClock(clock));
            }
            None => {}
        }
    }

    fn paint_bottom(&self, controls: &mut Controls<'_>, places: &Places, asked: &mut Asked) {
        match self.lobby.host() == self.me {
            true => {
                let starts =
                    Rule::unless(self.lobby.freeze().err().map(|why| self.start_reason(why)));
                if controls.main_action(places.act, "Start", &starts) {
                    asked.picked = Some(Picked::Start);
                }
            }
            false => {
                let readies = match self.lobby.readied(self.me) {
                    true => Rule::refuses("You are ready"),
                    false => self.rule(LobbyEdit::SetReady { ready: true }),
                };
                if controls.main_action(places.act, "Ready", &readies) {
                    asked.edits.push(LobbyEdit::SetReady { ready: true });
                }
            }
        }
        if controls.action(places.leave, "Leave", &Rule::Allows) {
            asked.picked = Some(Picked::Leave);
        }
    }

    fn rule(&self, edit: LobbyEdit) -> Rule {
        Rule::unless(
            self.lobby
                .clone()
                .edit(self.me, edit)
                .err()
                .map(|why| refusal_sentence(why, edit)),
        )
    }

    fn start_reason(&self, why: probe_protocol::NotReady) -> String {
        let team = |slot: usize| {
            self.lobby
                .slots()
                .get(slot)
                .map_or_else(|| "a seat".to_string(), |slot| team_name(slot.team))
        };
        match why {
            probe_protocol::NotReady::NoSeats => "The lobby holds no seats".to_string(),
            probe_protocol::NotReady::OpenSeat { slot } => {
                format!("Waiting for a player to take a seat on {}", team(slot))
            }
            probe_protocol::NotReady::Unready { slot } => {
                format!("Waiting for {} to be ready", team(slot))
            }
            probe_protocol::NotReady::HostUnseated => "You hold no seat in this match".to_string(),
        }
    }

    fn toggle(&mut self, choice: Open) {
        self.open = match self.open == Some(choice) {
            true => None,
            false => Some(choice),
        };
    }

    fn holder_choices(&self, slot: usize) -> Vec<Value<Holder>> {
        [
            Holder::Player {
                player: self.me,
                ready: false,
            },
            Holder::Open,
            Holder::Closed,
        ]
        .into_iter()
        .chain(BOTS.map(Holder::Bot))
        .map(|holder| Value {
            value: holder,
            label: self.holder_name(holder),
            rule: self.rule(LobbyEdit::SetSlot { slot, holder }),
        })
        .collect()
    }

    fn team_choices(&self, slot: usize) -> Vec<Value<TeamId>> {
        (0..MAX_SLOTS as u8)
            .map(TeamId)
            .map(|team| Value {
                value: team,
                label: team_name(team),
                rule: self.rule(LobbyEdit::SetTeam { slot, team }),
            })
            .collect()
    }

    fn clock_choices(&self) -> Vec<Value<Tick>> {
        CLOCKS
            .map(|clock| Value {
                value: clock,
                label: clock_name(clock),
                rule: self.rule(LobbyEdit::SetClock(clock)),
            })
            .into_iter()
            .collect()
    }

    fn holder_name(&self, holder: Holder) -> String {
        match holder {
            Holder::Open => "Open".to_string(),
            Holder::Closed => "Closed".to_string(),
            Holder::Bot(bot) => titled(Personality::of(bot).name),
            Holder::Player { player, .. } if player == self.me => "You".to_string(),
            Holder::Player { player, .. } => player_name(player),
        }
    }
}

impl Places {
    pub fn over(window: Rect) -> Places {
        let top = Pos2::new(
            window.left() + panel::MARGIN,
            window.top() + panel::MARGIN * 1.5,
        );
        let mut table = panel::column(top, TABLE_WIDTH, MAX_SLOTS + 1);
        let head = Row::over(table.next().unwrap_or(Rect::ZERO), MAX_SLOTS);
        let rows = table
            .enumerate()
            .map(|(slot, rect)| Row::over(rect, slot))
            .collect();

        let [seed, clock] = panel::rows(
            Pos2::new(
                window.right() - SETTINGS_WIDTH - panel::MARGIN,
                window.top() + panel::MARGIN * 1.5,
            ),
            SETTINGS_WIDTH,
        );
        let act = Rect::from_min_size(
            Pos2::new(
                window.center().x - ACTION_WIDTH - panel::ROW_HEIGHT / 2.0,
                window.bottom() - panel::MARGIN - panel::ROW_HEIGHT,
            ),
            Vec2::new(ACTION_WIDTH, panel::ROW_HEIGHT),
        );
        Places {
            head,
            rows,
            seed: settings_control(seed),
            clock: settings_control(clock),
            act,
            leave: act.translate(Vec2::new(ACTION_WIDTH + panel::ROW_HEIGHT, 0.0)),
        }
    }
}

impl Row {
    fn over(rect: Rect, slot: usize) -> Row {
        let cell = |left: f32, width: f32| {
            Rect::from_min_size(Pos2::new(left, rect.top()), Vec2::new(width, rect.height()))
        };
        let seat = cell(rect.left(), SEAT_WIDTH);
        let holder = cell(seat.right() + CELL_GAP, HOLDER_WIDTH);
        let kick = cell(holder.right() + CELL_GAP, KICK_WIDTH);
        let team = cell(kick.right() + CELL_GAP, TEAM_WIDTH);
        Row {
            slot,
            seat,
            holder,
            kick,
            team,
            ready: cell(team.right() + CELL_GAP, READY_WIDTH),
        }
    }
}

fn settings_control(row: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(row.left() + row.width() * LABEL_SHARE, row.top()),
        row.max,
    )
}

fn team_name(team: TeamId) -> String {
    format!("Team {}", team.0 as u16 + 1)
}

fn player_name(player: PlayerId) -> String {
    format!("Player {}", player.0 as u64 + 1)
}

fn clock_name(clock: Tick) -> String {
    match clock.seconds() as u64 / 60 {
        1 => "1 minute".to_string(),
        minutes => format!("{minutes} minutes"),
    }
}

fn refusal_sentence(why: Refused, edit: LobbyEdit) -> String {
    match why {
        Refused::NotHost => format!("Only the host changes {}", subject(edit)),
        Refused::NotYours => "That seat is not yours".to_string(),
        Refused::NotSeated => "You hold no seat".to_string(),
        Refused::NoSuchSlot => "There is no such seat".to_string(),
        Refused::BadTeam => "There is no such team".to_string(),
        Refused::BadClock => "The clock does not go that far".to_string(),
        Refused::AlreadySeated => "That player already holds a seat".to_string(),
        Refused::NotAGuest => "That seat holds no guest".to_string(),
        Refused::HeldByAGuest => "Kick this player to open their seat".to_string(),
    }
}

fn subject(edit: LobbyEdit) -> &'static str {
    match edit {
        LobbyEdit::SetSlot { .. } => "who holds a seat",
        LobbyEdit::Kick(_) => "who is in the room",
        LobbyEdit::SetTeam { .. } => "a seat's team",
        LobbyEdit::SetSeed(_) => "the seed",
        LobbyEdit::SetClock(_) => "the clock",
        LobbyEdit::SetReady { .. } => "readiness",
    }
}

fn belt_from(_seed: u64) -> Scene {
    Scene::of_belt(&Belt::fixed(Belt::GRAVITY), Belt::GRAVITY, Tick::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skirmish() -> Lobby {
        Lobby::skirmish(PlayerId::HOST)
    }

    fn window() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0))
    }

    #[test]
    fn the_table_holds_one_row_per_seat_the_match_can_hold_whatever_holds_them() {
        let places = Places::over(window());

        assert_eq!(
            places.rows.iter().map(|row| row.slot).collect::<Vec<_>>(),
            (0..MAX_SLOTS).collect::<Vec<_>>()
        );
        assert!(
            places
                .rows
                .windows(2)
                .all(|pair| pair[0].seat.top() < pair[1].seat.top()),
            "the rows stand in slot order down the table"
        );
        for row in &places.rows {
            assert!(row.seat.right() <= row.holder.left());
            assert!(row.holder.right() <= row.kick.left());
            assert!(row.kick.right() <= row.team.left());
            assert!(row.team.right() <= row.ready.left());
            assert!(
                row.ready.right() < places.seed.left(),
                "no cell reaches the settings"
            );
        }
        assert!(places.head.seat.bottom() <= places.rows[0].seat.top());
    }

    #[test]
    fn a_closed_seat_keeps_its_row_and_offers_only_its_holder() {
        let lobby = skirmish();
        let closed = lobby
            .slots()
            .iter()
            .position(|slot| slot.holder == Holder::Closed)
            .expect("a skirmish closes the seats it does not use");
        let screen = LobbyScreen::of(lobby, PlayerId::HOST);

        assert_eq!(
            screen.holder_name(screen.lobby.slots()[closed].holder),
            "Closed"
        );
        assert!(
            screen
                .holder_choices(closed)
                .iter()
                .any(|holder| holder.rule.allows()),
            "the host can put something in a closed seat"
        );
    }

    #[test]
    fn a_skirmish_can_be_started_the_moment_its_lobby_opens() {
        let screen = LobbyScreen::of(skirmish(), PlayerId::HOST);

        let starts = Rule::unless(
            screen
                .lobby
                .freeze()
                .err()
                .map(|why| screen.start_reason(why)),
        );

        assert!(
            starts.allows(),
            "a skirmish opens on a match: {:?}",
            starts.why()
        );
    }

    #[test]
    fn every_label_the_lobby_shows_is_words_and_numbers_from_one() {
        let screen = LobbyScreen::of(skirmish(), PlayerId::HOST);

        assert_eq!(team_name(TeamId(0)), "Team 1");
        assert_eq!(team_name(TeamId(3)), "Team 4");
        assert_eq!(clock_name(CLOCKS[0]), "1 minute");
        assert_eq!(clock_name(CLOCKS[2]), "15 minutes");
        assert_eq!(player_name(PlayerId(1)), "Player 2");
        assert_eq!(screen.holder_name(screen.lobby.slots()[0].holder), "You");
        assert_eq!(screen.holder_name(screen.lobby.slots()[1].holder), "Expand");
        assert_eq!(screen.holder_name(screen.lobby.slots()[2].holder), "Closed");
    }

    #[test]
    fn every_clock_the_choice_offers_is_one_the_lobby_takes() {
        let screen = LobbyScreen::of(skirmish(), PlayerId::HOST);

        assert!(
            screen
                .clock_choices()
                .iter()
                .all(|clock| clock.rule.allows()),
            "the host is offered every clock"
        );
        assert!(
            CLOCKS
                .iter()
                .all(|clock| probe_protocol::CLOCK_RANGE.contains(clock))
        );
    }

    #[test]
    fn a_guests_controls_are_disabled_by_the_rule_that_would_refuse_them() {
        let mut lobby = Lobby::room(PlayerId::HOST);
        let guest = PlayerId(1);
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 1,
                    holder: Holder::Player {
                        player: guest,
                        ready: false,
                    },
                },
            )
            .expect("the host seats the guest");
        let screen = LobbyScreen::of(lobby, guest);

        let seed = screen.rule(LobbyEdit::SetSeed(7));
        let own_team = screen.rule(LobbyEdit::SetTeam {
            slot: 1,
            team: TeamId(2),
        });
        let other_team = screen.rule(LobbyEdit::SetTeam {
            slot: 0,
            team: TeamId(2),
        });
        let kick = screen.rule(LobbyEdit::Kick(PlayerId::HOST));

        assert!(!seed.allows());
        assert!(own_team.allows(), "a guest owns its own seat's team");
        assert!(!other_team.allows());
        assert!(!kick.allows());
        assert_eq!(
            refusal_sentence(Refused::NotHost, LobbyEdit::SetSeed(7)),
            "Only the host changes the seed"
        );
    }
}
