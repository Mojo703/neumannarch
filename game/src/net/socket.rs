//! This machine's own socket to a room, speaking the protocol over it.

use probe_protocol::{Message, Wire};
use probe_sim::{SeatId, Stamped, Tick};

use crate::net::link::Link;
use crate::net::transport::Transport;

/// The transport of a match with peers: one WebSocket to the room, the
/// protocol's messages both ways.
///
/// Nothing waits on it. A message said before the socket opens is sent
/// when it does, and one that arrives is read at the next call.
pub struct Socket {
    link: Link,
    /// The room this socket was opened to, as `host:port`.
    address: String,
}

impl Socket {
    /// The room this socket was opened to, as `host:port`.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Whether the socket has closed, which a room that was never reached
    /// also reads as.
    pub fn closed(&self) -> bool {
        self.link.closed()
    }

    /// Every message the room has sent since the last call, in arrival
    /// order. Bytes that are not a message are not read.
    pub fn heard(&mut self) -> Vec<Message> {
        self.link
            .heard()
            .iter()
            .filter_map(|bytes| Message::decode(bytes).ok())
            .collect()
    }

    /// Opens a socket to the room at `address`, as `host:port`, and asks to
    /// join it. The welcome arrives as a [`Message::Welcome`].
    pub fn joining(address: &str) -> Socket {
        let mut socket = Socket {
            link: Link::opening(address),
            address: address.to_string(),
        };
        socket.say(Message::Join);
        socket
    }

    /// Passes `message` to the room.
    pub fn say(&mut self, message: Message) {
        self.link.say(message.encoded());
    }
}

impl Transport for Socket {
    fn acknowledge(&mut self, seat: SeatId, up_to: Tick) {
        self.say(Message::Acknowledge { seat, up_to });
    }

    fn leave(&mut self) {
        self.say(Message::Leave);
    }

    fn received(&mut self) -> Vec<Message> {
        self.heard()
    }

    fn report(&mut self, tick: Tick, hash: u64) {
        self.say(Message::Hash { tick, hash });
    }

    fn send(&mut self, stamped: Stamped) {
        self.say(Message::Command(stamped));
    }
}
