use mirage_engine::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use neumannarch_sim::roster::{Glyph, Roster};
use neumannarch_sim::state::{Draft, GRACE, STAGE_SPAN};
use neumannarch_sim::{AsteroidId, SeatId, Tick};

use crate::display::glyph::{self, Cell, Drawing, Look};
use crate::display::glyph_quad::seat_color32;
use crate::display::scene::{Fill, asteroid_name};
use crate::display::wheel;
use crate::display::wheels::RESTING_ALPHA;
use crate::screens::panel;

pub const LEFT: f32 = panel::MARGIN / 2.0;

pub const TOP: f32 = panel::MARGIN * 1.5;

const TITLE: &str = "Draft";

const CLOCK: &str = "Clock";

const INSET: f32 = 10.0;

const ROW_HEIGHT: f32 = 2.0 * glyph::HALF + 2.0;

const GLYPH_SLOT: f32 = 2.0 * glyph::HALF;

const GAP: f32 = 6.0;

const NAME_WIDTH: f32 = 9.0 * wheel::CHARACTER_WIDTH;

const BAR_LENGTH: f32 = 60.0;

const BAR_HEIGHT: f32 = wheel::MARK;

pub const WIDTH: f32 = INSET + GLYPH_SLOT + GAP + NAME_WIDTH + GAP + BAR_LENGTH + INSET;

pub struct Order {
    frame: Rect,
    title: Pos2,
    rows: Vec<Row>,
    alpha: f32,
}

struct Row {
    rect: Rect,
    stage: Option<(SeatId, Glyph)>,
    name: String,
    standing: Standing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Standing {
    Waiting,
    Running { left: f32 },
    RanOut,
    Placed(AsteroidId),
}

impl Order {
    pub fn over(
        window: Rect,
        draft: &Draft,
        tick: Tick,
        roster: &Roster,
        names: &[String],
        alpha: f32,
    ) -> Order {
        let stages = draft.stages();
        let running = stages
            .iter()
            .position(|stage| Some(*stage) == draft.running());
        let since = tick.0.saturating_sub(draft.began().0) as f32;
        let left = |span: Tick| (1.0 - since / span.0 as f32).clamp(0.0, 1.0);
        let corner = Pos2::new(window.left() + LEFT, window.top() + TOP);
        let title = Pos2::new(
            corner.x + WIDTH / 2.0,
            corner.y + INSET + wheel::LINE_HEIGHT / 2.0,
        );
        let first = Pos2::new(
            corner.x + INSET,
            corner.y + INSET + wheel::LINE_HEIGHT + GAP,
        );
        let row_at = |index: usize| {
            Rect::from_min_size(
                Pos2::new(first.x, first.y + index as f32 * ROW_HEIGHT),
                Vec2::new(WIDTH - 2.0 * INSET, ROW_HEIGHT),
            )
        };
        let mut rows: Vec<Row> = stages
            .iter()
            .enumerate()
            .map(|(at, stage)| Row {
                rect: row_at(at),
                stage: Some((stage.seat, roster[stage.row].glyph())),
                name: names[usize::from(stage.seat.0)].clone(),
                standing: match (stage.placed, running) {
                    (Some(asteroid), _) => Standing::Placed(asteroid),
                    (None, Some(now)) if at == now => Standing::Running {
                        left: left(STAGE_SPAN),
                    },
                    (None, Some(now)) if at > now => Standing::Waiting,
                    (None, _) => Standing::RanOut,
                },
            })
            .collect();
        if running.is_none() && draft.ended().is_none() {
            rows.push(Row {
                rect: row_at(rows.len()),
                stage: None,
                name: CLOCK.to_string(),
                standing: Standing::Running { left: left(GRACE) },
            });
        }
        let bottom = rows.last().map_or(first.y, |row| row.rect.bottom()) + INSET;
        let frame = Rect::from_min_max(corner, Pos2::new(corner.x + WIDTH, bottom));
        Order {
            frame,
            title,
            rows,
            alpha,
        }
    }

