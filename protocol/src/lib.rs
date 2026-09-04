//! Every value two machines of Probe Game exchange: the lobby, the
//! messages, and the record of a finished match. No io and no engine, so
//! the game, the server and the harness all speak it.

pub use ids::PlayerId;
pub use lobby::{
    Bot, CLOCK_RANGE, Control, DEFAULT_CLOCK, DEFAULT_SEED, Lobby, LobbyEdit, MAX_SLOTS, NotReady,
    Refused, SeatSlot,
};
pub use message::{Message, Refusal};
pub use record::{BadRecord, Record};
pub use seating::{Crew, Holder, Seating, Started};
pub use wire::{Malformed, Wire};

/// The port a room is served on, and the one a join offers: no registered
/// service holds it.
pub const DEFAULT_PORT: u16 = 4747;

/// The version of this protocol. A room takes a join carrying this number
/// and refuses every other by name, so two builds that would read each
/// other's bytes differently never share a match. It rises with any change
/// to a value below.
pub const VERSION: u32 = 1;

/// The largest message the wire carries, in bytes. Both ends read under
/// this bound, so neither sends what the other will not read; the biggest
/// message a lobby or a match sends is a lobby or a setup, both far under
/// it.
pub const WIRE_CAP: usize = 64 * 1024;

mod ids;
mod lobby;
mod message;
mod record;
mod seating;
mod wire;
