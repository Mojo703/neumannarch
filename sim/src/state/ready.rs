use crate::ids::EntityId;
use crate::pattern::Slot;
use crate::time::Moment;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Ready {
    entity: EntityId,
    slot: Slot,
    at: Moment,
    kept: Option<EntityId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct ReadyDamage(Vec<Ready>);

impl Ready {
    pub(crate) fn new(entity: EntityId, slot: Slot, at: Moment, kept: Option<EntityId>) -> Ready {
        Ready {
            entity,
            slot,
            at,
            kept,
        }
    }

    pub(crate) fn entity(&self) -> EntityId {
        self.entity
    }

    pub(crate) fn slot(&self) -> Slot {
        self.slot
    }

    pub(crate) fn at(&self) -> Moment {
        self.at
    }

    pub(crate) fn kept(&self) -> Option<EntityId> {
        self.kept
    }
}

impl ReadyDamage {
    pub(crate) fn ready_again(
        &mut self,
        entity: EntityId,
        slot: Slot,
        at: Moment,
        kept: Option<EntityId>,
    ) {
        if let Ok(found) = self.index_of(entity, slot) {
            self.0[found].at = at;
            self.0[found].kept = kept;
        }
    }

    pub(crate) fn ready_from(
        &mut self,
        entity: EntityId,
        slots: impl Iterator<Item = Slot>,
        at: Moment,
    ) {
        self.0
            .extend(slots.map(|slot| Ready::new(entity, slot, at, None)));
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

    fn index_of(&self, entity: EntityId, slot: Slot) -> Result<usize, usize> {
        self.0
            .binary_search_by(|ready| ready.entity.cmp(&entity).then(ready.slot.cmp(&slot)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_slots_each(entities: [u32; 3]) -> ReadyDamage {
        let mut ready = ReadyDamage::default();
        for id in entities {
            ready.ready_from(EntityId(id), Slot::EVERY.into_iter(), Moment(f64::from(id)));
        }
        ready
    }

    #[test]
    fn readying_one_slot_again_moves_that_moment_and_its_keep_and_no_other() {
        let mut ready = two_slots_each([0, 1, 2]);

        ready.ready_again(EntityId(1), Slot::Second, Moment(9.0), Some(EntityId(2)));

        let moved = ready
            .iter()
            .find(|one| one.entity() == EntityId(1) && one.slot() == Slot::Second)
            .expect("the slot that was readied again");
        assert_eq!(moved.at(), Moment(9.0));
        assert_eq!(moved.kept(), Some(EntityId(2)));
        assert_eq!(
            ready.iter().filter(|one| one.at() == Moment(9.0)).count(),
            1
        );
        assert_eq!(ready.iter().filter(|one| one.kept().is_some()).count(), 1);
    }

    #[test]
    fn reaping_takes_every_dead_entitys_slots_and_leaves_the_rest() {
        let mut ready = two_slots_each([0, 1, 2]);

        ready.reap(&[EntityId(1)]);

        assert_eq!(
            ready.iter().map(Ready::entity).collect::<Vec<EntityId>>(),
            vec![EntityId(0), EntityId(0), EntityId(2), EntityId(2)]
        );
        assert_eq!(
            ready.iter().map(Ready::slot).collect::<Vec<Slot>>(),
            vec![Slot::First, Slot::Second, Slot::First, Slot::Second]
        );
    }

    #[test]
    fn reaping_drops_a_keep_on_a_dead_target_and_keeps_the_others() {
        let mut ready = two_slots_each([0, 1, 2]);
        ready.ready_again(EntityId(0), Slot::First, Moment(1.0), Some(EntityId(1)));
        ready.ready_again(EntityId(2), Slot::First, Moment(1.0), Some(EntityId(0)));

        ready.reap(&[EntityId(1)]);

        assert_eq!(
            ready
                .iter()
                .map(Ready::kept)
                .collect::<Vec<Option<EntityId>>>(),
            vec![None, None, Some(EntityId(0)), None],
            "a slot kept a target the reap took"
        );
    }
}
