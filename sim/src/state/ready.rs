use crate::ids::EntityId;
use crate::time::Moment;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Ready {
    entity: EntityId,
    weapon: u8,
    at: Moment,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct ReadyWeapons(Vec<Ready>);

impl Ready {
    pub(crate) fn new(entity: EntityId, weapon: u8, at: Moment) -> Ready {
        Ready { entity, weapon, at }
    }

    pub(crate) fn entity(&self) -> EntityId {
        self.entity
    }

    pub(crate) fn weapon(&self) -> u8 {
        self.weapon
    }

    pub(crate) fn at(&self) -> Moment {
        self.at
    }
}

impl ReadyWeapons {
    pub(crate) fn arm(&mut self, entity: EntityId, weapon: u8, at: Moment) {
        if let Ok(found) = self.place_of(entity, weapon) {
            self.0[found].at = at;
        }
    }

    pub(crate) fn armed(
        &mut self,
        entity: EntityId,
        weapons: impl Iterator<Item = u8>,
        at: Moment,
    ) {
        self.0
            .extend(weapons.map(|weapon| Ready::new(entity, weapon, at)));
    }

    pub(crate) fn reap(&mut self, dead: &[EntityId]) {
        if dead.is_empty() {
            return;
        }
        self.0
            .retain(|ready| dead.binary_search(&ready.entity).is_err());
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Ready> {
        self.0.iter()
    }

    fn place_of(&self, entity: EntityId, weapon: u8) -> Result<usize, usize> {
        self.0
            .binary_search_by(|ready| ready.entity.cmp(&entity).then(ready.weapon.cmp(&weapon)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn armed(entities: [u32; 3]) -> ReadyWeapons {
        let mut ready = ReadyWeapons::default();
        for id in entities {
            ready.armed(EntityId(id), [0, 1].into_iter(), Moment(f64::from(id)));
        }
        ready
    }

    #[test]
    fn arming_a_weapon_moves_that_moment_and_no_other() {
        let mut ready = armed([0, 1, 2]);

        ready.arm(EntityId(1), 1, Moment(9.0));

        assert_eq!(
            ready
                .iter()
                .find(|one| one.entity() == EntityId(1) && one.weapon() == 1)
                .map(Ready::at),
            Some(Moment(9.0))
        );
        assert_eq!(
            ready.iter().filter(|one| one.at() == Moment(9.0)).count(),
            1
        );
    }

    #[test]
    fn reaping_takes_the_dead_entities_weapons_and_leaves_the_rest() {
        let mut ready = armed([0, 1, 2]);

        ready.reap(&[EntityId(1)]);

        assert_eq!(
            ready.iter().map(Ready::entity).collect::<Vec<EntityId>>(),
            vec![EntityId(0), EntityId(0), EntityId(2), EntityId(2)]
        );
        assert_eq!(
            ready.iter().map(Ready::weapon).collect::<Vec<u8>>(),
            vec![0, 1, 0, 1]
        );
    }
}
