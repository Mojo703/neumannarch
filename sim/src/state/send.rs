use crate::TICKS_PER_SECOND;
use crate::ids::{EntityId, RockId, SeatId};
use crate::state::{Schedule, State};
use crate::time::Time;

const SEARCH_STEP: u64 = TICKS_PER_SECOND as u64;

const SEARCH_BOUND: u64 = 600 * TICKS_PER_SECOND as u64;

#[derive(Clone, Debug, PartialEq)]
pub struct Send {
    pub source: RockId,
    pub destination: RockId,
    pub schedule: Schedule,
    pub members: Vec<EntityId>,
}

impl Send {
    pub const FORMING_TICKS: u64 = TICKS_PER_SECOND as u64 / 2;

    pub(crate) fn joining(
        state: &State,
        source: RockId,
        destination: RockId,
        seat: SeatId,
        members: &[EntityId],
    ) -> Option<Send> {
        let schedule = Send::forming(state, source, destination, seat)
            .or_else(|| Send::solved(state, source, destination))?;
        Some(Send {
            source,
            destination,
            schedule,
            members: members.to_vec(),
        })
    }

    fn forming(
        state: &State,
        source: RockId,
        destination: RockId,
        seat: SeatId,
    ) -> Option<Schedule> {
        state
            .entities()
            .filter(|entity| entity.seat() == seat && entity.home() == destination)
            .filter_map(|entity| entity.flight())
            .find(|flight| flight.source() == source && !flight.has_departed(state.time()))
            .map(|flight| flight.schedule())
    }

    fn solved(state: &State, source: RockId, destination: RockId) -> Option<Schedule> {
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
