//! The match over time: the snapshot ring, the stamped log, and the
//! session every frontend drives over them.

pub use session::{Rewound, Session, Unseated};
pub use snapshots::{Retention, WINDOW_SECONDS};

mod log;
mod session;
mod snapshots;
