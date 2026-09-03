//! The stamped commands of a match, by the tick they take effect at.

use std::collections::BTreeMap;

use crate::state::{Batch, Refused, Stamped};
use crate::time::Tick;

static NOTHING: Batch = Batch::new();

/// Every command a session knows, each in its tick's batch. A tick with no
/// command holds nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Log {
    ticks: BTreeMap<Tick, Batch>,
}

impl Log {
    /// Takes `stamped` into its tick's batch, or refuses it by name.
    /// Whether the tick is one to hold at all is the session's judgement.
    pub(crate) fn insert(&mut self, stamped: Stamped) -> Result<(), Refused> {
        // A new tick's batch is empty and takes its first command, so a
        // refusal never leaves an empty batch behind.
        self.ticks
            .entry(stamped.tick)
            .or_default()
            .insert(stamped.issued)
    }

    /// The commands stamped at `tick`, in `(seat, seq)` order.
    pub(crate) fn at(&self, tick: Tick) -> &Batch {
        self.ticks.get(&tick).unwrap_or(&NOTHING)
    }

    /// The commands stamped before `until`.
    pub(crate) fn until(&self, until: Tick) -> Log {
        Log {
            ticks: self
                .ticks
                .range(..until)
                .map(|(tick, batch)| (*tick, batch.clone()))
                .collect(),
        }
    }

    /// Every command it holds, in tick then `(seat, seq)` order.
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
    use crate::place::{Band, Place};
    use crate::state::{Command, Issued};

    fn stamped(tick: u64, seat: u8, seq: u32) -> Stamped {
        Stamped {
            tick: Tick(tick),
            issued: Issued {
                seat: SeatId(seat),
                seq,
                command: Command::Want {
                    place: Place {
                        rock: RockId(0),
                        band: Band::Inner,
                    },
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
