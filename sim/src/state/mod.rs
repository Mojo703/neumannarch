//! The synced state and the ways it is read.

use core::ops::Index;
use std::collections::BTreeMap;

pub use attractor::Attractor;
pub use command::{
    Batch, Command, Issued, MAX_COMMANDS_PER_TICK, MAX_WANT, Refused, Rejected, Sequence, Stamped,
};
pub use entity::{Entity, Motion};
pub use flight::{Burn, Flight};
pub use frame::Frame;
pub use ready::Ready;
pub use rock::Rock;
pub use seat::Seat;
pub use sight::Sight;
pub use standings::Standings;
pub use view::View;
pub use wants::Wants;

use crate::ids::{EntityId, FlightId, RockId, RowId, SeatId};
use crate::materials::Materials;
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::place::{Place, Post};
use crate::roster::{Roster, Row};
use crate::state::sweep::Sweep;
use crate::time::{Moment, Tick};

/// Everything the match is, at one tick. Equal and hashed field by field,
/// so the desync hash covers every field without a hand edit. Every
/// collection is in id or key order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct State {
    tick: Tick,
    clock: Tick,
    /// The map's seed, which map generation lays the belt from. Hashed
    /// with every other field, so two machines that disagree on it desync
    /// at once.
    seed: u64,
    gravity: Gravity,
    seats: Vec<Seat>,
    roster: Roster,
    rocks: Vec<Rock>,
    entities: BTreeMap<EntityId, Entity>,
    next_entity: EntityId,
    flights: BTreeMap<FlightId, Flight>,
    next_flight: FlightId,
    wants: BTreeMap<Post, Wants>,
    frames: Vec<Frame>,
    ready: Vec<Ready>,
}

impl State {
    /// The start of a match: nothing on the map, ending at `clock`, over
    /// the map `seed` laid `rocks`.
    pub fn new(
        clock: Tick,
        seed: u64,
        gravity: Gravity,
        roster: Roster,
        rocks: Vec<Rock>,
        seats: Vec<Seat>,
    ) -> State {
        State {
            tick: Tick::ZERO,
            clock,
            seed,
            gravity,
            seats,
            roster,
            rocks,
            entities: BTreeMap::new(),
            next_entity: EntityId(0),
            flights: BTreeMap::new(),
            next_flight: FlightId(0),
            wants: BTreeMap::new(),
            frames: Vec::new(),
            ready: Vec::new(),
        }
    }

    /// The current tick.
    pub fn tick(&self) -> Tick {
        self.tick
    }

    /// The tick the match ends at.
    pub fn clock(&self) -> Tick {
        self.clock
    }

    /// The central mass's gravitational parameter.
    pub fn gravity(&self) -> Gravity {
        self.gravity
    }

    /// Every rock, in id order.
    pub fn rocks(&self) -> &[Rock] {
        &self.rocks
    }

    /// The rock `id` names, if the map has one.
    pub fn rock(&self, id: RockId) -> Option<&Rock> {
        self.rocks.get(id.0 as usize)
    }

    /// Every living entity, in id order.
    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    /// The entity `id` names, if it is alive.
    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// Every seat, in id order.
    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    /// The seat `id` names, if the match has one.
    pub fn seat(&self, id: SeatId) -> Option<&Seat> {
        self.seats.get(usize::from(id.0))
    }

    pub fn roster(&self) -> &Roster {
        &self.roster
    }

    /// The wants at `post`, if any row is wanted there.
    pub fn wants(&self, post: Post) -> Option<&Wants> {
        self.wants.get(&post)
    }

    /// Every post with a want, in key order.
    pub fn posts(&self) -> impl Iterator<Item = (Post, &Wants)> {
        self.wants.iter().map(|(post, wants)| (*post, wants))
    }

    /// Every entity whose home is `place`, any seat, in id order.
    pub fn entities_at(&self, place: Place) -> impl Iterator<Item = &Entity> {
        self.entities
            .values()
            .filter(move |entity| entity.home() == place)
    }

