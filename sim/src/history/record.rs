//! The match as a value: what it started from, and the commands that
//! reproduce it.

use crate::history::log::Log;
use crate::setup::Setup;
use crate::state::State;
use crate::time::Tick;

/// A match's settled history: the setup every machine built it from and
/// every command up to the settled tick. It moves to `protocol` when that
/// crate lands.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    setup: Setup,
    log: Log,
}

impl Record {
    pub(crate) fn new(setup: Setup, log: Log) -> Record {
        Record { setup, log }
    }

    /// What the match was set up as.
    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    /// How many commands it holds.
    pub fn commands(&self) -> usize {
        self.log.stamped().count()
    }

    /// The state at `until`, from the setup's initial state stepped over
    /// the log.
    pub fn replay(&self, until: Tick) -> State {
        let mut state = State::start(&self.setup);
        while state.tick() < until {
            let (next, _) = state.step(self.log.at(state.tick()));
            state = next;
        }
        state
    }
}
