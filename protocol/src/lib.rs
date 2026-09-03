//! Every value two machines of Probe Game exchange: the lobby, the
//! messages, and the record of a finished match. No io and no engine, so
//! the game, the server and the harness all speak it.

pub use ids::PlayerId;
pub use lobby::{
    Bot, CLOCK_RANGE, Control, DEFAULT_CLOCK, DEFAULT_SEED, Lobby, LobbyEdit, MAX_SLOTS, NotReady,
    Refused, SeatSlot,
};
pub use message::Message;
pub use record::{BadRecord, Record};
pub use wire::{Malformed, Wire};

mod ids;
mod lobby;
mod message;
mod record;
mod wire;
