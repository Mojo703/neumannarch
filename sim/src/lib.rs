use core::time::Duration;

pub use history::{Retention, Rewound, Session, Unseated, WINDOW_SECONDS};
pub use ids::{EntityId, RockId, RowId, SeatId, TeamId};
pub use materials::{Material, Materials, Stockpile};
pub use post::Post;
pub use real::Real;
pub use setup::{BadSetup, MAX_SEATS, Setup};
pub use state::{Batch, Refused, Sequence, Stamped};
pub use time::{Moment, Tick};
pub use vec3::Vec3;

pub const TICKS_PER_SECOND: u32 = 120;

pub const TICK: Duration = Duration::from_secs(1)
    .checked_div(TICKS_PER_SECOND)
    .unwrap();

pub mod belt;
pub mod history;
mod ids;
mod materials;
pub mod orbit;
mod post;
mod real;
pub mod roster;
mod setup;
pub mod state;
pub mod step;
mod time;
mod vec3;
