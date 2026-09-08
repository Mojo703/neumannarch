use core::ops::{Index, IndexMut};
use std::collections::BTreeMap;

pub use asteroid::Asteroid;
pub use command::{
    Batch, Command, Issued, MAX_COMMANDS_PER_TICK, MAX_WANT, Refused, Rejected, Sequence, Stamped,
};
pub use draft::{Draft, GRACE, STAGE_SPAN, STAGES_PER_SEAT, Stage};
pub(crate) use entity::{Entity, Motion};
pub(crate) use frame::Frame;
pub use preview::{Preview, ShortfallFilling};
pub(crate) use ready::Ready;
pub(crate) use schedule::{Flight, Schedule};
pub use seat::Seat;
pub use send::{Route, Send};
pub use standings::Standings;
pub(crate) use threat::{Aim, Assigned, Threat};
pub(crate) use wants::Wants;

use crate::TICKS_PER_SECOND;
use crate::belt::Belt;
use crate::ids::{AsteroidId, EntityId, RowId, SeatId};
use crate::materials::Materials;
use crate::orbit::body::{Body, Gravity};
use crate::post::Post;
use crate::posting::Posting;
use crate::roster::{Kind, Roster, Row};
use crate::state::sweep::Sweep;
use crate::time::{Moment, Tick, Time};
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Held {
    pub present: u32,
    pub surplus: u32,
    pub leaving: u32,
    pub arriving: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct State {
    tick: Tick,
    length: Time,
    draft: Draft,
    seed: u64,
    gravity: Gravity,
    seats: Vec<Seat>,
    roster: Roster,
    asteroids: Vec<Asteroid>,
    entities: BTreeMap<EntityId, Entity>,
    next_entity: EntityId,
    wants: BTreeMap<Post, Wants>,
    frames: Vec<Frame>,
    ready: Vec<Ready>,
}

impl State {
    pub fn new(
        length: Time,
        seed: u64,
        gravity: Gravity,
        roster: Roster,
        asteroids: Vec<Asteroid>,
        seats: Vec<Seat>,
    ) -> State {
        State {
            tick: Tick::ZERO,
            length,
            draft: Draft::of(seed, &seats, &roster),
            seed,
            gravity,
            seats,
            roster,
            asteroids,
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

    pub fn time(&self) -> Time {
        Time(self.tick.0).since(Time(self.draft.ended().unwrap_or(self.tick).0))
    }

    pub fn length(&self) -> Time {
        self.length
    }

    pub fn draft(&self) -> &Draft {
        &self.draft
    }

    pub fn drafting(&self) -> bool {
        self.draft.ended().is_none()
    }

    pub(crate) fn close_draft(&mut self) {
        if !self.drafting() {
            return;
        }
        self.draft.pass(self.tick);
        if self.draft.over(self.tick) {
            self.draft.end(self.tick);
        }
    }

    pub fn gravity(&self) -> Gravity {
        self.gravity
    }

    pub fn asteroids(&self) -> impl Iterator<Item = (AsteroidId, &Asteroid)> {
        (0..u32::MAX).map(AsteroidId).zip(&self.asteroids)
    }

    pub fn asteroid(&self, id: AsteroidId) -> Option<&Asteroid> {
        self.asteroids.get(id.0 as usize)
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

    pub(crate) fn seat(&self, id: SeatId) -> Option<&Seat> {
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

    pub fn entities_at(&self, asteroid: AsteroidId) -> impl Iterator<Item = &Entity> {
        self.entities
            .values()
            .filter(move |entity| entity.home() == asteroid)
    }

    pub fn standing_at(&self, asteroid: AsteroidId) -> impl Iterator<Item = &Entity> {
        self.entities
            .values()
            .filter(move |entity| entity.standing(self.time()) == Some(asteroid))
    }

    pub fn is_taken(&self, asteroid: AsteroidId) -> bool {
        self.entities_at(asteroid).next().is_some()
    }

    pub fn held_by(&self, seat: SeatId) -> impl Iterator<Item = AsteroidId> {
        self.deduped(
            self.of_seat(seat)
                .filter(|entity| self[entity.row()].kind() == Kind::Structure)
                .filter_map(|entity| entity.standing(self.time())),
        )
    }

    pub fn occupied_by(&self, seat: SeatId) -> impl Iterator<Item = AsteroidId> {
        self.deduped(self.of_seat(seat).map(Entity::home))
    }

    fn of_seat(&self, seat: SeatId) -> impl Iterator<Item = &Entity> {
        self.entities().filter(move |entity| entity.seat() == seat)
    }

    fn deduped(
        &self,
        asteroids: impl Iterator<Item = AsteroidId>,
    ) -> impl Iterator<Item = AsteroidId> {
        let mut asteroids: Vec<AsteroidId> = asteroids.collect();
        asteroids.sort_unstable();
        asteroids.dedup();
        asteroids.into_iter()
    }

    pub(crate) fn ready(&self) -> &[Ready] {
        &self.ready
    }

    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    pub fn frames_at(&self, post: Post) -> impl Iterator<Item = &Frame> {
        self.frames.iter().filter(move |frame| frame.post() == post)
    }

    pub fn frames_of(&self, post: Post, row: RowId) -> impl Iterator<Item = (usize, &Frame)> {
        self.frames
            .iter()
            .enumerate()
            .filter(move |(_, frame)| frame.post() == post && frame.row() == row)
    }

    pub fn count(&self, post: Post, row: RowId) -> u32 {
        let counted = self
            .entities_at(post.asteroid)
            .filter(|entity| entity.seat() == post.seat && entity.row() == row)
            .count();
        u32::try_from(counted).unwrap_or(u32::MAX)
    }

    pub fn surplus_at(&self, posting: Posting) -> Vec<EntityId> {
        let (post, row) = (posting.post(), posting.row());
        let covered = self
            .wants(post)
            .map_or(0, |wants| wants.get(row))
            .saturating_sub(self.frames_of(post, row).count() as u32);
        let mut held: Vec<EntityId> = self
            .entities_at(post.asteroid)
            .filter(|entity| entity.seat() == post.seat && entity.row() == row)
            .filter(|entity| entity.flight().is_none())
            .map(Entity::id)
            .collect();
        held.sort_unstable_by(|a, b| b.cmp(a));
        held.truncate(self.count(post, row).saturating_sub(covered) as usize);
        held
    }

    pub fn holdings(&self) -> BTreeMap<Posting, Held> {
        let mut holdings: BTreeMap<Posting, Held> = BTreeMap::new();
        for entity in self.entities.values() {
            let at = |asteroid: AsteroidId| Posting::of(asteroid, entity.seat(), entity.row());
            match (entity.standing(self.time()), entity.flight()) {
                (Some(asteroid), None) => holdings.entry(at(asteroid)).or_default().present += 1,
                (Some(asteroid), Some(_)) => holdings.entry(at(asteroid)).or_default().leaving += 1,
                (None, _) => {}
            }
            if entity.flight().is_some() {
                holdings.entry(at(entity.home())).or_default().arriving += 1;
            }
        }
        holdings
    }

    pub fn asteroid_body(&self, asteroid: AsteroidId) -> Body {
        self[asteroid].orbit().at(self.time(), self.gravity)
    }

    pub fn body_of(&self, entity: &Entity) -> Body {
        match entity.motion() {
            Motion::Fixed => self.asteroid_body(entity.home()),
            Motion::Steered { body, .. } => body,
        }
    }

    pub(crate) fn sweep(&self) -> Sweep {
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
        home: AsteroidId,
        motion: Motion,
    ) -> EntityId {
        let id = self.next_entity;
        self.next_entity = EntityId(id.0 + 1);
        let hp = self[row].hp.0;
        self.entities
            .insert(id, Entity::new(id, seat, row, home, hp, motion));
        let weapons: Vec<u8> = self[row].damage_weapons().collect();
        let now = Moment::at(self.time());
        self.ready.extend(
            weapons
                .into_iter()
                .map(|weapon| Ready::new(id, weapon, now)),
        );
        id
    }

    pub(crate) fn place_from_reserve(&mut self, post: Post, row: RowId) -> bool {
        let taken = self
            .seat_mut(post.seat)
            .is_some_and(|seat| seat.take_reserved(row));
        if taken {
            self.spawn_at(post, row);
        }
        taken
    }

    pub(crate) fn spawn_at(&mut self, post: Post, row: RowId) {
        let motion = match self[row].kind() {
            Kind::Structure => Motion::Fixed,
            Kind::Unit => Motion::Steered {
                body: self.spawn_body(post.asteroid, self.time().next()),
                flight: None,
            },
        };
        self.spawn(post.seat, row, post.asteroid, motion);
    }

    pub(crate) fn spawn_body(&self, asteroid: AsteroidId, at: Time) -> Body {
        let home = self[asteroid].orbit().at(at, self.gravity());
        let already = self
            .standing_at(asteroid)
            .filter(|entity| entity.motion() != Motion::Fixed)
            .count();
        let radial = home.pos.normalized().unwrap_or(Vec3::ZERO);
        let floor = self[asteroid].radius() + Belt::SPACING_METERS;
        Body::new(
            home.pos + radial * (floor + Belt::SPACING_METERS * already as f64),
            home.vel,
        )
    }

    pub(crate) fn set_motion(&mut self, id: EntityId, motion: Motion) {
        if let Some(entity) = self.entities.get_mut(&id) {
            entity.set_motion(motion);
        }
    }

    pub(crate) fn advance(&mut self) {
        self.close_draft();
        self.tick = self.tick.next();
        if self.time().0.is_multiple_of(u64::from(TICKS_PER_SECOND)) {
            self.seats.iter_mut().for_each(Seat::close_second);
            self.asteroids.iter_mut().for_each(Asteroid::close_second);
        }
    }

    pub fn remove_entity(&mut self, id: EntityId) {
        if self.entities.remove(&id).is_some() {
            self.ready.retain(|ready| ready.entity() != id);
        }
    }

    pub(crate) fn seat_mut(&mut self, id: SeatId) -> Option<&mut Seat> {
        self.seats.get_mut(usize::from(id.0))
    }

    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
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

    pub fn close_post(&mut self, post: Post) {
        self.wants.remove(&post);
    }

    pub fn set_ready(&mut self, entity: EntityId, weapon: u8, at: Moment) {
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

impl Index<AsteroidId> for State {
    type Output = Asteroid;

    fn index(&self, id: AsteroidId) -> &Asteroid {
        &self.asteroids[id.0 as usize]
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

impl IndexMut<AsteroidId> for State {
    fn index_mut(&mut self, id: AsteroidId) -> &mut Asteroid {
        &mut self.asteroids[id.0 as usize]
    }
}

impl IndexMut<SeatId> for State {
    fn index_mut(&mut self, id: SeatId) -> &mut Seat {
        &mut self.seats[usize::from(id.0)]
    }
}

mod asteroid;
mod command;
mod draft;
mod entity;
mod frame;
pub(crate) mod hash;
mod preview;
mod ready;
mod schedule;
mod seat;
mod send;
pub mod standings;
pub(crate) mod sweep;
mod threat;
pub mod view;
mod wants;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::roster::Kind;
    use crate::vec3::Vec3;

    const ASTEROID: AsteroidId = AsteroidId(0);

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    fn world() -> World {
        World::ring(GRAVITY, 1, &[TeamId(0), TeamId(0)])
    }

    fn row_of(state: &State, kind: Kind) -> RowId {
        state
            .roster()
            .iter()
            .find(|(_, row)| row.kind() == kind)
            .map(|(id, _)| id)
            .expect("the shipped roster has both kinds")
    }

    #[test]
    fn held_by_names_the_asteroids_of_a_seats_structures_and_occupied_by_all_it_is_homed_at() {
        let mut world = World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)]);
        let structure = row_of(&world.state, Kind::Structure);
        let unit = row_of(&world.state, Kind::Unit);
        let away = AsteroidId(1);
        let body = world.state.spawn_body(away, world.state.time());
        world.fix(0, structure, ASTEROID);
        world.fix(0, structure, ASTEROID);
        world.free(0, unit, away, body);

        assert_eq!(
            world.state.held_by(SeatId(0)).collect::<Vec<AsteroidId>>(),
            vec![ASTEROID],
            "a unit stands where no structure of the seat does"
        );
        assert_eq!(
            world
                .state
                .occupied_by(SeatId(0))
                .collect::<Vec<AsteroidId>>(),
            vec![ASTEROID, away]
        );
        assert_eq!(world.state.occupied_by(SeatId(1)).next(), None);
        assert!(world.state.is_taken(away), "the unit is homed there");
    }

    #[test]
    fn count_is_the_seat_entities_of_the_row_at_the_asteroid() {
        let mut world = world();
        let structure = row_of(&world.state, Kind::Structure);
        let unit = row_of(&world.state, Kind::Unit);
        let first = world.fix(0, structure, ASTEROID);
        let second = world.fix(0, structure, ASTEROID);
        world.fix(1, structure, ASTEROID);
        assert_ne!(first, second);
        assert_eq!(world.count(0, ASTEROID, structure), 2);
        assert_eq!(world.count(0, ASTEROID, unit), 0);
        assert_eq!(world.state.entities_at(ASTEROID).count(), 3);
        assert_eq!(world.state[first].hp(), world.state[structure].hp.0);
        world.state.remove_entity(first);
        assert_eq!(world.count(0, ASTEROID, structure), 1);
        assert_eq!(world.state.entity(first), None);
        assert_eq!(world.state[second].id(), second);
    }

    #[test]
    fn a_fixed_entity_has_its_asteroids_body_and_a_free_one_its_own() {
        let mut world = world();
        let structure = row_of(&world.state, Kind::Structure);
        let unit = row_of(&world.state, Kind::Unit);
        let body = Body::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0));
        let fixed = world.fix(0, structure, ASTEROID);
        let free = world.free(0, unit, ASTEROID, body);
        assert_eq!(world.body(fixed), world.state.asteroid_body(ASTEROID));
        assert_eq!(world.body(free), body);
    }

    #[test]
    fn a_asteroid_at_its_epoch_tick_is_at_the_body_it_came_from() {
        let world = world();
        assert_eq!(world.state.tick(), Tick::ZERO);
        let body = world.state.asteroid_body(ASTEROID);
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
