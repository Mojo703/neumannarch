use neumannarch_sim::state::Command;
use neumannarch_sim::state::view::View;
use neumannarch_sim::step::fire::Shots;
use neumannarch_sim::{SeatId, Sequence, Session, Stamped, TICKS_PER_SECOND, Tick};

pub use bots::scripted::Scripted;
pub use bots::scripted::personality::{Mix, Personality};
pub use bots::{Shipped, shipped};
pub use harness::guarantees::Guarantees;
pub use harness::played_match::{PlayedMatch, free_for_all, minutes};

pub const DECISION_INTERVAL: Tick = Tick(TICKS_PER_SECOND as u64);

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
            .map(|command| self.sequence.stamp(tick, command))
            .collect()
    }
}

mod bots;
mod harness;

#[cfg(test)]
mod tests;
