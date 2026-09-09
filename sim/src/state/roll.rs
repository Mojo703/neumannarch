use core::ops::{Index, Range};
use std::collections::BTreeMap;

use super::State;
use super::circle::Circle;
use super::entities::{Entities, Entity};
use super::stage::{FightStage, Line};
use super::threat::{Aim, AssignedDamage, Reach, Shooter, Threats};
use crate::ids::{AsteroidId, EntityId, RowId, SeatId, TeamId};
use crate::orbit::body::Body;
use crate::vec3::Vec3;

pub(crate) struct Rolls<'a>(Vec<Roll<'a>>);

pub(crate) struct Roll<'a> {
    entities: &'a Entities,
    asteroid: AsteroidId,
    body: Body,
    standing: Range<usize>,
    seats_ascending: Vec<Seated>,
    stations: BTreeMap<EntityId, Vec3>,
    threats: Threats,
}

struct Seated {
    seat: SeatId,
    run: Range<usize>,
}

impl<'a> Rolls<'a> {
    pub(crate) fn called(state: &'a State) -> Rolls<'a> {
        Rolls(
            state
                .asteroids()
                .map(|(asteroid, _)| Roll::called(state, asteroid))
                .collect(),
        )
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Roll<'a>> {
        self.0.iter()
    }
}

impl<'a> Roll<'a> {
    fn called(state: &'a State, asteroid: AsteroidId) -> Roll<'a> {
        let standing = state.entities.slice(asteroid);
        let body = state.asteroid_body(asteroid);
        let manned = manned(state, asteroid);
        let stage = FightStage::of(
            body,
            state[asteroid].radius(),
            state.roster(),
            &Line::standing_at(state, asteroid),
        );
        let mut stations: BTreeMap<EntityId, Vec3> = manned
            .iter()
            .flat_map(|((team, row), ids)| {
                ids.iter()
                    .copied()
                    .zip(stage.stations_of(*team, *row).iter().copied())
            })
            .collect();
        for ((_, row), ids) in &manned {
            if state[*row].standoff().is_some() {
                continue;
            }
            for id in ids {
                let circle = Circle::of(*id, &state[*row], &state[asteroid], body);
                stations.insert(*id, circle.station(state.time()));
            }
        }
        Roll {
            entities: &state.entities,
            asteroid,
            body,
            seats_ascending: seated(&state.entities, standing.clone()),
            stations,
            threats: Threats::among(state, standing.clone()),
            standing,
        }
    }

    pub(crate) fn asteroid(&self) -> AsteroidId {
        self.asteroid
    }

    pub(crate) fn body(&self) -> Body {
        self.body
    }

    pub(crate) fn standing(&self) -> impl Iterator<Item = Entity<'a>> {
        self.standing.clone().map(|at| self.entities.at(at))
    }

    pub(crate) fn seats(&self) -> impl Iterator<Item = SeatId> {
        self.seats_ascending.iter().map(|seated| seated.seat)
    }

    pub(crate) fn of_seat(&self, seat: SeatId) -> impl Iterator<Item = Entity<'a>> {
        self.seats_ascending
            .binary_search_by_key(&seat, |seated| seated.seat)
            .map_or(0..0, |at| self.seats_ascending[at].run.clone())
            .map(|at| self.entities.at(at))
    }

    pub(crate) fn station(&self, id: EntityId) -> Option<Vec3> {
        self.stations.get(&id).copied()
    }

    pub(crate) fn at(&self, at: usize) -> Entity<'a> {
        self.entities.at(at)
    }

    pub(crate) fn body_of(&self, entity: Entity) -> Body {
        entity.steered().unwrap_or(self.body)
    }

    pub(crate) fn best(
        &self,
        shooter: Shooter,
        from: Vec3,
        reach: Reach,
        dealt: &AssignedDamage,
        kept: Option<EntityId>,
    ) -> Option<Aim> {
        self.threats.best(shooter, from, reach, dealt, kept, self)
    }
}

impl<'a> Index<AsteroidId> for Rolls<'a> {
    type Output = Roll<'a>;

    fn index(&self, asteroid: AsteroidId) -> &Roll<'a> {
        &self.0[asteroid.0 as usize]
    }
}

fn manned(state: &State, asteroid: AsteroidId) -> BTreeMap<(TeamId, RowId), Vec<EntityId>> {
    let mut lines: BTreeMap<(TeamId, RowId), Vec<EntityId>> = BTreeMap::new();
    for entity in state.entities.standing_at(asteroid) {
        if entity.steered().is_none() {
            continue;
        }
        lines
            .entry((state[entity.seat()].team(), entity.row()))
            .or_default()
            .push(entity.id());
    }
    for ids in lines.values_mut() {
        ids.sort_unstable();
    }
    lines
}

fn seated(entities: &Entities, standing: Range<usize>) -> Vec<Seated> {
    let mut seats: Vec<Seated> = Vec::new();
    for at in standing {
        let seat = entities.at(at).seat();
        match seats.last_mut() {
            Some(last) if last.seat == seat => last.run.end = at + 1,
            _ => seats.push(Seated {
                seat,
                run: at..at + 1,
            }),
        }
    }
    seats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::orbit::body::Gravity;
    use crate::roster::{FRIGATE, LANCER};

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: AsteroidId = AsteroidId(0);

    #[test]
    fn a_rolls_seats_read_in_ascending_order_and_each_answers_with_its_own_units() {
        let mut world = World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1), TeamId(0)]);
        let last_seat = world.hold(2, LANCER, HOME, 0.0);
        let first_seat = world.hold(0, FRIGATE, HOME, 1.0);
        let also_first_seat = world.hold(0, LANCER, HOME, 2.0);
        world.hold(1, FRIGATE, AsteroidId(1), 0.0);

        let rolls = Rolls::called(&world.state);
        let roll = &rolls[HOME];

        let seats: Vec<SeatId> = roll.seats().collect();
        assert_eq!(seats, vec![SeatId(0), SeatId(2)]);
        assert!(
            seats.windows(2).all(|pair| pair[0] < pair[1]),
            "the seats do not ascend, and the roll finds one by halving: {seats:?}"
        );
        assert_eq!(
            roll.of_seat(SeatId(0))
                .map(Entity::id)
                .collect::<Vec<EntityId>>(),
            vec![first_seat, also_first_seat]
        );
        assert_eq!(
            roll.of_seat(SeatId(2))
                .map(Entity::id)
                .collect::<Vec<EntityId>>(),
            vec![last_seat]
        );
        assert_eq!(roll.of_seat(SeatId(1)).count(), 0);
    }
}
