use serde::{Deserialize, Serialize};

use super::State;
use crate::ids::{AsteroidId, RowId, SeatId};
use crate::post::Post;
use crate::posting::Posting;
use crate::time::Tick;

pub const MAX_WANT: u32 = 200;

pub const MAX_COMMANDS_PER_TICK: usize = 32;

const PICK: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Command {
    Want {
        asteroid: AsteroidId,
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
    NoSuchAsteroid,
    NoSuchRow,
    TooMany,
    NotYet,
    AsteroidTaken,
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

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn of_seat(&self, seat: SeatId) -> impl Iterator<Item = &Issued> {
        self.0.iter().filter(move |held| held.seat == seat)
    }
}

impl State {
    pub fn apply(&mut self, issued: Issued) -> Result<(), Rejected> {
        let Command::Want {
            asteroid,
            row,
            count,
        } = issued.command;
        let posting = Posting::of(asteroid, issued.seat, row);
        self.admits_want(posting, count)?;
        if count == PICK {
            self.pick(posting);
        }
        self.set_want(posting.post(), row, count);
        Ok(())
    }

    pub fn admits_want(&self, posting: Posting, count: u32) -> Result<(), Rejected> {
        let Some(seated) = self.seat(posting.seat()) else {
            return Err(Rejected::NoSuchSeat);
        };
        if !seated.alive() {
            return Err(Rejected::DeadSeat);
        }
        if self.asteroid(posting.asteroid()).is_none() {
            return Err(Rejected::NoSuchAsteroid);
        }
        if self.roster().get(posting.row()).is_none() {
            return Err(Rejected::NoSuchRow);
        }
        if count > MAX_WANT {
            return Err(Rejected::TooMany);
        }
        if count != PICK {
            return Ok(());
        }
        match self.draft.awaits(posting.seat(), posting.row()) {
            Some(false) => Err(Rejected::NotYet),
            Some(true) if self.draft.took(posting.asteroid()).is_some() => {
                Err(Rejected::AsteroidTaken)
            }
            Some(_) | None => Ok(()),
        }
    }

    fn pick(&mut self, posting: Posting) {
        if self.draft.awaits(posting.seat(), posting.row()) == Some(true) {
            self.draft
                .place(posting.asteroid(), posting.seat(), posting.row(), self.tick);
        }
    }

    fn set_want(&mut self, post: Post, row: RowId, count: u32) {
        let wants = self.wants.entry(post).or_default();
        wants.set(row, count);
        if wants.is_empty() {
            self.wants.remove(&post);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::roster::FRIGATE;
    use crate::time::Time;

    const HERE: AsteroidId = AsteroidId(0);

    #[test]
    fn what_the_state_admits_is_what_applying_the_want_takes() {
        let mut world = World::drafting(&[TeamId(0), TeamId(1)], Time(600));
        let stages = world.state.draft().stages().to_vec();
        let (first, later) = (stages[0], stages[1]);
        let asked = |state: &State, posting: Posting, count: u32| {
            let admitted = state.admits_want(posting, count);
            let mut applying = state.clone();
            let applied = applying.apply(Issued {
                seat: posting.seat(),
                seq: 0,
                command: Command::Want {
                    asteroid: posting.asteroid(),
                    row: posting.row(),
                    count,
                },
            });
            assert_eq!(admitted, applied, "the query and the command disagree");
            admitted
        };

        assert_eq!(
            asked(&world.state, Posting::of(HERE, first.seat, first.row), 1),
            Ok(())
        );
        assert_eq!(
            asked(&world.state, Posting::of(HERE, later.seat, later.row), 1),
            Err(Rejected::NotYet),
            "a seat whose stage has not begun cannot place"
        );
        assert_eq!(
            asked(&world.state, Posting::of(HERE, first.seat, FRIGATE), 1),
            Ok(()),
            "a frigate is no pick"
        );
        assert_eq!(
            asked(
                &world.state,
                Posting::of(HERE, first.seat, FRIGATE),
                MAX_WANT + 1
            ),
            Err(Rejected::TooMany)
        );
        assert_eq!(
            asked(&world.state, Posting::of(HERE, SeatId(9), FRIGATE), 1),
            Err(Rejected::NoSuchSeat)
        );

        world.tick(&[Issued::numbered(first.seat.0, 0, HERE, first.row, 1)]);
        let next = world.state.draft().running().expect("the next stage runs");

        assert_eq!(
            asked(&world.state, Posting::of(HERE, next.seat, next.row), 1),
            Err(Rejected::AsteroidTaken)
        );
        assert_eq!(
            asked(
                &world.state,
                Posting::of(AsteroidId(1), next.seat, next.row),
                1
            ),
            Ok(())
        );
    }
}
