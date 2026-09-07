use crate::TICKS_PER_SECOND;
use crate::ids::{AsteroidId, EntityId, SeatId};
use crate::state::{Schedule, State};
use crate::time::Time;

const SEARCH_STEP: u64 = TICKS_PER_SECOND as u64;

const SEARCH_BOUND: u64 = 600 * TICKS_PER_SECOND as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Route {
    pub source: AsteroidId,
    pub destination: AsteroidId,
    pub seat: SeatId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Send {
    route: Route,
    pub(crate) schedule: Schedule,
    pub(crate) members: Vec<EntityId>,
}

impl Send {
    pub const FORMING_TICKS: u64 = TICKS_PER_SECOND as u64 / 2;

    pub(crate) fn joining(state: &State, route: Route, members: &[EntityId]) -> Option<Send> {
        let schedule = Send::forming(state, route)
            .or_else(|| Send::solved(state, route.source, route.destination))?;
        Some(Send {
            route,
            schedule,
            members: members.to_vec(),
        })
    }

    pub fn route(&self) -> Route {
        self.route
    }

    fn forming(state: &State, route: Route) -> Option<Schedule> {
        state
            .entities()
            .filter(|entity| entity.seat() == route.seat && entity.home() == route.destination)
            .filter_map(|entity| entity.flight())
            .find(|flight| flight.source() == route.source && !flight.has_departed(state.time()))
            .map(|flight| flight.schedule())
    }

    fn solved(state: &State, source: AsteroidId, destination: AsteroidId) -> Option<Schedule> {
        let gravity = state.gravity();
        let depart = Time(state.time().0 + Send::FORMING_TICKS).next();
        let from = state[source].orbit().at(depart, gravity);
        let limit = state.roster().movement_limit().0;
        (SEARCH_STEP..=SEARCH_BOUND)
            .step_by(SEARCH_STEP as usize)
            .find_map(|step| {
                let arrive = Time(depart.0 + step);
                let to = state[destination].orbit().at(arrive, gravity);
                Schedule::between(from, to, depart, arrive, limit, gravity)
            })
    }
}
