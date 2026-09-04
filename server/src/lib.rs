//! The match server of Probe Game: one room, the authority on its lobby,
//! forwarding the match its members play. It holds no tick clock and steps
//! no sim.
//!
//! The game embeds it to host a room in its own process; the binary serves
//! the same room to machines that connect over the network.

pub use records::Records;
pub use rooms::{Joined, Post, Room, To};
pub use stream::Hosted;

mod playing;
mod records;
mod rooms;
mod stream;
