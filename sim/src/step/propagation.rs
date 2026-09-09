use crate::ids::EntityId;
use crate::orbit::body::Body;
use crate::state::{Entity, Flight, State};
use crate::step::holding::Thrusts;
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Move {
    pub(crate) entity: EntityId,
    pub body: Body,
    pub(crate) flight: Option<Flight>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Moved(Vec<Move>);

pub(crate) struct Propagation<'a> {
    state: &'a State,
    thrusts: &'a Thrusts,
}

impl<'a> Propagation<'a> {
    pub(crate) fn of(state: &'a State, thrusts: &'a Thrusts) -> Propagation<'a> {
        Propagation { state, thrusts }
    }

    pub(crate) fn run(self) -> Moved {
        Moved(
            self.state
                .entities
                .steered()
                .filter_map(|entity| self.moved(entity))
                .collect(),
        )
    }

    fn moved(&self, entity: Entity) -> Option<Move> {
        let body = entity.steered()?;
        let flight = entity.flight();
        let tick = self.state.time();
        let scheduled = flight.map_or(Vec3::ZERO, |flight| flight.thrust(tick));
        let thrust = scheduled + self.thrusts.of(entity.id());
        Some(Move {
            entity: entity.id(),
            body: self
                .departing(body, flight)
                .after_tick(thrust, self.state.gravity()),
            flight: flight.filter(|flight| tick.next() < flight.arrive()),
        })
    }

    fn departing(&self, body: Body, flight: Option<Flight>) -> Body {
        match flight.filter(|flight| flight.departs() == self.state.time()) {
            Some(flight) => Body::new(body.pos, self.state.asteroid_body(flight.source()).vel),
            None => body,
        }
    }
}

impl Moved {
    pub(crate) fn iter(&self) -> impl Iterator<Item = Move> + '_ {
        self.0.iter().copied()
    }
}
