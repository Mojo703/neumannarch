pub use ids::PlayerId;
pub use lobby::{
    Bot, CLOCK_RANGE, DEFAULT_CLOCK, DEFAULT_SEED, Holder, Lobby, LobbyEdit, MAX_SLOTS, NotReady,
    Refused, SeatSlot,
};
pub use message::{Message, Notice, Relayed, Request};
pub use record::{BadRecord, Record};
pub use seating::{Crew, Occupant, Seating, Started};
pub use wire::{Codec, Malformed};

pub const DEFAULT_PORT: u16 = 4747;

pub const VERSION: u32 = 1;

pub const WIRE_CAP: usize = 64 * 1024;

mod ids;
mod lobby;
mod message;
mod record;
mod seating;
mod wire;
