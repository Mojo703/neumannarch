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
            .entities
            .in_transit_to(route.destination)
            .filter(|entity| entity.seat() == route.seat)
            .filter_map(|entity| entity.flight())
            .find(|flight| flight.source() == route.source && !flight.has_departed(state.time()))
            .map(|flight| flight.schedule())
    }

    fn solved(state: &State, source: AsteroidId, destination: AsteroidId) -> Option<Schedule> {
        let gravity = state.gravity();
        let depart = Time(state.time().0 + Send::FORMING_TICKS).next();
        let from = state[source].orbit().at(depart, gravity);
        let limit = state.roster().movement_limit().0;
        let arrival = |candidate: u64| Time(depart.0 + candidate * SEARCH_STEP);
        let target = |arrive: Time| state[destination].orbit().at(arrive, gravity);
        let burns_fit = |candidate: u64| {
            let arrive = arrival(candidate);
            Schedule::burns_fit(from, target(arrive), depart, arrive, limit, gravity)
        };
        let last = SEARCH_BOUND / SEARCH_STEP;
        if !burns_fit(last) {
            return None;
        }
        let mut earliest = 1;
        let mut fitting = last;
        while earliest < fitting {
            let middle = earliest + (fitting - earliest) / 2;
            match burns_fit(middle) {
                true => fitting = middle,
                false => earliest = middle + 1,
            }
        }
        (fitting..=last).find_map(|candidate| {
            let arrive = arrival(candidate);
            Schedule::between(from, target(arrive), depart, arrive, limit, gravity)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS_PER_SECOND;
    use crate::fixture::World;

    const NEIGHBOUR_METERS: core::ops::Range<f64> = 1_800.0..2_200.0;

    fn hops(state: &State) -> Vec<f64> {
        let asteroids = state.asteroids().count();
        let body = |at: usize| state.asteroid_body(AsteroidId(at as u32));
        let mut hops = Vec::new();
        for source in 0..asteroids {
            for destination in 0..asteroids {
                if source == destination
                    || !NEIGHBOUR_METERS.contains(&body(source).pos.distance(body(destination).pos))
                {
                    continue;
                }
                hops.push(flown(state, source, destination).expect("a neighbour is reachable"));
            }
        }
        hops.sort_by(f64::total_cmp);
        hops
    }

    fn flown(state: &State, source: usize, destination: usize) -> Option<f64> {
        Send::solved(
            state,
            AsteroidId(source as u32),
            AsteroidId(destination as u32),
        )
        .map(|schedule| (schedule.arrive().0 - state.time().0) as f64 / f64::from(TICKS_PER_SECOND))
    }

    fn farthest(state: &State) -> (usize, usize) {
        let asteroids = state.asteroids().count();
        let apart = |source: usize, destination: usize| {
            state
                .asteroid_body(AsteroidId(source as u32))
                .pos
                .distance(state.asteroid_body(AsteroidId(destination as u32)).pos)
        };
        (0..asteroids)
            .flat_map(|source| {
                (source + 1..asteroids).map(move |destination| (source, destination))
            })
            .max_by(|one, other| apart(one.0, one.1).total_cmp(&apart(other.0, other.1)))
            .expect("the belt holds two asteroids")
    }

    #[test]
    fn a_hop_between_asteroids_two_kilometers_apart_arrives_in_about_twenty_seconds() {
        let world = World::seated(Vec::new());
        let hops = hops(&world.state);
        let middling = hops
            .get(hops.len() / 2)
            .copied()
            .expect("the belt holds two kilometer hops");
        assert!(
            (15.0..=25.0).contains(&middling),
            "half the two kilometer hops take longer than {middling} seconds"
        );
    }

    fn walked(state: &State, source: usize, destination: usize) -> Option<Time> {
        let gravity = state.gravity();
        let depart = Time(state.time().0 + Send::FORMING_TICKS).next();
        let from = state[AsteroidId(source as u32)].orbit().at(depart, gravity);
        let limit = state.roster().movement_limit().0;
        (SEARCH_STEP..=SEARCH_BOUND)
            .step_by(SEARCH_STEP as usize)
            .find_map(|step| {
                let arrive = Time(depart.0 + step);
                let to = state[AsteroidId(destination as u32)]
                    .orbit()
                    .at(arrive, gravity);
                Schedule::between(from, to, depart, arrive, limit, gravity).map(|_| arrive)
            })
    }

    #[test]
    fn the_solve_arrives_when_a_walk_over_every_candidate_arrival_would() {
        let world = World::seated(Vec::new());
        let state = &world.state;
        let (far_source, far_destination) = farthest(state);
        let pairs = [(0, 1), (0, 2), (3, 4), (far_source, far_destination)];
        for (source, destination) in pairs {
            let solved = Send::solved(
                state,
                AsteroidId(source as u32),
                AsteroidId(destination as u32),
            )
            .map(|schedule| schedule.arrive());
            assert_eq!(
                solved,
                walked(state, source, destination),
                "from {source} to {destination}"
            );
        }
    }

    #[test]
    fn the_widest_transfer_across_the_belt_arrives_inside_the_search_bound() {
        let world = World::seated(Vec::new());
        let (source, destination) = farthest(&world.state);
        let seconds = flown(&world.state, source, destination).expect("the widest transfer flies");
        assert!(
            seconds * f64::from(TICKS_PER_SECOND) < SEARCH_BOUND as f64,
            "the widest transfer takes {seconds} seconds"
        );
    }
}
