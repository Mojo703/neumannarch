//! The one verb: how it is stamped, how a tick's commands are ordered,
//! and how one applies to the state.

use serde::{Deserialize, Serialize};

use super::State;
use crate::ids::{RowId, SeatId};
use crate::place::{Band, Place, Post};
use crate::roster::Kind;
use crate::time::Tick;

/// The most of one row a post can want.
pub const MAX_WANT: u32 = 200;

/// The most commands one seat has at one tick, which bounds what a peer
/// can put in one tick of the log.
pub const MAX_COMMANDS_PER_TICK: usize = 32;

/// What a seat can ask for.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Command {
    /// Set the count of `row` wanted at `place`.
    Want {
        place: Place,
        row: RowId,
        count: u32,
    },
}

/// A command with the seat that issued it and that seat's own count of
/// its commands, which counts from zero for the match.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Issued {
    pub seat: SeatId,
    pub seq: u32,
    pub command: Command,
}

/// An issued command with the tick it takes effect at, wherever it is
/// applied.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Stamped {
    pub tick: Tick,
    pub issued: Issued,
}

/// One seat's count of its own commands, which stamps every command it
/// issues.
#[derive(Clone, Debug)]
pub struct Sequence {
    seat: SeatId,
    next: u32,
}

/// The commands of one tick, in `(seat, seq)` order with no key twice.
/// What [`State::step`] applies.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Batch(Vec<Issued>);

/// Why a stamped command was never logged, and so never applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Refused {
    /// The seat already has a command at this tick with this `seq`.
    Duplicate,
    /// The seat is already at [`MAX_COMMANDS_PER_TICK`] for this tick.
    TooMany,
    /// The tick is further back than the session can rewind.
    Late,
    /// The tick is further ahead than the session can be rewound to.
    Ahead,
}

/// Why a command changed nothing. Checked in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rejected {
    /// The match has no such seat.
    NoSuchSeat,
    /// The seat is out of the match.
    DeadSeat,
    /// The place names a rock the map lacks.
    NoSuchRock,
    /// The row is not in the roster.
    NoSuchRow,
    /// A structure wanted in an outer band.
    StructureOutside,
    /// A count above `MAX_WANT`.
    TooMany,
}

impl Sequence {
    /// The count `seat` starts a match with.
    pub fn new(seat: SeatId) -> Sequence {
        Sequence { seat, next: 0 }
    }

    /// The seat it counts for.
    pub fn seat(&self) -> SeatId {
        self.seat
    }

    /// `command` as this seat's next, taking effect at `tick`.
    pub fn stamp(&mut self, tick: Tick, command: Command) -> Stamped {
        let issued = Issued {
            seat: self.seat,
            seq: self.next,
            command,
        };
        self.next += 1;
        Stamped { tick, issued }
    }
}

impl Batch {
    /// Nothing issued.
    pub const fn new() -> Batch {
        Batch(Vec::new())
    }

    /// Takes `issued` into its place in `(seat, seq)` order, or refuses it
    /// by name.
    pub fn insert(&mut self, issued: Issued) -> Result<(), Refused> {
        let key = (issued.seat, issued.seq);
        let at = self.0.partition_point(|held| (held.seat, held.seq) < key);
        if self
            .0
            .get(at)
            .is_some_and(|held| (held.seat, held.seq) == key)
        {
            return Err(Refused::Duplicate);
        }
        if self.of_seat(issued.seat).count() >= MAX_COMMANDS_PER_TICK {
            return Err(Refused::TooMany);
        }
        self.0.insert(at, issued);
        Ok(())
    }

    /// The tick's commands, in `(seat, seq)` order.
    pub fn iter(&self) -> impl Iterator<Item = Issued> + '_ {
        self.0.iter().copied()
    }

    /// Whether nothing was issued this tick.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn of_seat(&self, seat: SeatId) -> impl Iterator<Item = &Issued> {
        self.0.iter().filter(move |held| held.seat == seat)
    }
}

impl State {
    /// Applies `issued` or rejects it by name, changing nothing.
    pub(crate) fn apply(&mut self, issued: Issued) -> Result<(), Rejected> {
        let Command::Want { place, row, count } = issued.command;
        let post = Post {
            place,
            seat: issued.seat,
        };
        self.validate(post, row, count)?;
        self.set_want(post, row, count);
        Ok(())
    }

    fn validate(&self, post: Post, row: RowId, count: u32) -> Result<(), Rejected> {
        let Some(seat) = self.seat(post.seat) else {
            return Err(Rejected::NoSuchSeat);
        };
        if !seat.alive() {
            return Err(Rejected::DeadSeat);
        }
        if self.rock(post.place.rock).is_none() {
            return Err(Rejected::NoSuchRock);
        }
        let Some(row) = self.roster().get(row) else {
            return Err(Rejected::NoSuchRow);
        };
        if row.kind() == Kind::Structure && post.place.band == Band::Outer {
            return Err(Rejected::StructureOutside);
        }
        if count > MAX_WANT {
            return Err(Rejected::TooMany);
        }
        Ok(())
    }

    fn set_want(&mut self, post: Post, row: RowId, count: u32) {
        let wants = self.wants.entry(post).or_default();
        wants.set(row, count);
        if wants.is_empty() {
            self.wants.remove(&post);
        }
    }
}
