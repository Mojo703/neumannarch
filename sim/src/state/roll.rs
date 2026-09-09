use core::ops::{Index, Range};

use super::State;
use super::entities::{Entities, Entity};
use super::threat::{Aim, AssignedDamage, Shooter, Threats};
use crate::ids::{AsteroidId, SeatId};
use crate::orbit::body::Body;
use crate::vec3::Vec3;

pub(crate) struct Rolls<'a>(Vec<Roll<'a>>);

pub(crate) struct Roll<'a> {
    entities: &'a Entities,
    asteroid: AsteroidId,
    body: Body,
    standing: Range<usize>,
    seats: Vec<Seated>,
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
        Roll {
            entities: &state.entities,
            asteroid,
            body: state.asteroid_body(asteroid),
            seats: seated(&state.entities, standing.clone()),
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
        self.seats.iter().map(|seated| seated.seat)
    }

    pub(crate) fn of_seat(&self, seat: SeatId) -> impl Iterator<Item = Entity<'a>> {
        self.seats
            .iter()
            .find(|seated| seated.seat == seat)
            .map_or(0..0, |seated| seated.run.clone())
            .map(|at| self.entities.at(at))
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
        range: f64,
        dealt: &AssignedDamage,
    ) -> Option<Aim> {
        self.threats.best(shooter, from, range, dealt, self)
    }
}

impl<'a> Index<AsteroidId> for Rolls<'a> {
    type Output = Roll<'a>;

    fn index(&self, asteroid: AsteroidId) -> &Roll<'a> {
        &self.0[asteroid.0 as usize]
    }
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
