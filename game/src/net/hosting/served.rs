//! The room a build carrying the match server serves.

use std::net::SocketAddr;

use crate::net::hosting::NoRoom;

/// A room served on this machine. Dropping it stops the room and closes
/// every socket to it.
pub struct Hosting(probe_server::Hosted);

impl Hosting {
    /// The address a machine joins this room at, as `host:port`.
    pub fn address(&self) -> String {
        format!("127.0.0.1:{}", self.0.address().port())
    }

    /// Serves a room on this machine, on every interface at the protocol's
    /// own port.
    ///
    /// The title holds what this answers, so Host is offered only where a
    /// room is already served and the click cannot fail.
    pub fn opened() -> Result<Hosting, NoRoom> {
        Hosting::serving(SocketAddr::from((
            [0, 0, 0, 0],
            probe_protocol::DEFAULT_PORT,
        )))
    }

    /// Serves a room at `address`, whatever port it names.
    pub fn serving(address: SocketAddr) -> Result<Hosting, NoRoom> {
        probe_server::Hosted::serving(address)
            .map(Hosting)
            .map_err(|_| NoRoom::Held)
    }
}