    pub fn paint(&self, painter: &egui::Painter) {
        let faded = |colour: Color32| colour.gamma_multiply(self.alpha);
        painter.rect_filled(self.frame, 0.0, faded(panel::SCRIM));
        painter.rect_stroke(
            self.frame,
            0.0,
            Stroke::new(1.0, faded(panel::LINE)),
            egui::StrokeKind::Inside,
        );
        painter.text(
            self.title,
            Align2::CENTER_CENTER,
            TITLE,
            FontId::monospace(wheel::LINE_HEIGHT),
            faded(panel::INK),
        );
        for row in &self.rows {
            let alpha = self.alpha
                * match row.standing {
                    Standing::Running { .. } => 1.0,
                    _ => RESTING_ALPHA,
                };
            row.paint(painter, alpha);
        }
    }
}

impl Row {
    fn marked(&self, seat: SeatId) -> (Fill, Color32) {
        match self.standing {
            Standing::Placed(_) => (Fill::Solid, Color32::WHITE),
            _ => (Fill::Hollow, seat_color32(seat)),
        }
    }

    fn paint(&self, painter: &egui::Painter, alpha: f32) {
        let ink = |colour: Color32| colour.gamma_multiply(alpha);
        let middle = self.rect.center().y;
        if let Some((seat, glyph)) = &self.stage {
            let (fill, outline) = self.marked(*seat);
            Drawing::of(*glyph).paint(
                painter,
                Cell {
                    centre: Pos2::new(self.rect.left() + GLYPH_SLOT / 2.0, middle),
                    half: glyph::HALF,
                },
                Look {
                    colour: seat_color32(*seat),
                    outline,
                    fill,
                    alpha,
                    starved: None,
                },
            );
        }
        painter.text(
            Pos2::new(self.rect.left() + GLYPH_SLOT + GAP, middle),
            Align2::LEFT_CENTER,
            &self.name,
            FontId::monospace(wheel::LINE_HEIGHT),
            ink(panel::INK),
        );
        let bar = Rect::from_min_size(
            Pos2::new(self.rect.right() - BAR_LENGTH, middle - BAR_HEIGHT / 2.0),
            Vec2::new(BAR_LENGTH, BAR_HEIGHT),
        );
        let fill = |left: f32| {
            painter.rect_filled(
                Rect::from_min_size(bar.min, Vec2::new(bar.width() * left, bar.height())),
                0.0,
                ink(panel::DIM_INK),
            );
            painter.rect_stroke(
                bar,
                0.0,
                Stroke::new(1.0, ink(panel::LINE)),
                egui::StrokeKind::Inside,
            );
        };
        match self.standing {
            Standing::Waiting => fill(1.0),
            Standing::Running { left } => fill(left),
            Standing::RanOut => fill(0.0),
            Standing::Placed(asteroid) => {
                painter.text(
                    Pos2::new(bar.left(), middle),
                    Align2::LEFT_CENTER,
                    asteroid_name(asteroid),
                    FontId::monospace(wheel::LINE_HEIGHT),
                    ink(panel::INK),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use neumannarch_protocol::{Lobby, PlayerId};
    use neumannarch_sim::state::Command;
    use neumannarch_sim::{Retention, Sequence, Session};

    use super::*;
    use crate::screens::lobby::seat_names;

    const WINDOW: Rect = Rect::from_min_max(Pos2::ZERO, egui::pos2(1280.0, 720.0));

    fn skirmish() -> (Session, Vec<String>) {
        let started = Lobby::skirmish(PlayerId::HOST)
            .freeze()
            .expect("a skirmish starts");
        let names = seat_names(started.seating(), PlayerId::HOST, started.setup().seed());
        let (setup, _) = started.parts();
        let session = Session::new(setup, Retention::shipped(), &[SeatId(0), SeatId(1)])
            .expect("both seats are local");
        (session, names)
    }

    fn place(session: &mut Session, seat: SeatId, asteroid: AsteroidId) {
        let stage = session
            .state()
            .draft()
            .running()
            .expect("a stage is running");
        assert_eq!(stage.seat, seat);
        let stamped = Sequence::new(seat).stamp(
            session.state().tick(),
            Command::Want {
                asteroid,
                row: stage.row,
                count: 1,
            },
        );
        session.insert(stamped).expect("the pick is taken");
        assert!(session.advance().rejected.is_empty());
    }

    fn run(session: &mut Session, ticks: u64) {
        for _ in 0..ticks {
            session.advance();
        }
    }

    fn order(session: &Session, names: &[String]) -> Order {
        let state = session.state();
        Order::over(
            WINDOW,
            state.draft(),
            state.tick(),
            state.roster(),
            names,
            1.0,
        )
    }

    #[test]
    fn a_row_per_stage_in_the_order_they_run_named_for_its_seat_under_the_title() {
        let (session, names) = skirmish();
        let order = order(&session, &names);
        let stages = session.state().draft().stages();

        assert_eq!(
            order.rows.len(),
            stages.len(),
            "no clock row while a stage runs"
        );
        assert!(
            order.rows.iter().zip(stages).all(|(row, stage)| row
                .stage
                .as_ref()
                .map(|(seat, _)| *seat)
                == Some(stage.seat)),
            "each row is its stage's seat"
        );
        assert_eq!(
            order
                .rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            stages
                .iter()
                .map(|stage| names[usize::from(stage.seat.0)].as_str())
                .collect::<Vec<_>>()
        );
        assert!(
            order
                .rows
                .windows(2)
                .all(|pair| (pair[0].rect.bottom() - pair[1].rect.top()).abs() < 1e-3),
            "rows stack downward without a gap"
        );
        assert!(order.frame.left() >= WINDOW.left() && order.frame.top() > 0.0);
        assert!((order.frame.width() - WIDTH).abs() < 1e-3);
        assert!(
            order.title.y < order.rows[0].rect.top(),
            "the title stands above the rows"
        );
        assert!(
            order
                .rows
                .iter()
                .all(|row| order.frame.contains_rect(row.rect)),
            "the panel holds every row"
        );
    }

    #[test]
    fn the_running_stage_drains_over_its_span_and_one_that_ran_out_keeps_an_empty_bar() {
        let (mut session, names) = skirmish();
        let first = order(&session, &names);
        assert_eq!(first.rows[0].standing, Standing::Running { left: 1.0 });
        assert_eq!(first.rows[1].standing, Standing::Waiting);

        run(&mut session, STAGE_SPAN.0 / 2);
        let half = order(&session, &names);
        let Standing::Running { left } = half.rows[0].standing else {
            panic!("the first stage still runs: {:?}", half.rows[0].standing);
        };
        assert!((left - 0.5).abs() < 1e-3, "{left}");

        run(&mut session, STAGE_SPAN.0 / 2 + 1);
        let passed = order(&session, &names);
        assert_eq!(passed.rows[0].standing, Standing::RanOut);
        assert!(matches!(passed.rows[1].standing, Standing::Running { .. }));
    }

    #[test]
    fn a_placed_stage_names_its_asteroid_and_fills_its_glyph() {
        let (mut session, names) = skirmish();
        let seat = session.state().draft().stages()[0].seat;
        place(&mut session, seat, AsteroidId(2));

        let order = order(&session, &names);

        assert_eq!(order.rows[0].standing, Standing::Placed(AsteroidId(2)));
        assert_eq!(asteroid_name(AsteroidId(2)), "Asteroid 3");
        assert!(matches!(order.rows[1].standing, Standing::Running { .. }));
        for row in &order.rows {
            let (seat, _) = row.stage.as_ref().expect("a stage row");
            let seat = *seat;
            assert_eq!(
                row.marked(seat),
                match row.standing {
                    Standing::Placed(_) => (Fill::Solid, Color32::WHITE),
                    _ => (Fill::Hollow, seat_color32(seat)),
                },
                "the glyph is filled in the seat's colour once placed and hollow in it before"
            );
        }
        assert_ne!(seat_color32(SeatId(0)), seat_color32(SeatId(1)));
    }

    #[test]
    fn once_the_last_stage_ends_a_clock_row_drains_the_grace() {
        let (mut session, names) = skirmish();
        let stages = session.state().draft().stages().len() as u64;
        run(&mut session, stages * STAGE_SPAN.0 + GRACE.0 / 2);
        assert!(session.state().drafting(), "the grace is running");

        let order = order(&session, &names);

        assert_eq!(order.rows.len(), stages as usize + 1);
        assert!(
            order.rows[..stages as usize]
                .iter()
                .all(|row| row.standing == Standing::RanOut)
        );
        let clock = order.rows.last().expect("the clock row");
        assert!(clock.stage.is_none(), "the clock places nothing");
        assert_eq!(clock.name, CLOCK);
        let Standing::Running { left } = clock.standing else {
            panic!("the grace drains: {:?}", clock.standing);
        };
        assert!((left - 0.5).abs() < 0.01, "{left}");
    }
}
