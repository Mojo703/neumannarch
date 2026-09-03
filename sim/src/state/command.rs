//! The one verb, and its application to the state.

use super::State;
use crate::ids::{RowId, SeatId};
use crate::place::{Band, Place, Post};
use crate::roster::Kind;

/// The most of one row a post can want. The one cap the sim states.
pub const MAX_WANT: u32 = 200;

/// What a seat can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    /// Set the count of `row` wanted at `place`.
    Want {
        place: Place,
        row: RowId,
        count: u32,
    },
}

/// A command with the seat that issued it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Issued {
    pub seat: SeatId,
    pub command: Command,
}

/// Why a command changed nothing. Checked in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rejected {
    /// The seat is out of the match, or is not one of the match's seats.
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

impl State {
    /// Applies `issued` or rejects it by name, changing nothing.
    pub fn apply(&mut self, issued: Issued) -> Result<(), Rejected> {
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
        if !self.seat(post.seat).is_some_and(|seat| seat.alive()) {
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
