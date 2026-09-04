use std::collections::BTreeMap;

use probe_protocol::Record;
use probe_sim::state::Batch;
use probe_sim::{Refused, Setup, Stamped, Tick};

const KEPT_MATCHES: usize = 16;

pub(crate) struct Log {
    ticks: BTreeMap<Tick, Batch>,
    clock: Tick,
}

#[derive(Default)]
pub struct Records {
    kept: Vec<Record>,
}

impl Log {
    pub(crate) fn of(clock: Tick) -> Log {
        Log {
            ticks: BTreeMap::new(),
            clock,
        }
    }

    pub(crate) fn record(&self, setup: Setup) -> Record {
        Record::played(setup, self.ticks.clone())
    }

    pub(crate) fn take(&mut self, stamped: Stamped) -> Result<(), Refused> {
        if stamped.tick > self.clock {
            return Err(Refused::Ahead);
        }
        self.ticks
            .entry(stamped.tick)
            .or_default()
            .insert(stamped.issued)
    }
}

impl Records {
    pub fn keep(&mut self, record: Record) {
        if self.kept.len() == KEPT_MATCHES {
            self.kept.remove(0);
        }
        self.kept.push(record);
    }

    pub fn kept(&self) -> &[Record] {
        &self.kept
    }
}
