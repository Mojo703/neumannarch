//! The room this machine is in: the socket to it, and the id it holds
//! there.

use probe_protocol::{Message, PlayerId};

use crate::net::socket::Socket;
use crate::net::transport::Transport;

/// One room this machine has asked to join. Its id arrives with the
/// welcome, and until then it holds none.
pub struct Room {
    socket: Socket,
    me: Option<PlayerId>,
}

impl Room {
    /// The room this socket was opened to, as `host:port`.
    pub fn address(&self) -> &str {
        self.socket.address()
    }

    /// Whether the socket has closed, which a room that was never reached
    /// also reads as.
    pub fn closed(&self) -> bool {
        self.socket.closed()
    }

    /// Every message the room has sent since the last call, in arrival
    /// order. The welcome's id is taken as it passes.
    pub fn heard(&mut self) -> Vec<Message> {
        let heard = self.socket.heard();
        for message in &heard {
            if let Message::Welcome { player, .. } = message {
                self.me = Some(*player);
            }
        }
        heard
    }

    /// The socket, for a match to speak to its peers through. The room
    /// keeps it, so a room outlives the match played in it.
    pub fn transport(&mut self) -> &mut dyn Transport {
        &mut self.socket
    }

    /// Opens a socket to the room at `address`, as `host:port`, and asks to
    /// join it.
    pub fn joining(address: &str) -> Room {
        Room {
            socket: Socket::joining(address),
            me: None,
        }
    }

    /// Tells the room this machine is leaving.
    pub fn leaving(&mut self) {
        self.socket.say(Message::Leave);
    }

    /// The id this machine holds in the room; `None` until the welcome.
    pub fn me(&self) -> Option<PlayerId> {
        self.me
    }

    /// Asks the room for `message`.
    pub fn say(&mut self, message: Message) {
        self.socket.say(message);
    }
}
