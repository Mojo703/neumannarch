//! The room this machine serves, where the build can serve one: one
//! module for a build that carries the match server, one for a build that
//! cannot.

#[cfg(not(all(feature = "host", not(target_arch = "wasm32"))))]
pub use nowhere::Hosting;
#[cfg(all(feature = "host", not(target_arch = "wasm32")))]
pub use served::Hosting;

/// Why this machine is not serving a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoRoom {
    /// This build does not carry the server. The browser cannot serve a
    /// room: it opens sockets and does not answer them.
    NotBuilt,
    /// Something else holds the port a room is served on.
    Held,
}

impl NoRoom {
    /// The one sentence a disabled Host shows.
    pub fn reason(self) -> &'static str {
        match self {
            NoRoom::NotBuilt => "This version cannot host",
            NoRoom::Held => "Another program holds the room's port",
        }
    }
}

#[cfg(not(all(feature = "host", not(target_arch = "wasm32"))))]
mod nowhere;
#[cfg(all(feature = "host", not(target_arch = "wasm32")))]
mod served;
