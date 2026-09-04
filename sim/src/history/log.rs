use std::collections::BTreeMap;

use crate::state::{Batch, Refused, Stamped};
use crate::time::Tick;

static NOTHING: Batch = Batch::new();

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Log {
    ticks: BTreeMap<Tick, Batch>,
}

impl Log {
    pub(crate) fn insert(&mut self, stamped: Stamped) -> Result<(), Refused> {
        self.ticks
            .entry(stamped.tick)
            .or_default()
            .insert(stamped.issued)
    }

    pub(crate) fn at(&self, tick: Tick) -> &Batch {
        self.ticks.get(&tick).unwrap_or(&NOTHING)
    }

    pub(crate) fn until(&self, until: Tick) -> Log {
        Log {
            ticks: self
                .ticks
                .range(..until)
                .map(|(tick, batch)| (*tick, batch.clone()))
                .collect(),
        }
    }

    pub(crate) fn stamped(&self) -> impl Iterator<Item = Stamped> + '_ {
        self.ticks.iter().flat_map(|(tick, batch)| {
            batch.iter().map(move |issued| Stamped {
                tick: *tick,
                issued,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{RockId, RowId, SeatId};
    use crate::state::{Command, Issued};

    fn stamped(tick: u64, seat: u8, seq: u32) -> Stamped {
        Stamped {
            tick: Tick(tick),
            issued: Issued {
                seat: SeatId(seat),
                seq,
                command: Command::Want {
                    rock: RockId(0),
                    row: RowId(0),
                    count: seq,
                },
            },
        }
    }

    #[test]
    fn a_tick_hands_back_its_own_commands_in_order_and_a_refusal_changes_nothing() {
        let mut log = Log::default();
        for stamped in [stamped(4, 1, 0), stamped(2, 0, 1), stamped(4, 0, 0)] {
            assert_eq!(log.insert(stamped), Ok(()));
        }

        assert_eq!(
            log.at(Tick(4))
                .iter()
                .map(|issued| (issued.seat.0, issued.seq))
                .collect::<Vec<_>>(),
            [(0, 0), (1, 0)]
        );
        assert!(log.at(Tick(3)).is_empty());
        assert_eq!(log.insert(stamped(3, 0, 0)), Ok(()));
        assert_eq!(log.insert(stamped(3, 0, 0)), Err(Refused::Duplicate));
        assert_eq!(log.at(Tick(3)).iter().count(), 1);
        assert_eq!(log.stamped().count(), 4);
    }
}
