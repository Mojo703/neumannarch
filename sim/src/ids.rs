//! Typed ids. Each indexes one `Vec` of the state and fits nowhere else.
//!
//! The four a command or a setup names are serialisable, so `protocol`
//! speaks them; an entity and a flight are named by no wire value.

use serde::{Deserialize, Serialize};

/// A rock, by its index in the state.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RockId(pub u32);

/// A living entity, by its index in the state; never reused in a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub u32);

/// One send in progress, by its key in the state; never reused in a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlightId(pub u32);

/// A row of the roster.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RowId(pub u16);

/// A player.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct SeatId(pub u8);

/// A side; every seat has one, and seats on the same side are allies.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct TeamId(pub u8);
