use core::ops::{Index, IndexMut};
use std::collections::BTreeMap;

pub use asteroid::Asteroid;
pub use command::{
    Batch, Command, Issued, MAX_COMMANDS_PER_TICK, MAX_WANT, Refused, Rejected, Sequence, Stamped,
};
pub use draft::{Draft, GRACE, PlacementStage, STAGE_SPAN, STAGES_PER_SEAT};
pub use entities::Berth;
pub(crate) use entities::{Entities, Entity, Motion, Pass};
pub(crate) use frame::Frame;
pub use preview::{Preview, ShortfallFilling};
pub(crate) use ready::{Ready, ReadyDamage};
pub(crate) use roll::{Roll, Rolls};
pub use seat::Seat;
pub use stage::{FightStage, Line};
pub use standings::Standings;
pub(crate) use threat::{AssignedDamage, Reach, Shooter};
pub(crate) use wants::Wants;

use crate::TICKS_PER_SECOND;
use crate::belt::Belt;
use crate::ids::{AsteroidId, EntityId, SeatId};
use crate::materials::Materials;
use crate::orbit::body::{Body, Gravity};
use crate::pattern::{EntityPattern, Kind, Slot};
use crate::post::Post;
use crate::posting::Posting;
use crate::time::{Moment, Tick, Time};
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Held {
    pub present: u32,
    pub surplus: u32,
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
    asteroids: Vec<Asteroid>,
    pub(crate) entities: Entities,
    wants: BTreeMap<Post, Wants>,
    frames: Vec<Frame>,
    ready: ReadyDamage,
}

