//! When a damage weapon next fires.

use crate::ids::EntityId;
use crate::time::Moment;

/// One damage weapon's next ready moment. One exists per damage weapon of
/// a living entity, and `at` is never earlier than the current tick.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ready {
    entity: EntityId,
    weapon: u8,
    at: Moment,
}

impl Ready {
    /// `weapon` is the index of the weapon in its row.
    pub fn new(entity: EntityId, weapon: u8, at: Moment) -> Ready {
        Ready { entity, weapon, at }
    }

    pub fn entity(&self) -> EntityId {
        self.entity
    }

    /// The index of the weapon in its row.
    pub fn weapon(&self) -> u8 {
        self.weapon
    }

    /// The moment the weapon is next ready.
    pub fn at(&self) -> Moment {
        self.at
    }

    /// Sets when the weapon is next ready.
    pub(crate) fn arm(&mut self, at: Moment) {
        self.at = at;
    }
}
