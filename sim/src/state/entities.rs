use core::ops::Range;
use std::collections::BTreeMap;

use super::schedule::Flight;
use crate::ids::{AsteroidId, EntityId, RowId, SeatId};
use crate::orbit::body::Body;
use crate::real::Real;
use crate::time::Time;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Berth {
    Standing(AsteroidId),
    Flying { from: AsteroidId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Motion {
    Fixed,
    Steered { body: Body },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Entities {
    ids: Vec<EntityId>,
    seats: Vec<SeatId>,
    rows: Vec<RowId>,
    homes: Vec<AsteroidId>,
    berths: Vec<Berth>,
    hp: Vec<Real>,
    motions: Vec<Motion>,
    flights: BTreeMap<EntityId, Flight>,
    in_transit: BTreeMap<AsteroidId, Vec<EntityId>>,
    ids_ascending: Vec<EntityId>,
    places_ascending: Vec<u32>,
    next_id: EntityId,
}

#[derive(Clone, Copy, Debug)]
pub struct Entity<'a> {
    entities: &'a Entities,
    at: usize,
}

impl Berth {
    fn of(home: AsteroidId, flight: Option<Flight>, now: Time) -> Berth {
        match flight {
            None => Berth::Standing(home),
            Some(flight) if flight.has_departed(now) => Berth::Flying {
                from: flight.source(),
            },
            Some(flight) => Berth::Standing(flight.source()),
        }
    }

    pub fn standing(self) -> Option<AsteroidId> {
        match self {
            Berth::Standing(asteroid) => Some(asteroid),
            Berth::Flying { .. } => None,
        }
    }

    pub fn flying_from(self) -> Option<AsteroidId> {
        match self {
            Berth::Standing(_) => None,
            Berth::Flying { from } => Some(from),
        }
    }
}

impl Entities {
    pub(crate) fn empty() -> Entities {
        Entities {
            ids: Vec::new(),
            seats: Vec::new(),
            rows: Vec::new(),
            homes: Vec::new(),
            berths: Vec::new(),
            hp: Vec::new(),
            motions: Vec::new(),
            flights: BTreeMap::new(),
            in_transit: BTreeMap::new(),
            ids_ascending: Vec::new(),
            places_ascending: Vec::new(),
            next_id: EntityId(0),
        }
    }

    pub(crate) fn spawn(
        &mut self,
        seat: SeatId,
        row: RowId,
        home: AsteroidId,
        hp: f64,
        motion: Motion,
    ) -> EntityId {
        let id = self.next_id;
        self.next_id = EntityId(id.0 + 1);
        let berth = Berth::Standing(home);
        let at = self.place_for(berth, seat, id);
        self.ids.insert(at, id);
        self.seats.insert(at, seat);
        self.rows.insert(at, row);
        self.homes.insert(at, home);
        self.berths.insert(at, berth);
        self.hp.insert(at, Real(hp));
        self.motions.insert(at, motion);
        for place in &mut self.places_ascending {
            *place += u32::from(*place >= at as u32);
        }
        self.ids_ascending.push(id);
        self.places_ascending.push(at as u32);
        id
    }

    pub(crate) fn entity(&self, id: EntityId) -> Entity<'_> {
        self.at(self.place_of(id))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = Entity<'_>> {
        (0..self.ids.len()).map(|at| self.at(at))
    }

    pub(crate) fn steered(&self) -> impl Iterator<Item = Entity<'_>> {
        self.motions
            .iter()
            .enumerate()
            .filter(|(_, motion)| **motion != Motion::Fixed)
            .map(|(at, _)| self.at(at))
    }

    pub(crate) fn in_id_order(&self) -> impl Iterator<Item = Entity<'_>> {
        self.places_ascending.iter().map(|at| self.at(*at as usize))
    }

    pub(crate) fn standing_at(&self, asteroid: AsteroidId) -> impl Iterator<Item = Entity<'_>> {
        self.slice(asteroid).map(|at| self.at(at))
    }

    pub(crate) fn homed_at(&self, asteroid: AsteroidId) -> impl Iterator<Item = Entity<'_>> {
        self.standing_at(asteroid)
            .filter(move |entity| entity.home() == asteroid)
            .chain(self.in_transit_to(asteroid))
    }

    pub(crate) fn in_transit_to(&self, asteroid: AsteroidId) -> impl Iterator<Item = Entity<'_>> {
        self.in_transit
            .get(&asteroid)
            .into_iter()
            .flatten()
            .map(|id| self.entity(*id))
    }

    pub(super) fn slice(&self, asteroid: AsteroidId) -> Range<usize> {
        let standing = Berth::Standing(asteroid);
        let start = self.berths.partition_point(|berth| *berth < standing);
        let end = self.berths.partition_point(|berth| *berth <= standing);
        start..end
    }

    pub(super) fn at(&self, at: usize) -> Entity<'_> {
        Entity { entities: self, at }
    }

    pub(crate) fn steer(&mut self, id: EntityId, body: Body, flight: Option<Flight>, now: Time) {
        let at = self.place_of(id);
        self.motions[at] = Motion::Steered { body };
        self.set_flight(id, at, flight, now);
    }

    pub(crate) fn join(
        &mut self,
        id: EntityId,
        destination: AsteroidId,
        flight: Flight,
        now: Time,
    ) {
        let at = self.place_of(id);
        self.forget_transit(id, self.homes[at]);
        self.homes[at] = destination;
        self.set_flight(id, at, Some(flight), now);
    }

    pub(crate) fn hurt(&mut self, id: EntityId, damage: f64) {
        let at = self.place_of(id);
        self.hp[at].0 -= damage;
    }

    pub(crate) fn heal(&mut self, id: EntityId, hp: f64, full: f64) {
        let at = self.place_of(id);
        self.hp[at].0 = (self.hp[at].0 + hp).min(full);
    }

    pub(crate) fn reap(&mut self) -> Vec<EntityId> {
        let dead: Vec<EntityId> = self
            .iter()
            .filter(|entity| entity.hp() <= 0.0)
            .map(Entity::id)
            .collect();
        if dead.is_empty() {
            return dead;
        }
        for id in &dead {
            let at = self.place_of(*id);
            self.forget_transit(*id, self.homes[at]);
            self.flights.remove(id);
        }
        let surviving: Vec<u32> = (0..self.ids.len() as u32)
            .filter(|at| self.hp[*at as usize].0 > 0.0)
            .collect();
        self.reorder(&surviving);
        dead
    }

    pub(crate) fn settle(&mut self, now: Time) {
        let flights = core::mem::take(&mut self.flights);
        for (id, flight) in &flights {
            let at = self.place_of(*id);
            self.berths[at] = Berth::of(self.homes[at], Some(*flight), now);
        }
        self.flights = flights;
        if (1..self.ids.len()).all(|at| !self.precedes(at, at - 1)) {
            return;
        }
        let mut order: Vec<u32> = (0..self.ids.len() as u32).collect();
        order.sort_unstable_by(|first, second| {
            self.berths[*first as usize]
                .cmp(&self.berths[*second as usize])
                .then(self.seats[*first as usize].cmp(&self.seats[*second as usize]))
                .then(self.ids[*first as usize].cmp(&self.ids[*second as usize]))
        });
        self.reorder(&order);
    }

    fn reorder(&mut self, order: &[u32]) {
        self.ids = gather(&self.ids, order);
        self.seats = gather(&self.seats, order);
        self.rows = gather(&self.rows, order);
        self.homes = gather(&self.homes, order);
        self.berths = gather(&self.berths, order);
        self.hp = gather(&self.hp, order);
        self.motions = gather(&self.motions, order);
        self.reindex();
    }

    fn set_flight(&mut self, id: EntityId, at: usize, flight: Option<Flight>, now: Time) {
        match flight {
            Some(flight) => {
                self.flights.insert(id, flight);
                let homed = self.in_transit.entry(self.homes[at]).or_default();
                if let Err(place) = homed.binary_search(&id) {
                    homed.insert(place, id);
                }
            }
            None => {
                self.flights.remove(&id);
                self.forget_transit(id, self.homes[at]);
            }
        }
        self.berths[at] = Berth::of(self.homes[at], flight, now);
    }

    fn forget_transit(&mut self, id: EntityId, home: AsteroidId) {
        let Some(homed) = self.in_transit.get_mut(&home) else {
            return;
        };
        if let Ok(place) = homed.binary_search(&id) {
            homed.remove(place);
        }
        if homed.is_empty() {
            self.in_transit.remove(&home);
        }
    }

    fn place_of(&self, id: EntityId) -> usize {
        self.places_ascending[self
            .ids_ascending
            .binary_search(&id)
            .expect("an entity the state still holds")] as usize
    }

    fn place_for(&self, berth: Berth, seat: SeatId, id: EntityId) -> usize {
        let mut low = 0;
        let mut high = self.ids.len();
        while low < high {
            let middle = low + (high - low) / 2;
            match self.berths[middle]
                .cmp(&berth)
                .then(self.seats[middle].cmp(&seat))
                .then(self.ids[middle].cmp(&id))
                .is_lt()
            {
                true => low = middle + 1,
                false => high = middle,
            }
        }
        low
    }

    fn precedes(&self, at: usize, before: usize) -> bool {
        self.berths[at]
            .cmp(&self.berths[before])
            .then(self.seats[at].cmp(&self.seats[before]))
            .then(self.ids[at].cmp(&self.ids[before]))
            .is_lt()
    }

    fn reindex(&mut self) {
        let mut order: Vec<u32> = (0..self.ids.len() as u32).collect();
        order.sort_unstable_by_key(|at| self.ids[*at as usize]);
        self.ids_ascending = order.iter().map(|at| self.ids[*at as usize]).collect();
        self.places_ascending = order;
    }
}

