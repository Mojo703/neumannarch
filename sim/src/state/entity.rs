//! A living copy of a row and how it moves.

use crate::ids::{EntityId, FlightId, RowId, SeatId};
use crate::orbit::body::Body;
use crate::place::Place;
use crate::real::Real;

/// A structure or unit in the match: a copy of `row` owned by `seat`,
/// counting toward `home`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    id: EntityId,
    seat: SeatId,
    row: RowId,
    home: Place,
    hp: Real,
    motion: Motion,
    /// Cost units of scrapping done, once it is marked surplus.
    scrap: Option<Real>,
}

/// How an entity moves. A structure has no body of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// A structure: its body is its rock's.
    Fixed,
    /// A unit with its own body, flying the send it joined until it
    /// arrives.
    Free {
        body: Body,
        flight: Option<FlightId>,
    },
}

impl Entity {
    /// A fresh entity with `hp` hit points.
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

    /// The place this entity counts toward.
    pub fn home(&self) -> Place {
        self.home
    }

    /// Hit points left.
    pub fn hp(&self) -> f64 {
        self.hp.0
    }

    pub fn motion(&self) -> Motion {
        self.motion
    }

    /// True once fulfilment has marked it surplus with nowhere to go; a
    /// builder at its rock scraps it.
    pub fn is_surplus(&self) -> bool {
        self.scrap.is_some()
    }

    /// Cost units of scrapping done, or `None` while it is not surplus.
    pub fn scrapped(&self) -> Option<f64> {
        self.scrap.map(|work| work.0)
    }

    /// Marks it surplus, keeping the scrapping already done.
    pub(crate) fn mark_surplus(&mut self) {
        self.scrap = self.scrap.or(Some(Real(0.0)));
    }

    /// Clears the surplus mark and the scrapping done with it.
    pub(crate) fn clear_surplus(&mut self) {
        self.scrap = None;
    }

    /// Adds `units` cost units of scrapping; nothing when it is not
    /// surplus.
    pub(crate) fn scrap(&mut self, units: f64) {
        if let Some(done) = &mut self.scrap {
            done.0 += units;
        }
    }

    /// Takes `damage` hit points, which may leave it at or below zero.
    pub(crate) fn hurt(&mut self, damage: f64) {
        self.hp.0 -= damage;
    }

    /// Adds `hp` hit points, never above `full`.
    pub(crate) fn heal(&mut self, hp: f64, full: f64) {
        self.hp.0 = (self.hp.0 + hp).min(full);
    }

    /// Moves the place this entity counts toward, as the surplus rule does.
    pub(crate) fn set_home(&mut self, home: Place) {
        self.home = home;
    }

    /// Replaces how this entity moves.
    pub(crate) fn set_motion(&mut self, motion: Motion) {
        self.motion = motion;
    }

    /// The send this entity is flying, if it is flying one.
    pub fn flight(&self) -> Option<FlightId> {
        match self.motion {
            Motion::Fixed | Motion::Free { flight: None, .. } => None,
            Motion::Free {
                flight: Some(id), ..
            } => Some(id),
        }
    }

    /// True from the tick it joins a send until it reaches its destination
    /// anchor. A flying unit neither shoots nor is shot at.
    pub fn is_flying(&self) -> bool {
        self.flight().is_some()
    }
}
