use std::collections::BTreeMap;

use field::Fields;

use crate::ids::EntityId;
use crate::orbit::body::Body;
use crate::roster::Row;
use crate::state::{AssignedDamage, Entity, Roll, Rolls, Shooter, State};
use crate::vec3::Vec3;

pub(crate) struct Holding<'a> {
    state: &'a State,
    rolls: &'a Rolls<'a>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Thrusts(BTreeMap<EntityId, Vec3>);

impl<'a> Holding<'a> {
    pub(crate) fn of(state: &'a State, rolls: &'a Rolls<'a>) -> Holding<'a> {
        Holding { state, rolls }
    }

    pub(crate) fn run(self) -> Thrusts {
        let fields = Fields::among(self.state, self.rolls);
        let mut thrusts = BTreeMap::new();
        for roll in self.rolls.iter() {
            for entity in roll.standing() {
                if let Some(thrust) = self.thrust(entity, roll, &fields) {
                    thrusts.insert(entity.id(), thrust);
                }
            }
        }
        Thrusts(thrusts)
    }

    fn thrust(&self, entity: Entity, roll: &Roll, fields: &Fields) -> Option<Vec3> {
        let body = entity.steered()?;
        let row = &self.state[entity.row()];
        let sample = fields.at(entity.id());
        let asteroid = &self.state[roll.asteroid()];
        let sum = terms::separation(body, row, self.neighbours(entity, roll))
            + terms::wander(row, self.state.time(), entity.id())
            + terms::returning(
                body,
                row,
                roll.body(),
                asteroid.strayed(roll.body(), body.pos),
            )
            + terms::cohesion(row, sample)
            + terms::caution(row, sample)
            + self.chasing(entity, body, row, roll);
        Some(sum.capped(row.manoeuvring.0))
    }

    fn chasing(&self, entity: Entity, body: Body, row: &Row, roll: &Roll) -> Vec3 {
        let Some(standoff) = row.standoff() else {
            return Vec3::ZERO;
        };
        let shooter = Shooter {
            team: self.state[entity.seat()].team(),
            plating: row.plating,
        };
        let Some(aim) = roll.best(shooter, body.pos, f64::INFINITY, &AssignedDamage::default())
        else {
            return Vec3::ZERO;
        };
        let prey = roll.body_of(self.state.entity(aim.target));
        let Some(toward) = (body.pos - prey.pos).normalized() else {
            return Vec3::ZERO;
        };
        terms::chase(body, row, Body::new(prey.pos + toward * standoff, prey.vel))
    }

    fn neighbours(&self, entity: Entity, roll: &'a Roll<'a>) -> impl Iterator<Item = Vec3> + 'a {
        let mine = entity.id();
        roll.standing()
            .filter(move |other| other.id() != mine)
            .filter_map(|other| other.steered().map(|body| body.pos))
    }
}

impl Thrusts {
    pub fn of(&self, id: EntityId) -> Vec3 {
        self.0.get(&id).copied().unwrap_or(Vec3::ZERO)
    }
}

mod field;
mod power;
mod terms;

#[cfg(test)]
mod tests;
