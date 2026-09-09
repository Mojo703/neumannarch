use core::ops::Range;
use std::collections::BTreeMap;

use super::State;
use super::entities::Entity;
use super::roll::Roll;
use crate::ids::{EntityId, TeamId};
use crate::real::Real;
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Aim {
    pub(crate) target: EntityId,
    pub(crate) distance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shooter {
    pub(crate) team: TeamId,
    pub(crate) plating: Real,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AssignedDamage(BTreeMap<EntityId, f64>);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Threats {
    rankings: Vec<Ranking>,
}

#[derive(Clone, Debug, PartialEq)]
struct Ranking {
    shooter: Shooter,
    ranked: Vec<Ranked>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ranked {
    at: u32,
    threat: f64,
}

impl Threats {
    pub(crate) fn among(state: &State, standing: Range<usize>) -> Threats {
        let mut rankings = Vec::new();
        for shooter in shooters(state, standing.clone()) {
            let mut ranked: Vec<Ranked> = standing
                .clone()
                .filter(|at| state[state.entities.at(*at).seat()].team() != shooter.team)
                .map(|at| {
                    let target = state.entities.at(at);
                    Ranked {
                        at: at as u32,
                        threat: state[target.row()].dps_through(shooter.plating.0) / target.hp(),
                    }
                })
                .collect();
            ranked.sort_by(|first, second| {
                second.threat.total_cmp(&first.threat).then(
                    state
                        .entities
                        .at(first.at as usize)
                        .id()
                        .cmp(&state.entities.at(second.at as usize).id()),
                )
            });
            rankings.push(Ranking { shooter, ranked });
        }
        Threats { rankings }
    }

    pub(crate) fn best(
        &self,
        shooter: Shooter,
        from: Vec3,
        range: f64,
        dealt: &AssignedDamage,
        roll: &Roll,
    ) -> Option<Aim> {
        let ranked = &self
            .rankings
            .iter()
            .find(|ranking| ranking.shooter == shooter)?
            .ranked;
        let mut best: Option<Aim> = None;
        let mut taken = 0.0;
        for target in ranked {
            if best.is_some() && target.threat < taken {
                break;
            }
            let prey = roll.at(target.at as usize);
            if !dealt.survived(prey) {
                continue;
            }
            let distance = roll.body_of(prey).pos.distance(from);
            if distance > range {
                continue;
            }
            if best.is_none_or(|aim| distance < aim.distance) {
                best = Some(Aim {
                    target: prey.id(),
                    distance,
                });
                taken = target.threat;
            }
        }
        best
    }
}

impl AssignedDamage {
    pub(crate) fn take(&mut self, target: EntityId, damage: f64) {
        *self.0.entry(target).or_default() += damage;
    }

    pub(crate) fn survived(&self, target: Entity) -> bool {
        target.hp() > self.0.get(&target.id()).copied().unwrap_or(0.0)
    }
}

fn shooters(state: &State, standing: Range<usize>) -> Vec<Shooter> {
    let mut shooters: Vec<Shooter> = standing
        .map(|at| state.entities.at(at))
        .filter(|entity| state[entity.row()].is_armed())
        .map(|entity| Shooter {
            team: state[entity.seat()].team(),
            plating: state[entity.row()].plating,
        })
        .collect();
    shooters.sort_unstable_by(|first, second| {
        first
            .team
            .cmp(&second.team)
            .then(first.plating.0.total_cmp(&second.plating.0))
    });
    shooters.dedup();
    shooters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::belt::Belt;
    use crate::fixture::World;
    use crate::ids::{AsteroidId, EntityId};
    use crate::orbit::body::Gravity;
    use crate::roster::{CONSTRUCTOR, FRIGATE, RAIDER};
    use crate::state::Rolls;

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: AsteroidId = AsteroidId(0);

    fn world() -> World {
        World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)])
    }

    fn shooting(state: &State, shooter: EntityId) -> Shooter {
        let entity = state.entity(shooter);
        Shooter {
            team: state[entity.seat()].team(),
            plating: state[entity.row()].plating,
        }
    }

    fn aimed_within(world: &World, shooter: EntityId, range: f64) -> Option<Aim> {
        let rolls = Rolls::called(&world.state);
        rolls[HOME].best(
            shooting(&world.state, shooter),
            world.state.body_of(world.state.entity(shooter)).pos,
            range,
            &AssignedDamage::default(),
        )
    }

    fn aimed(world: &World, shooter: EntityId) -> Option<Aim> {
        aimed_within(world, shooter, Belt::ZONE_RADIUS_METERS)
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
    fn an_enemy_at_another_asteroid_is_never_prey() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        world.hold(1, RAIDER, AsteroidId(1), 0.0);

        assert_eq!(aimed(&world, hunter), None);
    }

    #[test]
    fn a_target_beyond_the_range_is_never_taken() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        let far = world.hold(1, RAIDER, HOME, 8.0);

        assert_eq!(aimed_within(&world, hunter, 4.0), None);
        assert_eq!(
            aimed_within(&world, hunter, 12.0).map(|aim| aim.target),
            Some(far)
        );
    }

    #[test]
    fn damage_already_assigned_this_tick_passes_over_a_dead_target() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        let dangerous = world.hold(1, RAIDER, HOME, 2.0);
        let harmless = world.hold(1, CONSTRUCTOR, HOME, 4.0);
        let rolls = Rolls::called(&world.state);
        let from = world.state.body_of(world.state.entity(hunter)).pos;
        let shooter = shooting(&world.state, hunter);
        let mut dealt = AssignedDamage::default();

        let first = rolls[HOME].best(shooter, from, Belt::ZONE_RADIUS_METERS, &dealt);
        assert_eq!(first.map(|aim| aim.target), Some(dangerous));

        dealt.take(dangerous, world.state.entity(dangerous).hp());

        let second = rolls[HOME].best(shooter, from, Belt::ZONE_RADIUS_METERS, &dealt);
        assert_eq!(second.map(|aim| aim.target), Some(harmless));
    }

    #[test]
    fn ties_break_by_the_nearer_target_then_the_lower_id() {
        let mut world = world();
        let hunter = world.hold(0, FRIGATE, HOME, 0.0);
        let near = world.hold(1, CONSTRUCTOR, HOME, 2.0);
        world.hold(1, CONSTRUCTOR, HOME, 6.0);

        assert_eq!(aimed(&world, hunter).map(|aim| aim.target), Some(near));
    }

    #[test]
    fn a_tie_at_one_point_breaks_by_the_lower_id() {
        let mut coincident = world();
        let hunter = coincident.hold(0, FRIGATE, HOME, 0.0);
        let first = coincident.hold(1, CONSTRUCTOR, HOME, 3.0);
        let second = coincident.hold(1, CONSTRUCTOR, HOME, 3.0);
        assert!(first < second);

        assert_eq!(
            aimed(&coincident, hunter).map(|aim| aim.target),
            Some(first),
            "two enemies of one row at one point: the lower id"
        );
    }
}