impl<'a> Entity<'a> {
    pub fn id(self) -> EntityId {
        self.entities.ids[self.at]
    }

    pub fn seat(self) -> SeatId {
        self.entities.seats[self.at]
    }

    pub fn row(self) -> RowId {
        self.entities.rows[self.at]
    }

    pub fn home(self) -> AsteroidId {
        self.entities.homes[self.at]
    }

    pub fn berth(self) -> Berth {
        self.entities.berths[self.at]
    }

    pub fn standing(self) -> Option<AsteroidId> {
        self.berth().standing()
    }

    pub fn is_flying(self) -> bool {
        self.berth().flying_from().is_some()
    }

    pub(crate) fn hp(self) -> f64 {
        self.entities.hp[self.at].0
    }

    pub(crate) fn steered(self) -> Option<Body> {
        match self.entities.motions[self.at] {
            Motion::Fixed => None,
            Motion::Steered { body } => Some(body),
        }
    }

    pub(crate) fn flight(self) -> Option<Flight> {
        self.entities.flights.get(&self.id()).copied()
    }
}

fn gather<T: Copy>(column: &[T], order: &[u32]) -> Vec<T> {
    order.iter().map(|at| column[*at as usize]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::orbit::body::Gravity;
    use crate::roster::{FRIGATE, SHIPYARD};
    use crate::state::{Flight, Route, Send};

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: AsteroidId = AsteroidId(0);

    const AWAY: AsteroidId = AsteroidId(1);

    fn world() -> World {
        World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)])
    }

    fn standing(world: &World) -> Vec<Berth> {
        world.state.entities().map(Entity::berth).collect()
    }

    fn ids(world: &World) -> Vec<EntityId> {
        world.state.entities().map(Entity::id).collect()
    }

    fn sent(world: &mut World, unit: EntityId) -> Flight {
        let send = Send::joining(
            &world.state,
            Route {
                source: HOME,
                destination: AWAY,
                seat: SeatId(0),
            },
            &[unit],
        )
        .expect("a send across the ring");
        Flight::new(HOME, send.schedule)
    }

    #[test]
    fn the_store_reads_in_standing_then_seat_then_id_order() {
        let mut world = world();
        let last = world.hold(1, FRIGATE, AWAY, 0.0);
        let first = world.hold(0, FRIGATE, HOME, 0.0);
        let second = world.hold(1, FRIGATE, HOME, 2.0);
        let third = world.hold(1, FRIGATE, HOME, 4.0);

        assert_eq!(ids(&world), vec![first, second, third, last]);
        assert_eq!(
            world
                .state
                .entities
                .standing_at(HOME)
                .map(Entity::id)
                .collect::<Vec<EntityId>>(),
            vec![first, second, third],
            "an asteroid's units are one run"
        );
    }

    #[test]
    fn an_id_answers_the_same_entity_after_the_order_changes() {
        let mut world = world();
        let flier = world.hold(0, FRIGATE, HOME, 0.0);
        let stayer = world.hold(0, FRIGATE, HOME, 2.0);
        let flight = sent(&mut world, flier);
        world.launch(flier, HOME, 0.0, flight);
        while !world.state.entity(flier).is_flying() {
            world.state.advance();
        }

        assert_eq!(ids(&world), vec![stayer, flier], "the flier sorts last");
        assert_eq!(world.state.entity(flier).id(), flier);
        assert_eq!(world.state.entity(stayer).id(), stayer);
        assert_eq!(
            standing(&world),
            vec![Berth::Standing(HOME), Berth::Flying { from: HOME }]
        );
    }

    #[test]
    fn a_unit_whose_send_is_forming_is_homed_at_its_destination_and_stands_at_its_source() {
        let mut world = world();
        let leaving = world.hold(0, FRIGATE, HOME, 0.0);
        let flight = sent(&mut world, leaving);
        world.state.join(leaving, AWAY, flight);

        assert_eq!(world.state.entity(leaving).standing(), Some(HOME));
        assert_eq!(
            world
                .state
                .entities
                .homed_at(AWAY)
                .map(Entity::id)
                .collect::<Vec<EntityId>>(),
            vec![leaving]
        );
        assert_eq!(
            world.state.entities.homed_at(HOME).next().map(Entity::id),
            None
        );
    }

    #[test]
    fn reaping_the_dead_keeps_the_survivors_order_and_the_next_id() {
        let mut world = world();
        let first = world.hold(0, FRIGATE, HOME, 0.0);
        let doomed = world.hold(0, FRIGATE, HOME, 2.0);
        let last = world.hold(0, FRIGATE, HOME, 4.0);
        let hp = world.state.entity(doomed).hp();
        world.state.entities.hurt(doomed, hp);

        assert_eq!(world.state.reap(), vec![doomed]);
        assert_eq!(ids(&world), vec![first, last]);

        let next = world.fix(0, SHIPYARD, HOME);
        assert_eq!(next.0, last.0 + 1, "a dead id is never minted again");
    }
}
