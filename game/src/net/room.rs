//! The room this machine is in: the socket to it, and what it says back to
//! the screen watching it.

use probe_protocol::{Lobby, Message, PlayerId, Refusal, Started};

use crate::net::socket::Socket;
use crate::net::transport::Transport;

/// One thing a room says to the screen watching it.
///
/// The match's own words are not among these: a `Machine` reads those
/// through the transport, on the same socket.
#[derive(Clone, Debug, PartialEq)]
pub enum Word {
    /// The id the room gave this machine, and the lobby as it stands.
    Welcome { player: PlayerId, lobby: Lobby },
    /// The lobby after an edit landed, which is the truth about what took.
    Lobby(Lobby),
    /// The match every machine in the room now builds.
    Started(Started),
    /// Why the room did nothing this machine asked for.
    Refused(Refusal),
    /// The host has left, which ends the room.
    Left,
    /// The room has opened the slot this machine held.
    Removed,
}

/// One room this machine has asked to join.
pub struct Room {
    socket: Socket,
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

    /// Everything the room has said to this screen since the last call, in
    /// arrival order.
    pub fn heard(&mut self) -> Vec<Word> {
        self.socket
            .heard()
            .into_iter()
            .filter_map(word_of)
            .collect()
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
        }
    }

    /// Tells the room this machine is leaving.
    pub fn leaving(&mut self) {
        self.socket.say(Message::Leave);
    }

    /// Asks the room for `message`.
    pub fn say(&mut self, message: Message) {
        self.socket.say(message);
    }
}

/// What `message` says to a screen; `None` for a message no screen is the
/// reader of, which is dropped where bytes that are not a message are.
fn word_of(message: Message) -> Option<Word> {
    match message {
        Message::Welcome { player, lobby } => Some(Word::Welcome { player, lobby }),
        Message::Lobby(lobby) => Some(Word::Lobby(lobby)),
        Message::Start(started) => Some(Word::Started(started)),
        Message::Refused(why) => Some(Word::Refused(why)),
        Message::Leave => Some(Word::Left),
        Message::Removed => Some(Word::Removed),
        // A room's own inbound words, and the match's, which a `Machine`
        // reads through the transport.
        Message::Join { .. }
        | Message::Edit(_)
        | Message::Rematch
        | Message::Command(_)
        | Message::Acknowledge { .. }
        | Message::Hash { .. }
        | Message::Desync { .. } => None,
    }
}
