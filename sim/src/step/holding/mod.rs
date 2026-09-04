use std::collections::BTreeMap;

use field::Fields;

use crate::belt::Belt;
use crate::ids::{EntityId, RockId};
use crate::orbit::body::Body;
use crate::roster::Row;
use crate::state::sweep::Sweep;
use crate::state::{Assigned, Entity, Motion, State, Threat};
use crate::vec3::Vec3;

pub struct Holding<'a> {
    state: &'a State,
    sweep: &'a Sweep,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Thrusts(BTreeMap<EntityId, Vec3>);

impl<'a> Holding<'a> {
    pub fn of(state: &'a State, sweep: &'a Sweep) -> Holding<'a> {
        Holding { state, sweep }
    }

    pub fn run(self) -> Thrusts {
        let fields = Fields::of(self.state);
        Thrusts(
            self.state
                .entities()
                .filter_map(|entity| Some((entity.id(), self.thrust(entity, &fields)?)))
                .collect(),
        )
    }

    fn thrust(&self, entity: &Entity, fields: &Fields) -> Option<Vec3> {
        let Motion::Steered { body, .. } = entity.motion() else {
            return None;
        };
        let row = &self.state[entity.row()];
        let apart = terms::separation(body, row, self.neighbours(entity, body));
        let Some(here) = entity.standing(self.state.tick()) else {
            return Some(apart.capped(row.manoeuvring.0));
        };
        let sample = fields.at(entity.id());
        let rock = &self.state[here];
        let rock_body = self.state.rock_body(here);
        let sum = apart
            + terms::wander(row, self.state.tick(), entity.id())
            + terms::returning(body, row, rock_body, rock.strayed(rock_body, body.pos))
            + terms::cohesion(row, sample)
            + terms::caution(row, sample)
            + self.chasing(entity, body, row, here);
        Some(sum.capped(row.manoeuvring.0))
    }

    fn chasing(&self, entity: &Entity, body: Body, row: &Row, here: RockId) -> Vec3 {
        let Some(standoff) = row.standoff() else {
            return Vec3::ZERO;
        };
        let Some(target) = self.target(entity, here) else {
            return Vec3::ZERO;
        };
        let prey = self.state.body_of(target);
        let Some(toward) = (body.pos - prey.pos).normalized() else {
            return Vec3::ZERO;
        };
        terms::chase(body, row, Body::new(prey.pos + toward * standoff, prey.vel))
    }

    fn target(&self, entity: &Entity, here: RockId) -> Option<&Entity> {
        let aim = Threat::of(self.state, entity)?
            .best(self.state.standing_at(here), &Assigned::default())?;
        self.state.entity(aim.target)
    }

    fn neighbours(&self, entity: &Entity, body: Body) -> impl Iterator<Item = Vec3> + '_ {
        let mine = entity.id();
        self.sweep
            .within(body.pos, Belt::SPACING_METERS)
            .filter(move |id| *id != mine)
            .filter_map(|id| self.state.entity(id))
            .filter(|other| other.motion() != Motion::Fixed)
            .map(|other| self.state.body_of(other).pos)
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
