use super::schedule::Flight;
use crate::ids::{AsteroidId, EntityId, RowId, SeatId};
use crate::orbit::body::Body;
use crate::real::Real;
use crate::time::Time;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    id: EntityId,
    seat: SeatId,
    row: RowId,
    home: AsteroidId,
    hp: Real,
    motion: Motion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    Fixed,
    Steered { body: Body, flight: Option<Flight> },
}

impl Entity {
    pub fn new(
        id: EntityId,
        seat: SeatId,
        row: RowId,
        home: AsteroidId,
        hp: f64,
        motion: Motion,
    ) -> Entity {
        Entity {
            id,
            seat,
            row,
            home,
            hp: Real(hp),
            motion,
        }
    }

    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn seat(&self) -> SeatId {
        self.seat
    }

    pub fn row(&self) -> RowId {
        self.row
    }

    pub fn home(&self) -> AsteroidId {
        self.home
    }

    pub fn hp(&self) -> f64 {
        self.hp.0
    }

    pub fn motion(&self) -> Motion {
        self.motion
    }

    pub(crate) fn hurt(&mut self, damage: f64) {
        self.hp.0 -= damage;
    }

    pub(crate) fn heal(&mut self, hp: f64, full: f64) {
        self.hp.0 = (self.hp.0 + hp).min(full);
    }

    pub(crate) fn set_home(&mut self, home: AsteroidId) {
        self.home = home;
    }

    pub(crate) fn set_motion(&mut self, motion: Motion) {
        self.motion = motion;
    }

    pub fn flight(&self) -> Option<Flight> {
        match self.motion {
            Motion::Fixed | Motion::Steered { flight: None, .. } => None,
            Motion::Steered {
                flight: Some(flight),
                ..
            } => Some(flight),
        }
    }

    pub fn is_flying(&self, now: Time) -> bool {
        self.flight().is_some_and(|flight| flight.has_departed(now))
    }

    pub fn standing(&self, now: Time) -> Option<AsteroidId> {
        match self.flight() {
            None => Some(self.home),
            Some(flight) if flight.has_departed(now) => None,
            Some(flight) => Some(flight.source()),
        }
    }
}
