use crate::ids::EntityId;
use crate::time::Moment;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Ready {
    entity: EntityId,
    weapon: u8,
    at: Moment,
}

impl Ready {
    pub fn new(entity: EntityId, weapon: u8, at: Moment) -> Ready {
        Ready { entity, weapon, at }
    }

    pub fn entity(&self) -> EntityId {
        self.entity
    }

    pub fn weapon(&self) -> u8 {
        self.weapon
    }

    pub fn at(&self) -> Moment {
        self.at
    }

    pub(crate) fn arm(&mut self, at: Moment) {
        self.at = at;
    }
}