impl State {
    pub fn new(
        length: Time,
        seed: u64,
        gravity: Gravity,
        asteroids: Vec<Asteroid>,
        seats: Vec<Seat>,
    ) -> State {
        State {
            tick: Tick::ZERO,
            length,
            draft: Draft::of(seed, &seats),
            seed,
            gravity,
            seats,
            asteroids,
            entities: Entities::empty(),
            wants: BTreeMap::new(),
            frames: Vec::new(),
            ready: ReadyDamage::default(),
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn time(&self) -> Time {
        self.tick.since(self.draft.ended().unwrap_or(self.tick))
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

    pub(crate) fn ran(&self) -> Time {
        match self.drafting() {
            true => Time::ZERO,
            false => Time(1),
        }
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

    pub fn entities(&self) -> impl Iterator<Item = Entity<'_>> {
        self.entities.iter()
    }

    pub fn entity(&self, id: EntityId) -> Entity<'_> {
        self.entities.entity(id)
    }

    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    pub(crate) fn seat(&self, id: SeatId) -> Option<&Seat> {
        self.seats.get(usize::from(id.0))
    }

    pub fn wants(&self, post: Post) -> Option<&Wants> {
        self.wants.get(&post)
    }

    pub fn posts(&self) -> impl Iterator<Item = (Post, &Wants)> {
        self.wants.iter().map(|(post, wants)| (*post, wants))
    }

    pub fn is_taken(&self, asteroid: AsteroidId) -> bool {
        self.entities.homed_at(asteroid).next().is_some()
    }

    pub fn held_by(&self, seat: SeatId) -> impl Iterator<Item = AsteroidId> {
        self.deduped(
            self.of_seat(seat)
                .filter(|entity| entity.pattern().kind() == Kind::Structure)
                .filter_map(Entity::standing),
        )
    }

    pub fn occupied_by(&self, seat: SeatId) -> impl Iterator<Item = AsteroidId> {
        self.deduped(self.of_seat(seat).map(Entity::home))
    }

    fn of_seat(&self, seat: SeatId) -> impl Iterator<Item = Entity<'_>> {
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

    pub(crate) fn ready(&self) -> impl Iterator<Item = &Ready> {
        self.ready.iter()
    }

    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    pub fn frames_at(&self, post: Post) -> impl Iterator<Item = &Frame> {
        self.frames.iter().filter(move |frame| frame.post() == post)
    }

    pub fn frames_of(
        &self,
        post: Post,
        pattern: EntityPattern,
    ) -> impl Iterator<Item = (usize, &Frame)> {
        self.frames
            .iter()
            .enumerate()
            .filter(move |(_, frame)| frame.post() == post && frame.pattern() == pattern)
    }

    pub(crate) fn homed(&self) -> BTreeMap<Posting, u32> {
        let mut homed: BTreeMap<Posting, u32> = BTreeMap::new();
        for entity in self.entities() {
            *homed
                .entry(Posting::of(entity.home(), entity.seat(), entity.pattern()))
                .or_default() += 1;
        }
        homed
    }

    pub(crate) fn shortfalls(&self) -> BTreeMap<Posting, u32> {
        let homed = self.homed();
        self.posts()
            .flat_map(|(post, wants)| {
                let homed = &homed;
                wants.iter().filter_map(move |(pattern, want)| {
                    let posting = Posting::new(post, pattern);
                    want.checked_sub(homed.get(&posting).copied().unwrap_or_default())
                        .filter(|missing| *missing > 0)
                        .map(|missing| (posting, missing))
                })
            })
            .collect()
    }

    pub fn count(&self, post: Post, pattern: EntityPattern) -> u32 {
        let counted = self.posted(post, pattern).count();
        u32::try_from(counted).unwrap_or(u32::MAX)
    }

    pub fn surplus_at(&self, posting: Posting) -> Vec<EntityId> {
        let (post, pattern) = (posting.post(), posting.pattern());
        let covered = self
            .wants(post)
            .map_or(0, |wants| wants.get(pattern))
            .saturating_sub(self.frames_of(post, pattern).count() as u32);
        let mut posted: usize = 0;
        let mut held: Vec<EntityId> = Vec::new();
        for entity in self.posted(post, pattern) {
            posted += 1;
            if entity.standing().is_some() {
                held.push(entity.id());
            }
        }
        held.sort_unstable_by(|first, second| second.cmp(first));
        held.truncate(posted.saturating_sub(covered as usize));
        held
    }

    fn posted(&self, post: Post, pattern: EntityPattern) -> impl Iterator<Item = Entity<'_>> {
        self.entities
            .homed_at(post.asteroid)
            .filter(move |entity| entity.seat() == post.seat && entity.pattern() == pattern)
    }

    pub fn holdings(&self) -> BTreeMap<Posting, Held> {
        let mut holdings: BTreeMap<Posting, Held> = BTreeMap::new();
        for entity in self.entities.iter() {
            let at = |asteroid: AsteroidId| Posting::of(asteroid, entity.seat(), entity.pattern());
            match entity.standing() {
                Some(asteroid) => holdings.entry(at(asteroid)).or_default().present += 1,
                None => holdings.entry(at(entity.home())).or_default().arriving += 1,
            }
        }
        holdings
    }

    pub fn asteroid_body(&self, asteroid: AsteroidId) -> Body {
        self[asteroid].orbit().at(self.time(), self.gravity)
    }

    pub fn body_of(&self, entity: Entity) -> Body {
        entity
            .steered()
            .unwrap_or_else(|| self.asteroid_body(entity.home()))
    }

    pub fn hash(&self) -> u64 {
        hash::digest(self)
    }

    pub(crate) fn spawn(
        &mut self,
        seat: SeatId,
        pattern: EntityPattern,
        home: AsteroidId,
        motion: Motion,
    ) -> EntityId {
        let id = self.entities.spawn(seat, pattern, home, motion);
        let guns = pattern.hitscans().map(|(slot, _)| slot);
        self.ready.ready_from(id, guns, Moment::at(self.time()));
        id
    }

    pub(crate) fn place_from_reserve(&mut self, post: Post, pattern: EntityPattern, ran: Time) {
        if self
            .seat_mut(post.seat)
            .is_some_and(|seat| seat.take_reserved(pattern))
        {
            self.draft
                .place(post.asteroid, post.seat, pattern, self.tick);
            self.spawn_at(post, pattern, ran);
        }
    }

    pub(crate) fn spawn_at(&mut self, post: Post, pattern: EntityPattern, ran: Time) {
        let motion = match pattern.kind() {
            Kind::Structure => Motion::Fixed,
            Kind::Unit => Motion::Steered {
                body: self.spawn_body(post.asteroid, self.time().after(ran)),
            },
        };
        self.spawn(post.seat, pattern, post.asteroid, motion);
    }

    pub(crate) fn spawn_built(&mut self, frame: &Frame, ran: Time) {
        let post = frame.post();
        let built_at = frame.built_at();
        if !frame.built_elsewhere() {
            self.spawn_at(post, frame.pattern(), ran);
            return;
        }
        let motion = Motion::Steered {
            body: self.spawn_body(built_at, self.time().after(ran)),
        };
        let id = self.spawn(post.seat, frame.pattern(), built_at, motion);
        self.re_home(&BTreeMap::from([(id, post.asteroid)]));
    }

    pub(crate) fn spawn_body(&self, asteroid: AsteroidId, at: Time) -> Body {
        let home = self[asteroid].orbit().at(at, self.gravity());
        let already = self
            .entities
            .iter()
            .filter(|entity| entity.standing() == Some(asteroid) && entity.steered().is_some())
            .count();
        let radial = home.pos.normalized().unwrap_or(Vec3::ZERO);
        let floor = self[asteroid].radius() + Belt::SPACING_METERS;
        Body::new(
            home.pos + radial * (floor + Belt::SPACING_METERS * already as f64),
            home.vel,
        )
    }

    pub(crate) fn steer(&mut self, id: EntityId, body: Body) {
        self.entities.steer(id, body);
    }

    pub(crate) fn re_home(&mut self, sent: &BTreeMap<EntityId, AsteroidId>) {
        self.entities.re_home(sent);
    }

    pub(crate) fn arrive(&mut self, arrived: &[EntityId]) {
        self.entities.arrive(arrived);
    }

    pub(crate) fn reap(&mut self) -> Vec<EntityId> {
        let dead = self.entities.reap();
        self.ready.reap(&dead);
        dead
    }

    pub(crate) fn advance(&mut self) {
        self.close_draft();
        self.tick = self.tick.next();
        if self.time().0.is_multiple_of(u64::from(TICKS_PER_SECOND)) {
            self.seats.iter_mut().for_each(Seat::close_second);
            self.asteroids.iter_mut().for_each(Asteroid::close_second);
        }
    }

    pub(crate) fn seat_mut(&mut self, id: SeatId) -> Option<&mut Seat> {
        self.seats.get_mut(usize::from(id.0))
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

    pub(crate) fn set_ready(
        &mut self,
        entity: EntityId,
        slot: Slot,
        at: Moment,
        kept: Option<EntityId>,
    ) {
        self.ready.ready_again(entity, slot, at, kept);
    }

    pub(crate) fn refresh_capacities(&mut self) {
        let mut carried = vec![Materials::ZERO; self.seats.len()];
        for entity in self.entities.iter() {
            if let Some(sum) = carried.get_mut(usize::from(entity.seat().0)) {
                *sum += entity.pattern().capacity();
            }
        }
        for (seat, carried) in self.seats.iter_mut().zip(carried) {
            let capacity = seat.base_capacity() + carried;
            seat.stockpile_mut().set_capacity(capacity);
        }
    }
}

impl Index<AsteroidId> for State {
    type Output = Asteroid;

    fn index(&self, id: AsteroidId) -> &Asteroid {
        &self.asteroids[id.0 as usize]
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
mod circle;
mod command;
mod draft;
mod entities;
mod frame;
pub(crate) mod hash;
mod preview;
mod ready;
mod roll;
mod seat;
pub mod stage;
pub mod standings;
mod threat;
pub mod view;
mod wants;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::pattern::EntityPattern as P;
    use crate::vec3::Vec3;

    const ASTEROID: AsteroidId = AsteroidId(0);

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    fn world() -> World {
        World::ring(GRAVITY, 1, &[TeamId(0), TeamId(0)])
    }

    #[test]
    fn reaping_the_dead_takes_every_dead_units_damage_places_whatever_order_they_stand_in() {
        let mut world = World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)]);
        let raider = P::Raider;
        let later_asteroid_first = world.hold(0, raider, AsteroidId(1), 0.0);
        let earlier_asteroid_second = world.hold(0, raider, AsteroidId(0), 0.0);
        for unit in [later_asteroid_first, earlier_asteroid_second] {
            world.state.entities.hurt(unit, f64::MAX);
        }

        world.state.reap();

        assert!(
            world.state.ready().next().is_none(),
            "a dead unit kept a damage place: {:?}",
            world.state.ready().map(Ready::entity).collect::<Vec<_>>()
        );
    }

    #[test]
    fn held_by_names_the_asteroids_of_a_seats_structures_and_occupied_by_all_it_is_homed_at() {
        let mut world = World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)]);
        let structure = P::Storage;
        let unit = P::Raider;
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
    fn count_is_the_seat_entities_of_the_pattern_at_the_asteroid() {
        let mut world = world();
        let structure = P::Storage;
        let unit = P::Raider;
        let first = world.fix(0, structure, ASTEROID);
        let second = world.fix(0, structure, ASTEROID);
        world.fix(1, structure, ASTEROID);
        assert_ne!(first, second);
        assert_eq!(world.count(0, ASTEROID, structure), 2);
        assert_eq!(world.count(0, ASTEROID, unit), 0);
        assert_eq!(world.count(1, ASTEROID, structure), 1);
        assert_eq!(world.state.entities().count(), 3);
        assert_eq!(world.state.entity(first).hp(), structure.hp().0);
        assert_eq!(world.state.entity(second).id(), second);
    }

    #[test]
    fn a_fixed_entity_has_its_asteroids_body_and_a_free_one_its_own() {
        let mut world = world();
        let structure = P::Storage;
        let unit = P::Raider;
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
