use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::TICKS_PER_SECOND;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize,
)]
pub struct Tick(pub u64);

#[derive(
    Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize,
)]
pub struct Time(pub u64);

#[derive(Clone, Copy, Debug, Default)]
pub struct Moment(pub f64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RunningSpan(Time);

impl Tick {
    pub const ZERO: Tick = Tick(0);

    pub fn next(self) -> Tick {
        Tick(self.0 + 1)
    }

    pub fn previous(self) -> Option<Tick> {
        self.0.checked_sub(1).map(Tick)
    }

    pub fn back(self, ticks: u32) -> Tick {
        Tick(self.0.saturating_sub(u64::from(ticks)))
    }

    pub fn ahead(self, ticks: u32) -> Tick {
        Tick(self.0 + u64::from(ticks))
    }

    pub fn since(self, then: Tick) -> Time {
        Time(self.0.saturating_sub(then.0))
    }

    pub fn after(self, ran: Time) -> Tick {
        Tick(self.0 + ran.0)
    }

    pub fn seconds(self) -> f64 {
        self.0 as f64 / f64::from(TICKS_PER_SECOND)
    }
}

impl Time {
    pub const ZERO: Time = Time(0);

    pub fn next(self) -> Time {
        Time(self.0 + 1)
    }

    pub fn since(self, then: Time) -> Time {
        Time(self.0.saturating_sub(then.0))
    }

    pub fn after(self, ran: Time) -> Time {
        Time(self.0 + ran.0)
    }

    pub fn seconds(self) -> f64 {
        self.0 as f64 / f64::from(TICKS_PER_SECOND)
    }
}

impl RunningSpan {
    pub(crate) fn of(ran: Time) -> Option<RunningSpan> {
        (ran > Time::ZERO).then_some(RunningSpan(ran))
    }

    pub(crate) fn seconds(self) -> f64 {
        self.0.seconds()
    }

    pub(crate) fn ends_at(self, started: Time) -> Time {
        started.after(self.0)
    }
}

impl Moment {
    pub fn at(time: Time) -> Moment {
        Moment(time.0 as f64)
    }

    pub fn time(self) -> Time {
        Time(self.0.max(0.0) as u64)
    }

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
        let start = Moment::at(Time(7));
        assert_eq!(start.time(), Time(7));
        assert_eq!(
            start.after(0.5 / f64::from(TICKS_PER_SECOND)).time(),
            Time(7)
        );
        assert_eq!(
            start.after(1.0 / f64::from(TICKS_PER_SECOND)).time(),
            Time(8)
        );
        assert!(start < start.after(1.0));
    }

    #[test]
    fn the_tick_a_match_time_runs_to_is_the_tick_it_ran_from_plus_that_time() {
        let started = Tick(8_400);

        assert_eq!(started.after(Time(7_200)), Tick(15_600));
        assert_eq!(Tick(15_600).since(started), Time(7_200));
        assert_eq!(
            started.since(Tick(15_600)),
            Time::ZERO,
            "a tick before the one the clock started at has run no time"
        );
    }

    #[test]
    fn a_step_the_clock_did_not_run_over_has_no_running_span() {
        assert_eq!(RunningSpan::of(Time::ZERO), None, "the draft runs no time");

        let ran = RunningSpan::of(Time(1)).expect("the clock ran a tick");

        assert_eq!(ran.ends_at(Time(7)), Time(8));
        assert_eq!(ran.seconds(), 1.0 / f64::from(TICKS_PER_SECOND));
    }

    #[test]
    fn match_time_starts_where_the_clock_started_and_never_runs_backward() {
        assert_eq!(Time(500).since(Time(120)), Time(380));
        assert_eq!(Time(20).since(Time(120)), Time::ZERO, "never before zero");
        assert_eq!(Time(240).seconds(), 2.0);
    }
}
