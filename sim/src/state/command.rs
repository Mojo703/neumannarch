use serde::{Deserialize, Serialize};

use super::State;
use crate::ids::{RockId, RowId, SeatId};
use crate::post::Post;
use crate::time::Tick;

pub const MAX_WANT: u32 = 200;

pub const MAX_COMMANDS_PER_TICK: usize = 32;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Command {
    Want {
        rock: RockId,
        row: RowId,
        count: u32,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Issued {
    pub seat: SeatId,
    pub seq: u32,
    pub command: Command,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Stamped {
    pub tick: Tick,
    pub issued: Issued,
}

#[derive(Clone, Debug)]
pub struct Sequence {
    seat: SeatId,
    next: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Batch(Vec<Issued>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Refused {
    Duplicate,
    TooMany,
    Late,
    Ahead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rejected {
    NoSuchSeat,
    DeadSeat,
    NoSuchRock,
    NoSuchRow,
    TooMany,
    NotYet,
    RockTaken,
}

impl Sequence {
    pub fn new(seat: SeatId) -> Sequence {
        Sequence { seat, next: 0 }
    }

    pub fn seat(&self) -> SeatId {
        self.seat
    }

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
    pub const fn new() -> Batch {
        Batch(Vec::new())
    }

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

    pub fn iter(&self) -> impl Iterator<Item = Issued> + '_ {
        self.0.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn of_seat(&self, seat: SeatId) -> impl Iterator<Item = &Issued> {
        self.0.iter().filter(move |held| held.seat == seat)
    }
}

impl State {
    pub(crate) fn apply(&mut self, issued: Issued) -> Result<(), Rejected> {
        let Command::Want { rock, row, count } = issued.command;
        let post = Post {
            rock,
            seat: issued.seat,
        };
        self.validate(post, row, count)?;
        if count == 1 {
            self.pick(post, row)?;
        }
        self.set_want(post, row, count);
        Ok(())
    }

    fn pick(&mut self, post: Post, row: RowId) -> Result<(), Rejected> {
        match self.draft.awaits(post.seat, row) {
            Some(true) if self.draft.took(post.rock).is_none() => {
                self.draft.place(post.rock, post.seat, row, self.tick);
                Ok(())
            }
            Some(true) => Err(Rejected::RockTaken),
            Some(false) => Err(Rejected::NotYet),
            None => Ok(()),
        }
    }

    fn validate(&self, post: Post, row: RowId, count: u32) -> Result<(), Rejected> {
        let Some(seat) = self.seat(post.seat) else {
            return Err(Rejected::NoSuchSeat);
        };
        if !seat.alive() {
            return Err(Rejected::DeadSeat);
        }
        if self.rock(post.rock).is_none() {
            return Err(Rejected::NoSuchRock);
        }
        if self.roster().get(row).is_none() {
            return Err(Rejected::NoSuchRow);
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
