use std::collections::BTreeMap;

use neumannarch_sim::state::State;
use neumannarch_sim::{Batch, Refused, Session, Setup, Stamped, Tick};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadRecord {
    pub tick: Tick,
    pub reason: Refused,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(into = "Fields", try_from = "Fields")]
pub struct Record {
    setup: Setup,
    ticks: BTreeMap<Tick, Batch>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Fields {
    setup: Setup,
    log: Vec<Stamped>,
}

impl Record {
    pub fn of(session: &Session) -> Record {
        Record {
            setup: session.setup().clone(),
            ticks: folded(session.commands()).expect("a session's own log is a record"),
        }
    }

    pub fn played(setup: Setup, ticks: BTreeMap<Tick, Batch>) -> Record {
        Record { setup, ticks }
    }

    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    pub fn commands(&self) -> usize {
        self.ticks.values().map(|batch| batch.iter().count()).sum()
    }

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
    use neumannarch_sim::pattern::EntityPattern as P;
    use neumannarch_sim::state::{Command, Issued};
    use neumannarch_sim::{AsteroidId, Retention, SeatId, TeamId, Time};

    use super::*;
    use crate::wire::Codec;

    const CLOCK: Time = Time(10_000);

    const UNTIL: Tick = Tick(10_000);

    const PAST_EVERY_PATTERN: u8 = P::EVERY.len() as u8;

    #[derive(Deserialize, Serialize)]
    struct FieldsByCode {
        setup: Setup,
        log: Vec<StampedByCode>,
    }

    #[derive(Deserialize, Serialize)]
    struct StampedByCode {
        tick: Tick,
        issued: IssuedByCode,
    }

    #[derive(Deserialize, Serialize)]
    struct IssuedByCode {
        seat: SeatId,
        seq: u32,
        command: CommandByCode,
    }

    #[derive(Deserialize, Serialize)]
    enum CommandByCode {
        Want {
            asteroid: AsteroidId,
            pattern: u8,
            count: u32,
        },
    }

    fn setup() -> Setup {
        Setup::new(vec![TeamId(0), TeamId(1)], 11, CLOCK).expect("two seats are a match")
    }

    fn want(seq: u32, pattern: P) -> Issued {
        Issued {
            seat: SeatId(0),
            seq,
            command: Command::Want {
                asteroid: AsteroidId(0),
                pattern,
                count: 1,
            },
        }
    }

    fn played() -> Session {
        let mut session = Session::new(setup(), Retention::shipped(), &[SeatId(0), SeatId(1)])
            .expect("both seats are seated");
        for stamped in [
            Stamped {
                tick: Tick(2),
                issued: want(0, P::Shipyard),
            },
            Stamped {
                tick: Tick(2),
                issued: want(1, P::Constructor),
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
        assert_eq!(read.replay(UNTIL).hash(), record.replay(UNTIL).hash());
    }

    #[test]
    fn bytes_holding_a_command_twice_are_not_a_record() {
        let doubled = Fields {
            setup: setup(),
            log: vec![
                Stamped {
                    tick: Tick(2),
                    issued: want(0, P::Shipyard),
                },
                Stamped {
                    tick: Tick(2),
                    issued: want(0, P::Constructor),
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

    #[test]
    fn bytes_naming_a_code_no_pattern_has_are_not_a_record() {
        let unnamed = FieldsByCode {
            setup: setup(),
            log: vec![StampedByCode {
                tick: Tick(2),
                issued: IssuedByCode {
                    seat: SeatId(0),
                    seq: 0,
                    command: CommandByCode::Want {
                        asteroid: AsteroidId(0),
                        pattern: PAST_EVERY_PATTERN,
                        count: 1,
                    },
                },
            }],
        };

        let refused = Record::decode(&unnamed.encoded())
            .expect_err("a pattern code no pattern has is not a record")
            .to_string();

        assert!(
            refused.contains(&PAST_EVERY_PATTERN.to_string()),
            "{refused} does not name the code {PAST_EVERY_PATTERN}"
        );
    }

    #[test]
    fn a_record_of_a_want_of_every_pattern_replays_to_the_hash_it_holds() {
        for pattern in P::EVERY {
            let record = Record::played(
                setup(),
                BTreeMap::from([(Tick(2), batch(want(0, pattern)))]),
            );

            let read = Record::decode(&record.encoded()).expect("its own bytes are a record");

            assert_eq!(read, record, "{}", pattern.name());
            assert_eq!(
                read.replay(Tick(20)).hash(),
                record.replay(Tick(20)).hash(),
                "{}",
                pattern.name()
            );
        }
    }

    fn batch(issued: Issued) -> Batch {
        let mut batch = Batch::new();
        batch.insert(issued).expect("one command is a batch");
        batch
    }
}
