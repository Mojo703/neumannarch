//! The deterministic lockstep simulation of Probe Game. No engine, no
//! platform code, no floats but `f64`; the same initial state and command
//! log yield the same state on every target.

use core::time::Duration;

/// Sim steps per second. The engine's accumulator runs at this rate and the
/// harness measures in it.
pub const TICKS_PER_SECOND: u32 = 120;

/// The span one step advances the world by, in whole nanoseconds.
pub const TICK: Duration = Duration::from_secs(1)
    .checked_div(TICKS_PER_SECOND)
    .unwrap();
