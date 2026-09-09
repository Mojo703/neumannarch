use neumannarch_protocol::Bot;
use neumannarch_sim::roster::{RAIDER, Roster};
use neumannarch_sim::{AsteroidId, Retention, Setup, TeamId};

use super::*;

fn session() -> Session {
    let setup = Setup::new(vec![TeamId(0), TeamId(1)], 0, Tick(1_000)).expect("two seats");
    Session::new(setup, Retention::shipped(), &[SeatId(0), SeatId(1)])
        .expect("both seats are seated")
}

struct Counting(u32);

impl Agent for Counting {
    fn decide(&mut self, _view: &View) -> Vec<Command> {
        self.0 += 1;
        vec![Command::Want {
            asteroid: AsteroidId(0),
            row: RAIDER,
            count: 1,
        }]
    }
}

#[test]
fn every_bot_a_lobby_can_seat_is_named_and_built_by_the_registry() {
    let roster = Roster::shipped();
    for bot in [Bot::Turtle, Bot::Expand] {
        let shipped = Shipped::of(bot);
        assert_eq!(
            Shipped::named(shipped.name).map(|named| named.bot),
            Some(bot),
            "{bot:?} answers to no shipped name"
        );
        assert_eq!(
            shipped.seated(&roster).decide(&View::of(
                session().state(),
                SeatId(0),
                &Shots::default()
            )),
            Vec::new(),
            "a bot seated before its stage runs asks for nothing"
        );
    }
    assert!(Shipped::named("scripted").is_none());
}

#[test]
fn an_agent_decides_once_a_cadence_and_stamps_its_seat_and_its_count() {
    let mut seated = Seated::new(SeatId(1), Box::new(Counting(0)));
    let mut session = session();
    while session.state().drafting() {
        session.advance();
    }
    let mut issued: Vec<Stamped> = Vec::new();
    for _ in 0..2 * DECISION_INTERVAL.0 {
        issued.extend(seated.issue(&session));
        session.advance();
    }

    assert_eq!(issued.len(), 2, "one decision a cadence, and one want each");
    assert!(
        issued
            .iter()
            .all(|stamped| stamped.issued.seat == SeatId(1))
    );
    assert_eq!(
        issued
            .iter()
            .map(|stamped| stamped.issued.seq)
            .collect::<Vec<_>>(),
        [0, 1],
        "a seat's commands are counted from zero for the match"
    );
    assert!(
        issued[0].tick < issued[1].tick,
        "both decisions were stamped at one tick"
    );
}

#[test]
fn a_bot_places_on_the_first_tick_its_stage_runs_whatever_the_cadence() {
    let mut session = session();
    let staged = session.state().draft().stages()[1];
    let mut seated = Seated::new(
        staged.seat,
        Box::new(Scripted::new(Personality::expand(), Roster::shipped())),
    );

    while session.state().draft().running() != Some(staged) {
        assert_eq!(seated.issue(&session), Vec::new(), "its stage waits");
        session.advance();
    }
    let placing = seated.issue(&session);

    let tick = session.state().tick();
    assert_ne!(
        tick.0 % DECISION_INTERVAL.0,
        u64::from(staged.seat.0) % DECISION_INTERVAL.0,
        "the tick its stage runs is not its cadence tick"
    );
    assert_eq!(placing.len(), 1, "it places at once: {placing:?}");
}
