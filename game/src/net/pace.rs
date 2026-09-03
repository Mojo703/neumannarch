//! How far ahead of its peers one machine's sim may run.

use probe_sim::{Retention, Tick};

/// How often a machine tells its peers how far a seat of its own is
/// acknowledged: every sixteenth tick, an eighth of a second.
///
/// A hypothesis: often enough that no peer's window runs out waiting for
/// one, seldom enough that a match is not a message every tick.
pub const ACKNOWLEDGE_INTERVAL: u64 = 16;

/// How far ahead of the settled tick a machine runs before it slows, in
/// ticks: a fifth of a second. A hypothesis.
pub const LEAD_THRESHOLD: u32 = 24;

/// How often a machine reports its hash at the settled tick: every
/// sixtieth tick, half a second.
///
/// A hypothesis: a desync is caught within half a second of the tick it
/// happened at, and a match of four machines spends eight messages a
/// second saying so.
pub const REPORT_INTERVAL: u64 = 60;

/// One step in this many the engine offers is dropped by a machine that is
/// running ahead, which is a quarter off its tick rate: 90 steps a second
/// against 120. A hypothesis: it closes [`LEAD_THRESHOLD`] of lead in four
/// times that many ticks.
pub const SLOW_EVERY: u64 = 4;

/// What the pace allows a machine this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allowed {
    /// Step the sim.
    Advance,
    /// The match is held: a peer is further behind than a command from it
    /// could still be taken.
    Held,
    /// Skip this step, so the machines behind level up. The player is
    /// shown nothing: the match is still running.
    Slowed,
}

/// The rule by which one machine's sim keeps pace with its peers'.
///
/// A machine advances no further than the retention window ahead of the
/// lowest acknowledged tick among the seats, which is a peer's whenever a
/// peer is behind, since a machine acknowledges its own seats as it
/// advances.
#[derive(Clone, Copy, Debug)]
pub struct Pace {
    /// How far a session can rewind, in ticks.
    window: u32,
    /// Steps offered so far, which is what a dropped step is counted
    /// against: counting the ticks reached instead would drop the same
    /// tick for ever, since a dropped step reaches none.
    offered: u64,
}

impl Pace {
    /// The pace of a session keeping the shipped window.
    pub fn shipped() -> Pace {
        Pace {
            window: Retention::shipped().span(),
            offered: 0,
        }
    }

    /// What this machine may do with the step it is being offered, showing
    /// `live` with the seats settled to `settled`.
    pub fn allows(&mut self, live: Tick, settled: Tick) -> Allowed {
        self.offered += 1;
        let lead = live.0.saturating_sub(settled.0);
        if lead > u64::from(self.window) {
            return Allowed::Held;
        }
        match lead > u64::from(LEAD_THRESHOLD) && self.offered.is_multiple_of(SLOW_EVERY) {
            true => Allowed::Slowed,
            false => Allowed::Advance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lead at which the pace first holds the match.
    fn window() -> u64 {
        u64::from(Retention::shipped().span())
    }

    #[test]
    fn a_machine_runs_at_full_rate_until_it_leads_then_drops_a_step_in_four_then_holds() {
        let mut pace = Pace::shipped();
        let settled = Tick(1_000);
        let mut at = |lead: u64| pace.allows(Tick(settled.0 + lead), settled);

        assert_eq!(at(0), Allowed::Advance, "no lead is no reason to slow");
        assert_eq!(at(u64::from(LEAD_THRESHOLD)), Allowed::Advance);

        let lead = u64::from(LEAD_THRESHOLD) + 1;
        let leading: Vec<Allowed> = (0..4 * SLOW_EVERY).map(|_| at(lead)).collect();
        assert_eq!(
            leading
                .iter()
                .filter(|one| **one == Allowed::Slowed)
                .count() as u64,
            4,
            "one step in {SLOW_EVERY} is dropped past the threshold, not {leading:?}"
        );

        assert_ne!(at(window()), Allowed::Held, "the window itself is in reach");
        assert_eq!(at(window() + 1), Allowed::Held);
    }
}