    /// Every entity homed at either band of `rock`, any seat, in id order.
    pub fn entities_at_rock(&self, rock: RockId) -> impl Iterator<Item = &Entity> {
        self.entities
            .values()
            .filter(move |entity| entity.home().rock == rock)
    }

    /// Every damage weapon's next ready moment, in the order they were
    /// created.
    pub fn ready(&self) -> &[Ready] {
        &self.ready
    }

    /// Every open frame, in the order opened.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// The open frames at `post`, in the order opened.
    pub fn frames_at(&self, post: Post) -> impl Iterator<Item = &Frame> {
        self.frames.iter().filter(move |frame| frame.post() == post)
    }

    /// How many of `row` the post's seat has at the post's place, in flight
    /// or holding.
    pub fn count(&self, post: Post, row: RowId) -> u32 {
        let mine = self
            .entities_at(post.place)
            .filter(|entity| entity.seat() == post.seat && entity.row() == row)
            .count();
        // Ids are `u32` and never reused, so fewer than `u32::MAX` entities
        // live; only an `EntityId`-keyed collection could say so in its type.
        u32::try_from(mine).unwrap_or(u32::MAX)
    }

    /// `rock`'s body at the current tick.
    pub fn rock_body(&self, rock: RockId) -> Body {
        self[rock].orbit().at(self.tick, self.gravity)
    }

    /// The anchor of `place`: its rock's orbit shifted ahead along that
    /// orbit by the band's amplitude. Panics for a rock the map lacks, as
    /// `Index<RockId>` does.
    pub fn anchor(&self, place: Place) -> Orbit {
        self[place.rock].orbit().shifted(place.band.amplitude())
    }

    /// Where `entity` is at the current tick: its rock's body when it is
    /// fixed, its own body when it is free.
    pub fn body_of(&self, entity: &Entity) -> Body {
        match entity.motion() {
            Motion::Fixed => self.rock_body(entity.home().rock),
            Motion::Free { body, .. } => body,
        }
    }

    /// The send `id` names, if it is still flying.
    pub fn flight(&self, id: FlightId) -> Option<&Flight> {
        self.flights.get(&id)
    }

    /// Every send in progress, in id order.
    pub fn flights(&self) -> impl Iterator<Item = (FlightId, &Flight)> {
        self.flights.iter().map(|(id, flight)| (*id, flight))
    }

    /// Every entity's position, for the range queries of one step.
    pub fn sweep(&self) -> Sweep {
        Sweep::build(
            self.entities()
                .map(|entity| (entity.id(), self.body_of(entity).pos)),
        )
    }

    /// The desync hash, over every field.
    pub fn hash(&self) -> u64 {
        hash::digest(self)
    }

    /// Adds an entity of `row` at full HP and returns its id, which no
    /// earlier entity of this match has had.
    pub(crate) fn spawn(
        &mut self,
        seat: SeatId,
        row: RowId,
        home: Place,
        motion: Motion,
    ) -> EntityId {
        let id = self.next_entity;
        self.next_entity = EntityId(id.0 + 1);
        let hp = self[row].hp.0;
        self.entities
            .insert(id, Entity::new(id, seat, row, home, hp, motion));
        let weapons: Vec<u8> = self[row].damage_weapons().collect();
        let now = Moment::at(self.tick);
        self.ready.extend(
            weapons
                .into_iter()
                .map(|weapon| Ready::new(id, weapon, now)),
        );
        id
    }

    /// Adds `flight` and returns its id, which no earlier flight of this
    /// match has had.
    pub(crate) fn add_flight(&mut self, flight: Flight) -> FlightId {
        let id = self.next_flight;
        self.next_flight = FlightId(id.0 + 1);
        self.flights.insert(id, flight);
        id
    }

    /// Replaces the motion of the entity `id` names; nothing when it is
    /// gone.
    pub(crate) fn set_motion(&mut self, id: EntityId, motion: Motion) {
        if let Some(entity) = self.entities.get_mut(&id) {
            entity.set_motion(motion);
        }
    }

