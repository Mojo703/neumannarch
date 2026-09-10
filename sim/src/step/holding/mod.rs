use std::collections::BTreeMap;

use crate::ids::EntityId;
use crate::orbit::body::Body;
use crate::roster::Row;
use crate::state::{Entity, Pass, Roll, Rolls, State};
use crate::step::fire::Shots;
use crate::time::RunningSpan;
use crate::transfer::Transfer;
use crate::vec3::Vec3;

pub(crate) struct Holding<'a> {
    state: &'a State,
    rolls: &'a Rolls<'a>,
    shots: &'a Shots,
    over: RunningSpan,
}

pub(crate) struct HeldUnit<'a> {
    state: &'a State,
    entity: Entity<'a>,
    roll: &'a Roll<'a>,
    shots: &'a Shots,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Thrusts(BTreeMap<EntityId, Vec3>);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Passes(BTreeMap<EntityId, Pass>);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Steering {
    pub(crate) thrusts: Thrusts,
    pub(crate) passes: Passes,
}

impl<'a> Holding<'a> {
    pub(crate) fn of(
        state: &'a State,
        rolls: &'a Rolls<'a>,
        shots: &'a Shots,
        over: RunningSpan,
    ) -> Holding<'a> {
        Holding {
            state,
            rolls,
            shots,
            over,
        }
    }

    pub(crate) fn run(self) -> Steering {
        let mut steering = Steering::default();
        for roll in self.rolls.iter() {
            for entity in roll.standing() {
                let Some(held) = HeldUnit::of(self.state, entity, roll, self.shots) else {
                    continue;
                };
                let (thrust, pass) = held.steering();
                steering.thrusts.0.insert(entity.id(), thrust);
                if let Some(pass) = pass {
                    steering.passes.0.insert(entity.id(), pass);
                }
            }
        }
        for entity in self.state.entities.flying() {
            if let Some(thrust) = self.transfer_thrust(entity) {
                steering.thrusts.0.insert(entity.id(), thrust);
            }
        }
        steering
    }

    fn transfer_thrust(&self, entity: Entity) -> Option<Vec3> {
        let body = entity.steered()?;
        let destination = self.state.asteroid_body(entity.home());
        let limit = self.state.roster().movement_limit().0;
        Some(Transfer::of(body, destination, limit).thrust(self.over))
    }
}

impl<'a> HeldUnit<'a> {
    pub(crate) fn of(
        state: &'a State,
        entity: Entity<'a>,
        roll: &'a Roll<'a>,
        shots: &'a Shots,
    ) -> Option<HeldUnit<'a>> {
        entity.steered().is_some().then_some(HeldUnit {
            state,
            entity,
            roll,
            shots,
        })
    }

    pub(crate) fn steering(&self) -> (Vec3, Option<Pass>) {
        match self.place() {
            None => (self.thrust(None), None),
            Some((place, pass)) => (self.thrust(Some(place)), pass),
        }
    }

    fn thrust(&self, place: Option<Vec3>) -> Vec3 {
        let row = self.row();
        let body = self.body();
        let asteroid = &self.state[self.roll.asteroid()];
        let sum = terms::separation(body, row, self.neighbours())
            + terms::wander(row, self.state.time(), self.entity.id())
            + terms::returning(
                body,
                row,
                self.roll.body(),
                asteroid.toward_shell(self.roll.body(), body.pos),
            )
            + place.map_or(Vec3::ZERO, |place| {
                terms::stationing(body, row, Body::new(place, self.roll.body().vel))
            });
        sum.capped(row.manoeuvring.0)
    }

    fn place(&self) -> Option<(Vec3, Option<Pass>)> {
        let station = self.station()?;
        match self.passing() {
            Some((place, pass)) => Some((place, Some(pass))),
            None => Some((station, None)),
        }
    }

    pub(super) fn station(&self) -> Option<Vec3> {
        self.roll.station(self.entity.id())
    }

    fn body(&self) -> Body {
        self.roll.body_of(self.entity)
    }

    fn row(&self) -> &'a Row {
        &self.state[self.entity.row()]
    }

    fn neighbours(&self) -> impl Iterator<Item = Vec3> + 'a {
        let mine = self.entity.id();
        self.roll
            .standing()
            .filter(move |other| other.id() != mine)
            .filter_map(|other| other.steered().map(|body| body.pos))
    }
}

impl Thrusts {
    pub fn of(&self, id: EntityId) -> Vec3 {
        self.0.get(&id).copied().unwrap_or(Vec3::ZERO)
    }
}

impl Passes {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (EntityId, Pass)> {
        self.0.iter().map(|(id, pass)| (*id, *pass))
    }
}

impl Steering {
    pub(crate) fn set_passes(&self, next: &mut State) {
        for (entity, pass) in self.passes.iter() {
            next.entities.set_pass(entity, pass);
        }
    }
}

mod pass;
mod terms;

#[cfg(test)]
mod tests;
