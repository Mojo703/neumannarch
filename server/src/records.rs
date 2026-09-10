use std::collections::BTreeMap;

use neumannarch_protocol::Record;
use neumannarch_sim::state::Batch;
use neumannarch_sim::{Refused, Setup, Stamped, Tick};

const KEPT_MATCHES: usize = 16;

pub(crate) struct Log {
    ticks: BTreeMap<Tick, Batch>,
    ends_by: Tick,
}

#[derive(Default)]
pub struct Records {
    kept: Vec<Record>,
}

impl Log {
    pub(crate) fn of(ends_by: Tick) -> Log {
        Log {
            ticks: BTreeMap::new(),
            ends_by,
        }
    }

    pub(crate) fn record(&self, setup: Setup) -> Record {
        Record::played(setup, self.ticks.clone())
    }

    pub(crate) fn take(&mut self, stamped: Stamped) -> Result<(), Refused> {
        if stamped.tick > self.ends_by {
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
