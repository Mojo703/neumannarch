use core::num::NonZeroU32;
use std::collections::BTreeMap;

use crate::TICKS_PER_SECOND;
use crate::state::State;
use crate::time::Tick;

pub const WINDOW_SECONDS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    Window { ticks: u32, every: NonZeroU32 },
}

pub(crate) struct Snapshots {
    retention: Retention,
    kept: BTreeMap<Tick, State>,
}

impl Retention {
    pub fn shipped() -> Retention {
        Retention::Window {
            ticks: WINDOW_SECONDS * TICKS_PER_SECOND,
            every: NonZeroU32::MIN,
        }
    }

    pub fn span(&self) -> u32 {
        match self {
            Retention::Window { ticks, .. } => *ticks,
        }
    }

    fn keeps(&self, tick: Tick) -> bool {
        match self {
            Retention::Window { every, .. } => tick.0.is_multiple_of(u64::from(every.get())),
        }
    }
}

impl Snapshots {
    pub fn new(retention: Retention) -> Snapshots {
        Snapshots {
            retention,
            kept: BTreeMap::new(),
        }
    }

    pub(crate) fn span(&self) -> u32 {
        self.retention.span()
    }

    pub(crate) fn keep(&mut self, state: &State) {
        if self.retention.keeps(state.tick()) {
            self.kept.insert(state.tick(), state.clone());
        }
    }

    pub fn at_or_before(&self, tick: Tick) -> Option<&State> {
        self.kept.range(..=tick).next_back().map(|(_, kept)| kept)
    }

    pub fn discard_after(&mut self, tick: Tick) {
        self.kept.retain(|kept, _| *kept <= tick);
    }

    pub fn prune(&mut self, oldest: Tick) {
        let floor = self
            .kept
            .range(..=oldest)
            .next_back()
            .map(|(tick, _)| *tick);
        if let Some(floor) = floor {
            self.kept.retain(|kept, _| *kept >= floor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::TeamId;
    use crate::setup::Setup;

    fn state(tick: Tick) -> State {
        let setup = Setup::new(vec![TeamId(0)], 0, Tick(10_000)).expect("one seat is a match");
        let mut state = State::start(&setup);
        while state.tick() < tick {
            state.advance();
        }
        state
    }

    fn window(ticks: u32, every: u32) -> Retention {
        Retention::Window {
            ticks,
            every: NonZeroU32::new(every).expect("a stride of no ticks keeps nothing"),
        }
    }

    fn ring(retention: Retention, ticks: u64) -> Snapshots {
        let mut snapshots = Snapshots::new(retention);
        for tick in 0..ticks {
            snapshots.keep(&state(Tick(tick)));
            snapshots.prune(Tick(tick).back(snapshots.span()));
        }
        snapshots
    }

    #[test]
    fn the_ring_holds_a_state_at_or_before_every_tick_in_the_window_and_none_older() {
        for every in [1, 5] {
            let span = 12;
            let snapshots = ring(window(span, every), 40);
            let oldest = Tick(39).back(span);

            for tick in oldest.0..40 {
                let from = snapshots
                    .at_or_before(Tick(tick))
                    .expect("a state to re-step from");
                assert!(
                    tick - from.tick().0 < u64::from(every),
                    "re-stepping {} ticks at a stride of {every}",
                    tick - from.tick().0
                );
            }
            assert!(
                snapshots
                    .kept
                    .keys()
                    .all(|kept| *kept >= oldest.back(every))
            );
            assert!(snapshots.kept.len() <= (span / every) as usize + 2);
        }
    }
}
