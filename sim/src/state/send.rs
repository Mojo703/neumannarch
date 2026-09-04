use crate::TICKS_PER_SECOND;
use crate::ids::EntityId;
use crate::place::Place;
use crate::state::{Schedule, State};
use crate::time::Tick;

const SEARCH_STEP: u64 = TICKS_PER_SECOND as u64;

const SEARCH_BOUND: u64 = 600 * TICKS_PER_SECOND as u64;

#[derive(Clone, Debug, PartialEq)]
pub struct Send {
    pub source: Place,
    pub destination: Place,
    pub schedule: Schedule,
    pub members: Vec<EntityId>,
}

impl Send {
    pub(crate) fn solved(
        state: &State,
        source: Place,
        destination: Place,
        members: &[EntityId],
    ) -> Option<Send> {
        let gravity = state.gravity();
        let depart = state.tick().next();
        let from = state.anchor(source).at(depart, gravity);
        let limit = state.roster().movement_limit().0;
        (SEARCH_STEP..=SEARCH_BOUND)
            .step_by(SEARCH_STEP as usize)
            .find_map(|step| {
                let arrive = Tick(depart.0 + step);
                let to = state.anchor(destination).at(arrive, gravity);
                Schedule::between(from, to, depart, arrive, limit, gravity).map(|schedule| Send {
                    source,
                    destination,
                    schedule,
                    members: members.to_vec(),
                })
            })
    }
}
