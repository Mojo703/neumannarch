use crate::ids::EntityId;
use crate::time::Moment;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Ready {
    entity: EntityId,
    weapon: u8,
    at: Moment,
    kept: Option<EntityId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct ReadyWeapons(Vec<Ready>);

impl Ready {
    pub(crate) fn new(entity: EntityId, weapon: u8, at: Moment, kept: Option<EntityId>) -> Ready {
        Ready {
            entity,
            weapon,
            at,
            kept,
        }
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

    pub(crate) fn kept(&self) -> Option<EntityId> {
        self.kept
    }
}

impl ReadyWeapons {
    pub(crate) fn arm(&mut self, entity: EntityId, weapon: u8, at: Moment, kept: Option<EntityId>) {
        if let Ok(found) = self.place_of(entity, weapon) {
            self.0[found].at = at;
            self.0[found].kept = kept;
        }
    }

    pub(crate) fn armed(
        &mut self,
        entity: EntityId,
        weapons: impl Iterator<Item = u8>,
        at: Moment,
    ) {
        self.0
            .extend(weapons.map(|weapon| Ready::new(entity, weapon, at, None)));
    }

    pub(crate) fn reap(&mut self, dead: &[EntityId]) {
        if dead.is_empty() {
            return;
        }
        self.0
            .retain(|ready| dead.binary_search(&ready.entity).is_err());
        for ready in &mut self.0 {
            if ready
                .kept
                .is_some_and(|target| dead.binary_search(&target).is_ok())
            {
                ready.kept = None;
            }
        }
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
    fn arming_a_weapon_moves_that_moment_and_its_keep_and_no_other() {
        let mut ready = armed([0, 1, 2]);

        ready.arm(EntityId(1), 1, Moment(9.0), Some(EntityId(2)));

        let armed = ready
            .iter()
            .find(|one| one.entity() == EntityId(1) && one.weapon() == 1)
            .expect("the weapon that was armed");
        assert_eq!(armed.at(), Moment(9.0));
        assert_eq!(armed.kept(), Some(EntityId(2)));
        assert_eq!(
            ready.iter().filter(|one| one.at() == Moment(9.0)).count(),
            1
        );
        assert_eq!(ready.iter().filter(|one| one.kept().is_some()).count(), 1);
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

    #[test]
    fn reaping_drops_a_keep_on_a_dead_target_and_keeps_the_others() {
        let mut ready = armed([0, 1, 2]);
        ready.arm(EntityId(0), 0, Moment(1.0), Some(EntityId(1)));
        ready.arm(EntityId(2), 0, Moment(1.0), Some(EntityId(0)));

        ready.reap(&[EntityId(1)]);

        assert_eq!(
            ready
                .iter()
                .map(Ready::kept)
                .collect::<Vec<Option<EntityId>>>(),
            vec![None, None, Some(EntityId(0)), None],
            "a weapon kept a target the reap took"
        );
    }
}
