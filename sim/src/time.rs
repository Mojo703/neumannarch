//! The sim clock and the fractional moments within it.

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::TICKS_PER_SECOND;

/// Steps since the match began.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize,
)]
pub struct Tick(pub u64);

/// A fractional tick: when a weapon is next ready, and the order shots
/// resolve in. Ordered totally, equal and hashed by bit pattern.
#[derive(Clone, Copy, Debug, Default)]
pub struct Moment(pub f64);

impl Tick {
    pub const ZERO: Tick = Tick(0);

    pub fn next(self) -> Tick {
        Tick(self.0 + 1)
    }

    /// The tick before this one; `None` at the start of the match.
    pub fn previous(self) -> Option<Tick> {
        self.0.checked_sub(1).map(Tick)
    }

    /// The tick `ticks` earlier, or the start of the match when it is
    /// nearer than that.
    pub fn back(self, ticks: u32) -> Tick {
        Tick(self.0.saturating_sub(u64::from(ticks)))
    }

    /// The tick `ticks` later.
    pub fn ahead(self, ticks: u32) -> Tick {
        Tick(self.0 + u64::from(ticks))
    }

    /// Seconds since the match began.
    pub fn seconds(self) -> f64 {
        self.0 as f64 / f64::from(TICKS_PER_SECOND)
    }
}

impl Moment {
    /// The start of `tick`.
    pub fn at(tick: Tick) -> Moment {
        Moment(tick.0 as f64)
    }

    /// The tick this moment falls in.
    pub fn tick(self) -> Tick {
        Tick(self.0.max(0.0) as u64)
    }

    /// This moment plus `seconds`.
    pub fn after(self, seconds: f64) -> Moment {
        Moment(self.0 + seconds * f64::from(TICKS_PER_SECOND))
    }
}

impl Eq for Moment {}

impl Hash for Moment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl Ord for Moment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialEq for Moment {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl PartialOrd for Moment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_falls_in_the_tick_it_started_from_until_the_next() {
        let start = Moment::at(Tick(7));
        assert_eq!(start.tick(), Tick(7));
        assert_eq!(
            start.after(0.5 / f64::from(TICKS_PER_SECOND)).tick(),
            Tick(7)
        );
        assert_eq!(
            start.after(1.0 / f64::from(TICKS_PER_SECOND)).tick(),
            Tick(8)
        );
        assert!(start < start.after(1.0));
    }
}
