use neumannarch_sim::{Retention, Tick};

pub const ACKNOWLEDGE_INTERVAL: u64 = 16;

pub const LEAD_THRESHOLD: u32 = 24;

pub const REPORT_INTERVAL: u64 = 60;

pub const SLOW_EVERY: u64 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allowed {
    Advance,
    Held,
    Slowed,
}

#[derive(Clone, Copy, Debug)]
pub struct Pace {
    window: u32,
    offered: u64,
}

impl Pace {
    pub fn shipped() -> Pace {
        Pace {
            window: Retention::shipped().span(),
            offered: 0,
        }
    }

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
