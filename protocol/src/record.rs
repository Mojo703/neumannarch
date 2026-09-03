//! A match as a value: what it started from, and the commands that
//! reproduce it.

use std::collections::BTreeMap;

use probe_sim::state::State;
use probe_sim::{Batch, Refused, Session, Setup, Stamped, Tick};
use serde::{Deserialize, Serialize};

/// Why bytes are not a record: a command the tick's batch would not take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadRecord {
    pub tick: Tick,
    pub reason: Refused,
}

/// A match's settled history: the setup every machine built it from, and
/// every command up to the settled tick, one batch per tick.
///
/// It travels as the setup and a flat list of commands; reading one folds
/// that list into its ticks, so a record that replays is the only one that
/// exists.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(into = "Fields", try_from = "Fields")]
pub struct Record {
    setup: Setup,
    ticks: BTreeMap<Tick, Batch>,
}

/// A [`Record`] as it travels.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Fields {
    setup: Setup,
    log: Vec<Stamped>,
}

impl Record {
    /// What `session` has played up to its settled tick.
    pub fn of(session: &Session) -> Record {
        // A session's own log is already one batch per tick, so the fold
        // refuses nothing it hands out.
        Record {
            setup: session.setup().clone(),
            ticks: folded(session.commands()).expect("a session's own log is a record"),
        }
    }

    /// What the match was set up as.
    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    /// How many commands it holds.
    pub fn commands(&self) -> usize {
        self.ticks.values().map(|batch| batch.iter().count()).sum()
    }

    /// The state at `until`, from the setup's initial state stepped over
    /// the log.
    pub fn replay(&self, until: Tick) -> State {
        let mut state = State::start(&self.setup);
        let nothing = Batch::new();
        while state.tick() < until {
            let batch = self.ticks.get(&state.tick()).unwrap_or(&nothing);
            let (next, _) = state.step(batch);
            state = next;
        }
        state
    }
}

impl From<Record> for Fields {
    fn from(record: Record) -> Fields {
        Fields {
            setup: record.setup,
            log: record
                .ticks
                .iter()
                .flat_map(|(tick, batch)| {
                    batch.iter().map(move |issued| Stamped {
                        tick: *tick,
                        issued,
                    })
                })
                .collect(),
        }
    }
}

impl TryFrom<Fields> for Record {
    type Error = BadRecord;

    fn try_from(fields: Fields) -> Result<Record, BadRecord> {
        Ok(Record {
            setup: fields.setup,
            ticks: folded(fields.log)?,
        })
    }
}

impl core::fmt::Display for BadRecord {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(out, "{:?} at tick {}", self.reason, self.tick.0)
    }
}

/// `log` as one batch per tick, or the first command a batch would not
/// take: a repeated `(tick, seat, seq)`, or a seat past the tick's cap.
fn folded(log: Vec<Stamped>) -> Result<BTreeMap<Tick, Batch>, BadRecord> {
    let mut ticks: BTreeMap<Tick, Batch> = BTreeMap::new();
    for stamped in log {
        ticks
            .entry(stamped.tick)
            .or_default()
            .insert(stamped.issued)
            .map_err(|reason| BadRecord {
                tick: stamped.tick,
                reason,
            })?;
    }
    Ok(ticks)
}

#[cfg(test)]
mod tests {
    use probe_sim::roster::{CONSTRUCTOR, SHIPYARD};
    use probe_sim::state::{Command, Issued};
    use probe_sim::{Band, Place, Retention, RockId, RowId, SeatId, TeamId};

    use super::*;
    use crate::wire::Wire;

    /// A clock no test reaches.
    const CLOCK: Tick = Tick(10_000);

    fn setup() -> Setup {
        Setup::new(vec![TeamId(0), TeamId(1)], 11, CLOCK).expect("two seats are a match")
    }

    fn want(seq: u32, row: RowId) -> Issued {
        Issued {
            seat: SeatId(0),
            seq,
            command: Command::Want {
                place: Place {
                    rock: RockId(0),
                    band: Band::Inner,
                },
                row,
                count: 1,
            },
        }
    }

    /// A match of a few ticks with both seats this machine's, so every tick
    /// it plays is settled.
    fn played() -> Session {
        let mut session = Session::new(setup(), Retention::shipped(), &[SeatId(0), SeatId(1)])
            .expect("both seats are seated");
        for stamped in [
            Stamped {
                tick: Tick(2),
                issued: want(0, SHIPYARD),
            },
            Stamped {
                tick: Tick(2),
                issued: want(1, CONSTRUCTOR),
            },
        ] {
            session.insert(stamped).expect("a command of its own tick");
        }
        for _ in 0..20 {
            session.advance();
        }
        session
    }

    #[test]
    fn a_record_of_a_session_replays_to_the_hash_that_session_holds() {
        let session = played();
        let settled = session.settled();

        let record = Record::of(&session);

        assert_eq!(record.setup(), &setup());
        assert_eq!(record.commands(), 2);
        assert_eq!(
            Some(record.replay(settled).hash()),
            session.hash_at(settled)
        );
    }

    #[test]
    fn a_record_survives_the_wire_and_replays_the_same() {
        let record = Record::of(&played());

        let read = Record::decode(&record.encoded()).expect("its own bytes are a record");

        assert_eq!(read, record);
        assert_eq!(read.replay(CLOCK).hash(), record.replay(CLOCK).hash());
    }

    #[test]
    fn bytes_holding_a_command_twice_are_not_a_record() {
        let doubled = Fields {
            setup: setup(),
            log: vec![
                Stamped {
                    tick: Tick(2),
                    issued: want(0, SHIPYARD),
                },
                Stamped {
                    tick: Tick(2),
                    issued: want(0, CONSTRUCTOR),
                },
            ],
        };

        let refused = Record::decode(&doubled.encoded())
            .expect_err("a repeated (seat, seq) is not a record")
            .to_string();

        let named = BadRecord {
            tick: Tick(2),
            reason: Refused::Duplicate,
        }
        .to_string();
        assert!(refused.contains(&named), "{refused} does not name {named}");
    }
}
