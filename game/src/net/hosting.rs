//! The room this machine serves, where the build can serve one.

/// A room served on this machine. Dropping it stops the room.
#[cfg(feature = "host")]
pub struct Hosting(probe_server::Hosted);

/// A room served on this machine, which a build without the server cannot
/// hold one of.
#[cfg(not(feature = "host"))]
pub struct Hosting(Nowhere);

/// Why this machine is not serving a room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoRoom {
    /// This build does not carry the server. The browser cannot serve a
    /// room: it opens sockets and does not answer them.
    NotBuilt,
    /// The address could not be served, as the platform said.
    Unbound(String),
}

/// What a build with no server holds instead of a room, which is nothing:
/// no value of this type exists, so no such build has a room.
#[cfg(not(feature = "host"))]
enum Nowhere {}

impl Hosting {
    /// The address a machine joins this room at, as `host:port`.
    #[cfg(feature = "host")]
    pub fn address(&self) -> String {
        format!("127.0.0.1:{}", self.0.address().port())
    }

    /// The address a machine joins this room at, as `host:port`.
    #[cfg(not(feature = "host"))]
    pub fn address(&self) -> String {
        match self.0 {}
    }

    /// Serves a room on this machine, on every interface at the protocol's
    /// own port.
    #[cfg(feature = "host")]
    pub fn opened() -> Result<Hosting, NoRoom> {
        Hosting::serving(std::net::SocketAddr::from((
            [0, 0, 0, 0],
            probe_protocol::DEFAULT_PORT,
        )))
    }

    /// Serves a room on this machine, which this build cannot do.
    #[cfg(not(feature = "host"))]
    pub fn opened() -> Result<Hosting, NoRoom> {
        Err(NoRoom::NotBuilt)
    }

    /// Serves a room at `address`, whatever port it names.
    #[cfg(feature = "host")]
    pub fn serving(address: std::net::SocketAddr) -> Result<Hosting, NoRoom> {
        probe_server::Hosted::serving(address)
            .map(Hosting)
            .map_err(|why| NoRoom::Unbound(why.to_string()))
    }
}

impl core::fmt::Display for NoRoom {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NoRoom::NotBuilt => out.write_str("this build serves no room"),
            NoRoom::Unbound(why) => write!(out, "no room served: {why}"),
        }
    }
}
