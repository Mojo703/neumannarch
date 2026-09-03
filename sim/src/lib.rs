//! The deterministic lockstep simulation of Probe Game. No engine, no
//! platform code, no floats but `f64`; the same initial state and command
//! log yield the same state on every target.

use core::time::Duration;

pub use ids::{EntityId, FlightId, RockId, RowId, SeatId, TeamId};
pub use materials::{Materials, Stockpile};
pub use place::{Band, Place, Post};
pub use real::Real;
pub use time::{Moment, Tick};
pub use vec3::Vec3;

/// Sim steps per second. The engine's accumulator runs at this rate and the
/// harness measures in it.
pub const TICKS_PER_SECOND: u32 = 120;

/// The span one step advances the world by, in whole nanoseconds.
pub const TICK: Duration = Duration::from_secs(1)
    .checked_div(TICKS_PER_SECOND)
    .unwrap();

pub mod belt;
mod ids;
mod materials;
pub mod orbit;
mod place;
mod real;
pub mod roster;
pub mod session;
pub mod state;
pub mod step;
mod time;
mod vec3;
