use std::collections::BTreeMap;

use super::State;
use super::entity::Entity;
use crate::ids::{EntityId, RockId, TeamId};
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub target: EntityId,
    pub distance: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assigned(BTreeMap<EntityId, f64>);

pub struct Threat<'a> {
    state: &'a State,
    here: RockId,
    team: TeamId,
    from: Vec3,
    dealt: Vec<f64>,
}

impl<'a> Threat<'a> {
    pub fn of(state: &'a State, shooter: &Entity) -> Option<Threat<'a>> {
        let plating = state[shooter.row()].plating.0;
        Some(Threat {
            state,
            here: shooter.standing(state.time())?,
            team: state[shooter.seat()].team(),
            from: state.body_of(shooter).pos,
            dealt: state
                .roster()
                .iter()
                .map(|(_, row)| row.dps_through(plating))
                .collect(),
        })
    }

    pub fn best<'e>(
        &self,
        candidates: impl Iterator<Item = &'e Entity>,
        assigned: &Assigned,
    ) -> Option<Aim> {
        candidates
            .filter(|target| self.is_prey(target, assigned))
            .map(|target| {
                let dealt = self.dealt[usize::from(target.row().0)];
                let distance = self.state.body_of(target).pos.distance(self.from);
                (dealt / target.hp(), distance, target.id())
            })
            .max_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then(b.1.total_cmp(&a.1))
                    .then(b.2.cmp(&a.2))
            })
            .map(|(_, distance, target)| Aim { target, distance })
    }

    fn is_prey(&self, target: &Entity, assigned: &Assigned) -> bool {
        self.state[target.seat()].team() != self.team
            && target.standing(self.state.time()) == Some(self.here)
            && assigned.survived(target)
    }
}

impl Assigned {
    pub fn take(&mut self, target: EntityId, damage: f64) {
        *self.0.entry(target).or_default() += damage;
    }

    fn survived(&self, target: &Entity) -> bool {
        target.hp() > self.0.get(&target.id()).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::belt::Belt;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::orbit::body::Gravity;
    use crate::roster::{CONSTRUCTOR, FRIGATE, RAIDER};

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: RockId = RockId(0);

    fn world() -> World {
        World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)])
    }

    fn aimed(world: &World, shooter: EntityId) -> Option<Aim> {
        let sweep = world.state.sweep();
        let here = world.state.rock_body(HOME).pos;
        Threat::of(&world.state, &world.state[shooter])?.best(
            sweep
                .within(here, Belt::ZONE_RADIUS_METERS)
                .filter_map(|id| world.state.entity(id)),
            &Assigned::default(),
        )
    }

    #[test]
    fn the_highest_threat_through_the_shooters_plating_is_the_target() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        world.hold(1, CONSTRUCTOR, HOME, 2.0);
        let dangerous = world.hold(1, RAIDER, HOME, 4.0);

        assert_eq!(aimed(&world, hunter).map(|aim| aim.target), Some(dangerous));
    }

    #[test]
    fn a_teammate_is_never_prey() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        world.hold(0, RAIDER, HOME, 2.0);

        assert_eq!(aimed(&world, hunter), None);
    }

    #[test]
    fn an_enemy_at_another_rock_is_never_prey() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        world.hold(1, RAIDER, RockId(1), 0.0);

        assert_eq!(aimed(&world, hunter), None);
    }

    #[test]
    fn damage_already_assigned_this_tick_passes_over_a_dead_target() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        let dangerous = world.hold(1, RAIDER, HOME, 2.0);
        let harmless = world.hold(1, CONSTRUCTOR, HOME, 4.0);
        let sweep = world.state.sweep();
        let here = world.state.rock_body(HOME).pos;
        let threat = Threat::of(&world.state, &world.state[hunter]).expect("a standing shooter");

        let mut assigned = Assigned::default();
        assert_eq!(
            threat
                .best(
                    sweep
                        .within(here, Belt::ZONE_RADIUS_METERS)
                        .filter_map(|id| world.state.entity(id)),
                    &assigned
                )
                .map(|aim| aim.target),
            Some(dangerous)
        );
        assigned.take(dangerous, world.state[dangerous].hp());
        assert_eq!(
            threat
                .best(
                    sweep
                        .within(here, Belt::ZONE_RADIUS_METERS)
                        .filter_map(|id| world.state.entity(id)),
                    &assigned
                )
                .map(|aim| aim.target),
            Some(harmless)
        );
    }

    #[test]
    fn ties_break_by_the_nearer_target_then_the_lower_id() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        let near = world.hold(1, CONSTRUCTOR, HOME, 2.0);
        world.hold(1, CONSTRUCTOR, HOME, 6.0);

        assert_eq!(aimed(&world, hunter).map(|aim| aim.target), Some(near));
    }
}