    /// Advances the clock by one step.
    pub(crate) fn advance(&mut self) {
        self.tick = self.tick.next();
    }

    /// Removes the entity `id` names and its ready moments; nothing when it
    /// is already gone.
    pub(crate) fn remove_entity(&mut self, id: EntityId) {
        if self.entities.remove(&id).is_some() {
            self.ready.retain(|ready| ready.entity() != id);
        }
    }

    /// The seat `id` names, to spend from or eliminate.
    pub(crate) fn seat_mut(&mut self, id: SeatId) -> Option<&mut Seat> {
        self.seats.get_mut(usize::from(id.0))
    }

    /// The entity `id` names, to hurt, heal, re-home or mark.
    pub(crate) fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    /// Opens `frame`, which becomes the last of the frames.
    pub(crate) fn add_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    /// The frame at `at` in the order opened, to build.
    pub(crate) fn frame_mut(&mut self, at: usize) -> Option<&mut Frame> {
        self.frames.get_mut(at)
    }

    /// Closes the frames at `at`, in the order opened; indices the state
    /// does not have are ignored.
    pub(crate) fn close_frames(&mut self, at: &[usize]) {
        let mut closing: Vec<usize> = at.to_vec();
        closing.sort_unstable();
        closing.dedup();
        for at in closing.into_iter().rev() {
            if at < self.frames.len() {
                self.frames.remove(at);
            }
        }
    }

    /// Closes every frame of `seat`.
    pub(crate) fn close_seat_frames(&mut self, seat: SeatId) {
        self.frames.retain(|frame| frame.post().seat != seat);
    }

    /// Removes the send `id` names; nothing when it is already gone.
    pub(crate) fn remove_flight(&mut self, id: FlightId) {
        self.flights.remove(&id);
    }

    /// Removes the composition at `post`, wants and all.
    pub(crate) fn close_post(&mut self, post: Post) {
        self.wants.remove(&post);
    }

    /// Sets when one weapon of an entity is next ready; nothing when the
    /// state has no such weapon.
    pub(crate) fn set_ready(&mut self, entity: EntityId, weapon: u8, at: Moment) {
        if let Some(ready) = self
            .ready
            .iter_mut()
            .find(|ready| ready.entity() == entity && ready.weapon() == weapon)
        {
            ready.arm(at);
        }
    }

    /// Sets every seat's capacity to its base plus the capacity of its
    /// living entities, losing stock above it.
    pub(crate) fn refresh_capacities(&mut self) {
        let mut carried = vec![Materials::ZERO; self.seats.len()];
        for entity in self.entities.values() {
            if let Some(sum) = carried.get_mut(usize::from(entity.seat().0)) {
                *sum += self.roster[entity.row()].capacity;
            }
        }
        for (seat, carried) in self.seats.iter_mut().zip(carried) {
            let capacity = seat.base_capacity() + carried;
            seat.stockpile_mut().set_capacity(capacity);
        }
    }
}

impl Index<EntityId> for State {
    type Output = Entity;

    /// Panics for an entity that is not alive; a rule holding an id across
    /// a tick looks it up with `entity` instead.
    fn index(&self, id: EntityId) -> &Entity {
        self.entity(id)
            .unwrap_or_else(|| panic!("no living entity {id:?}"))
    }
}

impl Index<RockId> for State {
    type Output = Rock;

    fn index(&self, id: RockId) -> &Rock {
        &self.rocks[id.0 as usize]
    }
}

impl Index<RowId> for State {
    type Output = Row;

    fn index(&self, id: RowId) -> &Row {
        &self.roster[id]
    }
}

impl Index<SeatId> for State {
    type Output = Seat;

    fn index(&self, id: SeatId) -> &Seat {
        &self.seats[usize::from(id.0)]
    }
}

