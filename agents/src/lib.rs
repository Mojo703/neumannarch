//! Agents: a seat's player, and the cadence a runner calls one on.

use probe_sim::state::Command;
use probe_sim::state::view::View;
use probe_sim::step::fire::Shots;
use probe_sim::{SeatId, Sequence, Session, Stamped, TICKS_PER_SECOND, Tick};

pub use dice::Dice;
pub use personality::{Mix, Personality};
pub use roles::Roles;
pub use scripted::Scripted;

/// How many ticks pass between one agent's decisions: one second of sim
/// time, on the ground that a player's hands are no faster. A hypothesis
/// the harness confirms or kills.
pub const DECISION_INTERVAL: Tick = Tick(TICKS_PER_SECOND as u64);

/// The most wants one decision issues. Beyond this the least urgent wait
/// for the next decision, which is a second away. A hypothesis.
pub const MAX_COMMANDS_PER_DECISION: usize = 16;

// A decision lands in one tick, whose own cap would refuse its tail.
const _: () = assert!(MAX_COMMANDS_PER_DECISION <= probe_sim::state::MAX_COMMANDS_PER_TICK);

/// A seat's player: it reads its own fogged view and speaks the one verb.
///
/// Called on [`DECISION_INTERVAL`], never every tick. An implementation
/// holds no clock and no randomness but a seed it steps itself, so a match
/// of agents replays exactly.
pub trait Agent {
    /// The wants this decision issues, at most
    /// [`MAX_COMMANDS_PER_DECISION`] of them.
    fn decide(&mut self, view: &View) -> Vec<Command>;
}

/// One agent in one seat: what a runner drives, and the only place the
/// decision cadence lives.
pub struct Seated {
    sequence: Sequence,
    agent: Box<dyn Agent>,
}

impl Seated {
    /// `agent` playing `seat`.
    pub fn new(seat: SeatId, agent: Box<dyn Agent>) -> Seated {
        Seated {
            sequence: Sequence::new(seat),
            agent,
        }
    }

    /// The seat it plays.
    pub fn seat(&self) -> SeatId {
        self.sequence.seat()
    }

    /// What the agent issues at the tick `session` shows: nothing off its
    /// cadence, else its decision over that tick's fogged view, stamped
    /// with its seat and its next sequence. Seats decide on different
    /// ticks.
    pub fn issue(&mut self, session: &Session) -> Vec<Stamped> {
        let tick = session.state().tick();
        let interval = DECISION_INTERVAL.0;
        if tick.0 % interval != u64::from(self.seat().0) % interval {
            return Vec::new();
        }
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

mod dice;
mod memory;
mod personality;
mod plan;
mod roles;
mod scripted;
mod survey;

#[cfg(test)]
mod tests {
    use super::*;
    use probe_sim::roster::SCOUT;
    use probe_sim::{Band, Place, Retention, RockId, Setup, TeamId};

    /// A session of two seats, both this machine's, whose clock no test
    /// reaches.
    fn session() -> Session {
        let setup = Setup::new(vec![TeamId(0), TeamId(1)], 0, Tick(1_000)).expect("two seats");
        Session::new(setup, Retention::shipped(), &[SeatId(0), SeatId(1)])
    }

    /// An agent that counts its decisions and always asks for one scout.
    struct Counting(u32);

    impl Agent for Counting {
        fn decide(&mut self, _view: &View) -> Vec<Command> {
            self.0 += 1;
            vec![Command::Want {
                place: Place {
                    rock: RockId(0),
                    band: Band::Inner,
                },
                row: SCOUT,
                count: 1,
            }]
        }
    }

    #[test]
    fn an_agent_decides_once_a_cadence_and_stamps_its_seat_and_its_count() {
        let mut seated = Seated::new(SeatId(1), Box::new(Counting(0)));
        let mut session = session();
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
    fn a_decision_is_cut_to_the_commands_a_tick_allows() {
        struct Flooding;
        impl Agent for Flooding {
            fn decide(&mut self, _view: &View) -> Vec<Command> {
                (0..64)
                    .map(|rock| Command::Want {
                        place: Place {
                            rock: RockId(rock),
                            band: Band::Inner,
                        },
                        row: SCOUT,
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
