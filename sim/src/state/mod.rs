use core::ops::Index;
use std::collections::BTreeMap;

pub use attractor::Attractor;
pub use command::{
    Batch, Command, Issued, MAX_COMMANDS_PER_TICK, MAX_WANT, Refused, Rejected, Sequence, Stamped,
};
pub use entity::{Entity, Motion};
pub use frame::Frame;
pub use radar::Radar;
pub use ready::Ready;
pub use rock::Rock;
pub use schedule::{Flight, Schedule};
pub use seat::Seat;
pub use send::Send;
pub use sight::Sight;
pub use standings::Standings;
pub use view::View;
pub use wants::Wants;

use crate::ids::{EntityId, RockId, RowId, SeatId};
use crate::materials::Materials;
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::place::{Place, Post};
use crate::roster::{Roster, Row};
use crate::state::sweep::Sweep;
use crate::time::{Moment, Tick};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Held {
    pub present: u32,
    pub flying: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct State {
    tick: Tick,
    clock: Tick,
    seed: u64,
    gravity: Gravity,
    seats: Vec<Seat>,
    roster: Roster,
    rocks: Vec<Rock>,
    entities: BTreeMap<EntityId, Entity>,
    next_entity: EntityId,
    wants: BTreeMap<Post, Wants>,
    frames: Vec<Frame>,
    ready: Vec<Ready>,
}

impl State {
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
            wants: BTreeMap::new(),
            frames: Vec::new(),
            ready: Vec::new(),
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn clock(&self) -> Tick {
        self.clock
    }

    pub fn gravity(&self) -> Gravity {
        self.gravity
    }

    pub fn rocks(&self) -> &[Rock] {
        &self.rocks
    }

    pub fn rock(&self, id: RockId) -> Option<&Rock> {
        self.rocks.get(id.0 as usize)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    pub fn seat(&self, id: SeatId) -> Option<&Seat> {
        self.seats.get(usize::from(id.0))
    }

    pub fn roster(&self) -> &Roster {
        &self.roster
    }

    pub fn wants(&self, post: Post) -> Option<&Wants> {
        self.wants.get(&post)
    }

    pub fn posts(&self) -> impl Iterator<Item = (Post, &Wants)> {
        self.wants.iter().map(|(post, wants)| (*post, wants))
    }

    pub fn entities_at(&self, place: Place) -> impl Iterator<Item = &Entity> {
        self.entities
            .values()
            .filter(move |entity| entity.home() == place)
    }

    pub fn entities_at_rock(&self, rock: RockId) -> impl Iterator<Item = &Entity> {
        self.entities
            .values()
            .filter(move |entity| entity.home().rock == rock)
    }

    pub fn ready(&self) -> &[Ready] {
        &self.ready
    }

    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    pub fn frames_at(&self, post: Post) -> impl Iterator<Item = &Frame> {
        self.frames.iter().filter(move |frame| frame.post() == post)
    }

    pub fn count(&self, post: Post, row: RowId) -> u32 {
        let held = self.holding(post, row);
        held.present + held.flying
    }

    pub fn holding(&self, post: Post, row: RowId) -> Held {
        let mine = || {
            self.entities_at(post.place)
                .filter(move |entity| entity.seat() == post.seat && entity.row() == row)
        };

        let counted = |count: usize| u32::try_from(count).unwrap_or(u32::MAX);
        Held {
            present: counted(mine().filter(|entity| !entity.is_flying()).count()),
            flying: counted(mine().filter(|entity| entity.is_flying()).count()),
        }
    }

    pub fn rock_body(&self, rock: RockId) -> Body {
        self[rock].orbit().at(self.tick, self.gravity)
    }

    pub fn anchor(&self, place: Place) -> Orbit {
        self[place.rock].orbit().shifted(place.band.amplitude())
    }

    pub fn body_of(&self, entity: &Entity) -> Body {
        match entity.motion() {
            Motion::Fixed => self.rock_body(entity.home().rock),
            Motion::Free { body, .. } => body,
        }
    }

    pub fn sweep(&self) -> Sweep {
        Sweep::build(
            self.entities()
                .map(|entity| (entity.id(), self.body_of(entity).pos)),
        )
    }

    pub fn hash(&self) -> u64 {
        hash::digest(self)
    }

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

    pub(crate) fn set_motion(&mut self, id: EntityId, motion: Motion) {
        if let Some(entity) = self.entities.get_mut(&id) {
            entity.set_motion(motion);
        }
    }

    pub(crate) fn advance(&mut self) {
        self.tick = self.tick.next();
    }

    pub(crate) fn remove_entity(&mut self, id: EntityId) {
        if self.entities.remove(&id).is_some() {
            self.ready.retain(|ready| ready.entity() != id);
        }
    }

    pub(crate) fn seat_mut(&mut self, id: SeatId) -> Option<&mut Seat> {
        self.seats.get_mut(usize::from(id.0))
    }

    pub(crate) fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    pub(crate) fn add_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    pub(crate) fn frame_mut(&mut self, at: usize) -> Option<&mut Frame> {
        self.frames.get_mut(at)
    }

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

    pub(crate) fn close_seat_frames(&mut self, seat: SeatId) {
        self.frames.retain(|frame| frame.post().seat != seat);
    }

    pub(crate) fn close_post(&mut self, post: Post) {
        self.wants.remove(&post);
    }

    pub(crate) fn set_ready(&mut self, entity: EntityId, weapon: u8, at: Moment) {
        if let Some(ready) = self
            .ready
            .iter_mut()
            .find(|ready| ready.entity() == entity && ready.weapon() == weapon)
        {
            ready.arm(at);
        }
    }

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
mod frame;
pub mod hash;
mod radar;
mod ready;
mod rock;
mod schedule;
mod seat;
mod send;
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

    const GRAVITY: Gravity = Gravity::new(4.0e13);

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
