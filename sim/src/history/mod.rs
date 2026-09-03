//! The match over time: the snapshot ring, the stamped log, and the
//! session every frontend drives over them.

pub use record::Record;
pub use session::{Rewound, Session};
pub use snapshots::{Retention, WINDOW_SECONDS};

mod log;
mod record;
mod session;
mod snapshots;