mod attractor;
mod command;
mod entity;
mod flight;
mod frame;
pub mod hash;
mod ready;
mod rock;
mod seat;
mod sight;
pub mod standings;
pub mod sweep;
pub mod view;
mod wants;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::TeamId;
    use crate::materials::Materials;
    use crate::orbit::elements::Orbit;
    use crate::place::Band;
    use crate::roster::Kind;
    use crate::vec3::Vec3;

    const SEAT: SeatId = SeatId(0);
    const ROCK: RockId = RockId(0);

    /// The central mass's gravitational parameter, in m³/s².
    const GRAVITY: Gravity = Gravity::new(4.0e13);

    /// A rock on the circular belt orbit at ten thousand kilometers, whose
    /// speed is the circular speed under [`GRAVITY`].
    fn rock() -> Rock {
        let body = Body::new(Vec3::new(1.0e7, 0.0, 0.0), Vec3::new(0.0, 0.0, -2.0e3));
        let orbit = Orbit::from_body(body, Tick::ZERO, GRAVITY).expect("a circular orbit");
        Rock::new(orbit, Materials::new(1.0, 1.0, 1.0), 100.0)
    }

    fn seat() -> Seat {
        Seat::new(
            TeamId(0),
            Materials::new(1.0e3, 1.0e3, 1.0e3),
            BTreeMap::new(),
        )
    }

    fn state() -> State {
        State::new(
            Tick(1000),
            0,
            GRAVITY,
            Roster::shipped(),
            vec![rock()],
            vec![seat(), seat()],
        )
    }

    fn row_of(state: &State, kind: Kind) -> RowId {
        state
            .roster()
            .iter()
            .find(|(_, row)| row.kind() == kind)
            .map(|(id, _)| id)
            .expect("the shipped roster has both kinds")
    }

    fn inner() -> Place {
        Place {
            rock: ROCK,
            band: Band::Inner,
        }
    }

    #[test]
    fn count_is_the_seat_entities_of_the_row_at_the_place() {
        let mut state = state();
        let structure = row_of(&state, Kind::Structure);
        let unit = row_of(&state, Kind::Unit);
        let post = Post {
            place: inner(),
            seat: SEAT,
        };
        let first = state.spawn(SEAT, structure, inner(), Motion::Fixed);
        let second = state.spawn(SEAT, structure, inner(), Motion::Fixed);
        state.spawn(SeatId(1), structure, inner(), Motion::Fixed);
        assert_ne!(first, second);
        assert_eq!(state.count(post, structure), 2);
        assert_eq!(state.count(post, unit), 0);
        assert_eq!(state.entities_at(inner()).count(), 3);
        assert_eq!(state[first].hp(), state[structure].hp.0);
        state.remove_entity(first);
        assert_eq!(state.count(post, structure), 1);
        assert_eq!(state.entity(first), None);
        assert_eq!(state[second].id(), second);
    }

    #[test]
    fn a_fixed_entity_has_its_rocks_body_and_a_free_one_its_own() {
        let mut state = state();
        let structure = row_of(&state, Kind::Structure);
        let unit = row_of(&state, Kind::Unit);
        let body = Body::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0));
        let fixed = state.spawn(SEAT, structure, inner(), Motion::Fixed);
        let free = state.spawn(SEAT, unit, inner(), Motion::Free { body, flight: None });
        assert_eq!(state.body_of(&state[fixed]), state.rock_body(ROCK));
        assert_eq!(state.body_of(&state[free]), body);
        assert!(!state[free].is_flying());
        assert_eq!(state[free].flight(), None);
    }

    #[test]
    fn a_rock_at_its_epoch_tick_is_at_the_body_it_came_from() {
        let state = state();
        assert_eq!(state.tick(), Tick::ZERO);
        let body = state.rock_body(ROCK);
        assert!(
            body.pos.distance(Vec3::new(1.0e7, 0.0, 0.0)) < 1.0,
            "{body:?}"
        );
        assert!(
            body.vel.distance(Vec3::new(0.0, 0.0, -2.0e3)) < 1e-6,
            "{body:?}"
        );
    }
}
