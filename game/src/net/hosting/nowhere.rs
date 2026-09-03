//! What a build without the match server holds instead of a room, which
//! is nothing.

use crate::net::hosting::NoRoom;

/// A room served on this machine, which this build cannot hold one of: no
/// value of `Nowhere` exists, so no such build has a room.
pub struct Hosting(Nowhere);

enum Nowhere {}

impl Hosting {
    /// The address a machine joins this room at, as `host:port`.
    pub fn address(&self) -> String {
        match self.0 {}
    }

    /// Serves a room on this machine, which this build cannot do.
    pub fn opened() -> Result<Hosting, NoRoom> {
        Err(NoRoom::NotBuilt)
    }
}
