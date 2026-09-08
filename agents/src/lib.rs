use neumannarch_sim::state::Command;
use neumannarch_sim::state::view::View;
use neumannarch_sim::step::fire::Shots;
use neumannarch_sim::{SeatId, Sequence, Session, Stamped, TICKS_PER_SECOND, Tick};

pub use dice::Dice;
pub use guarantees::Guarantees;
pub use personality::{Mix, Personality};
pub use played_match::{PlayedMatch, free_for_all, minutes};
pub use roles::Roles;
pub use scripted::Scripted;

pub const DECISION_INTERVAL: Tick = Tick(TICKS_PER_SECOND as u64);

pub const MAX_COMMANDS_PER_DECISION: usize = 16;

const _: () = assert!(MAX_COMMANDS_PER_DECISION <= neumannarch_sim::state::MAX_COMMANDS_PER_TICK);

pub trait Agent {
    fn decide(&mut self, view: &View) -> Vec<Command>;
}

pub struct Seated {
    sequence: Sequence,
    agent: Box<dyn Agent>,
}

impl Seated {
    pub fn new(seat: SeatId, agent: Box<dyn Agent>) -> Seated {
        Seated {
            sequence: Sequence::new(seat),
            agent,
        }
    }

    pub fn seat(&self) -> SeatId {
        self.sequence.seat()
    }

    pub fn deciding(&self, session: &Session) -> bool {
        let tick = session.state().tick();
        let interval = DECISION_INTERVAL.0;
        let staged = session
            .state()
            .draft()
            .running()
            .is_some_and(|stage| stage.seat == self.seat());
        staged || tick.0 % interval == u64::from(self.seat().0) % interval
    }

    pub fn issue(&mut self, session: &Session) -> Vec<Stamped> {
        if !self.deciding(session) {
            return Vec::new();
        }
        let tick = session.state().tick();
        let quiet = Shots::default();
        let shots = session.outcome().map_or(&quiet, |outcome| &outcome.shots);
        let view = View::of(session.state(), self.seat(), shots);
        self.agent
            .decide(&view)
            .into_iter()
            .take(MAX_COMMANDS_PER_DECISION)
            .map(|command| self.sequence.stamp(tick, command))
            .collect()
    }
}

mod commitments;
mod dice;
mod guarantees;
mod personality;
mod plan;
mod played_match;
mod ranking;
mod roles;
mod scripted;
mod survey;

#[cfg(test)]
mod tests {
    use super::*;
    use neumannarch_sim::roster::{RAIDER, Roster};
    use neumannarch_sim::{AsteroidId, Retention, Setup, TeamId};

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

    #[test]
    fn a_decision_is_cut_to_the_commands_a_tick_allows() {
        struct Flooding;
        impl Agent for Flooding {
            fn decide(&mut self, _view: &View) -> Vec<Command> {
                (0..64)
                    .map(|at| Command::Want {
                        asteroid: AsteroidId(at),
                        row: RAIDER,
                        count: 1,
                    })
                    .collect()
            }
        }

        let mut seated = Seated::new(SeatId(0), Box::new(Flooding));

        let issued = seated.issue(&session());

        assert_eq!(issued.len(), MAX_COMMANDS_PER_DECISION);
    }
}
