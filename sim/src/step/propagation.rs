use crate::ids::EntityId;
use crate::orbit::body::Body;
use crate::state::{Entity, Flight, Motion, State};
use crate::step::holding::Thrusts;
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub entity: EntityId,
    pub body: Body,
    pub flight: Option<Flight>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Moved(Vec<Move>);

pub struct Propagation<'a> {
    state: &'a State,
    thrusts: &'a Thrusts,
}

impl<'a> Propagation<'a> {
    pub fn of(state: &'a State, thrusts: &'a Thrusts) -> Propagation<'a> {
        Propagation { state, thrusts }
    }

    pub fn run(self) -> Moved {
        Moved(
            self.state
                .entities()
                .filter_map(|entity| self.moved(entity))
                .collect(),
        )
    }

    fn moved(&self, entity: &Entity) -> Option<Move> {
        let Motion::Steered { body, flight } = entity.motion() else {
            return None;
        };
        let tick = self.state.tick();
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
        match flight.filter(|flight| flight.departs() == self.state.tick()) {
            Some(flight) => Body::new(body.pos, self.state.rock_body(flight.source()).vel),
            None => body,
        }
    }
}

impl Moved {
    pub fn iter(&self) -> impl Iterator<Item = Move> + '_ {
        self.0.iter().copied()
    }
}
