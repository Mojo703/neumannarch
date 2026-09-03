//! What a room keeps of the matches it has served: the log it is
//! forwarding, and the records of the matches that finished.

use std::collections::BTreeMap;

use probe_protocol::Record;
use probe_sim::state::Batch;
use probe_sim::{Refused, Setup, Stamped, Tick};

/// How many finished matches a room keeps. Records go to files in a later
/// unit; until then the oldest is dropped, so a server that runs for a
/// month holds this many and no more.
const KEPT_MATCHES: usize = 16;

/// The commands a room has forwarded, one batch per tick: the record of
/// the match as it is played.
///
/// Every command it takes is a command it forwards, so what a machine
/// learns late and what the record holds are the same set.
pub(crate) struct Ledger {
    ticks: BTreeMap<Tick, Batch>,
    clock: Tick,
}

/// The records of the matches a room has served, oldest first.
#[derive(Default)]
pub struct Records {
    kept: Vec<Record>,
}

impl Ledger {
    /// An empty log of a match ending at `clock`.
    pub(crate) fn of(clock: Tick) -> Ledger {
        Ledger {
            ticks: BTreeMap::new(),
            clock,
        }
    }

    /// The record of the match `setup` began, as forwarded so far.
    pub(crate) fn record(&self, setup: Setup) -> Record {
        Record::played(setup, self.ticks.clone())
    }

    /// Takes `stamped` into the tick it names, or refuses it by name: a
    /// tick past the clock is past every machine's reach, and a tick's own
    /// batch bounds what one seat puts in it.
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
    /// Keeps `record`, dropping the oldest past [`KEPT_MATCHES`].
    pub fn keep(&mut self, record: Record) {
        if self.kept.len() == KEPT_MATCHES {
            self.kept.remove(0);
        }
        self.kept.push(record);
    }

    /// Every record kept, oldest first.
    pub fn kept(&self) -> &[Record] {
        &self.kept
    }
}
