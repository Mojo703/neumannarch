use super::schedule::Flight;
use crate::ids::{EntityId, RowId, SeatId};
use crate::orbit::body::Body;
use crate::place::Place;
use crate::real::Real;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    id: EntityId,
    seat: SeatId,
    row: RowId,
    home: Place,
    hp: Real,
    motion: Motion,
    scrap: Option<Real>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    Fixed,
    Free { body: Body, flight: Option<Flight> },
}

impl Entity {
    pub fn new(
        id: EntityId,
        seat: SeatId,
        row: RowId,
        home: Place,
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
            scrap: None,
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

    pub fn home(&self) -> Place {
        self.home
    }

    pub fn hp(&self) -> f64 {
        self.hp.0
    }

    pub fn motion(&self) -> Motion {
        self.motion
    }

    pub fn is_surplus(&self) -> bool {
        self.scrap.is_some()
    }

    pub fn scrapped(&self) -> Option<f64> {
        self.scrap.map(|work| work.0)
    }

    pub(crate) fn mark_surplus(&mut self) {
        self.scrap = self.scrap.or(Some(Real(0.0)));
    }

    pub(crate) fn clear_surplus(&mut self) {
        self.scrap = None;
    }

    pub(crate) fn scrap(&mut self, units: f64) {
        if let Some(done) = &mut self.scrap {
            done.0 += units;
        }
    }

    pub(crate) fn hurt(&mut self, damage: f64) {
        self.hp.0 -= damage;
    }

    pub(crate) fn heal(&mut self, hp: f64, full: f64) {
        self.hp.0 = (self.hp.0 + hp).min(full);
    }

    pub(crate) fn set_home(&mut self, home: Place) {
        self.home = home;
    }

    pub(crate) fn set_motion(&mut self, motion: Motion) {
        self.motion = motion;
    }

    pub fn flight(&self) -> Option<Flight> {
        match self.motion {
            Motion::Fixed | Motion::Free { flight: None, .. } => None,
            Motion::Free {
                flight: Some(flight),
                ..
            } => Some(flight),
        }
    }

    pub fn is_flying(&self) -> bool {
        self.flight().is_some()
    }
}
