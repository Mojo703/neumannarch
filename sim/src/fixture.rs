use std::collections::BTreeMap;

use crate::TICKS_PER_SECOND;
use crate::belt::Belt;
use crate::ids::{AsteroidId, EntityId, RowId, SeatId, TeamId};
use crate::materials::Materials;
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::post::Post;
use crate::roster::Roster;
use crate::setup::Setup;
use crate::state::view::View;
use crate::state::{Asteroid, Batch, Command, Issued, Motion, Rejected, Rolls, Seat, State, view};
use crate::step::fire::{Fire, Hit, Shots};
use crate::step::holding::Holding;
use crate::step::propagation::Propagation;
use crate::time::Time;
use crate::vec3::Vec3;

pub const CLOCK: Time = Time(15 * 60 * TICKS_PER_SECOND as u64);

const RING_RADIUS_METERS: f64 = 1.0e7;

const RING_SPACING_METERS: f64 = 1_000.0;

const ASTEROID_RADIUS_METERS: f64 = 1.5;

const RING_CAPS: Materials = Materials::new(1.0, 1.0, 1.0);

pub(crate) struct World {
    pub(crate) state: State,
}

impl World {
    pub fn started(teams: &[TeamId]) -> World {
        World::timed(teams, CLOCK)
    }

    pub fn timed(teams: &[TeamId], clock: Time) -> World {
        let mut world = World::drafting(teams, clock);
        world.start_the_clock();
        world
    }

    pub fn drafting(teams: &[TeamId], clock: Time) -> World {
        let setup = Setup::new(teams.to_vec(), 0, clock).expect("a match of these teams");
        World {
            state: State::start(&setup),
        }
    }

    pub(crate) fn seated(seats: Vec<Seat>) -> World {
        World::crewed(Roster::shipped(), seats)
    }

    pub(crate) fn crewed(roster: Roster, seats: Vec<Seat>) -> World {
        let mut world = World {
            state: State::new(CLOCK, 0, Belt::GRAVITY, roster, Belt::from_seed(0), seats),
        };
        world.start_the_clock();
        world
    }

    pub fn draft(&mut self, seat: u8, from: AsteroidId) {
        let picks: Vec<Issued> = self.state[SeatId(seat)]
            .reserve()
            .keys()
            .enumerate()
            .map(|(at, row)| {
                let at = at as u32;
                Issued::numbered(seat, at, AsteroidId(from.0 + at), *row, 1)
            })
            .collect();
        self.tick(&picks);
    }

    pub(crate) fn start_the_clock(&mut self) {
        while !self.state.draft().over(self.state.tick()) {
            self.tick(&[]);
        }
    }

    pub fn stocked(stock: Materials, reserve: BTreeMap<RowId, u32>) -> World {
        World::seated(vec![Seat::new(TeamId(0), stock, reserve)])
    }

    pub fn ring(gravity: Gravity, asteroids: usize, teams: &[TeamId]) -> World {
        let mut world = World {
            state: State::new(
                CLOCK,
                0,
                gravity,
                Roster::shipped(),
                (0..asteroids).map(|at| ringed(at, gravity)).collect(),
                teams
                    .iter()
                    .map(|team| Seat::new(*team, Materials::ZERO, BTreeMap::new()))
                    .collect(),
            ),
        };
        world.start_the_clock();
        world
    }

    pub fn period(&self, asteroid: AsteroidId) -> u64 {
        let seconds = self.state[asteroid].orbit().period(self.state.gravity());
        (seconds * f64::from(TICKS_PER_SECOND)) as u64
    }

    pub fn tick(&mut self, issued: &[Issued]) {
        let (next, outcome) = self.state.step(&Batch::of(issued));
        assert_eq!(outcome.rejected, Vec::new(), "the commands were rejected");
        self.state = next;
    }

    pub(crate) fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.tick(&[]);
        }
    }

    pub(crate) fn steers(&mut self, ticks: u64) {
        for _ in 0..ticks {
            let rolls = Rolls::called(&self.state);
            let steering = Holding::of(&self.state, &rolls, &Shots::default()).run();
            let moved = Propagation::of(&self.state, &steering.thrusts).run();
            drop(rolls);
            steering.set_passes(&mut self.state);
            for step in moved.iter() {
                self.state.steer(step.entity, step.body);
            }
            self.state.arrive(moved.arrived());
            self.state.advance();
        }
    }

    pub fn refusal(&self, issued: Issued) -> Option<Rejected> {
        let (_, outcome) = self.state.step(&Batch::of(&[issued]));
        outcome.rejected.first().map(|(_, why)| *why)
    }

    pub fn fix(&mut self, seat: u8, row: RowId, asteroid: AsteroidId) -> EntityId {
        self.state.spawn(SeatId(seat), row, asteroid, Motion::Fixed)
    }

    pub(crate) fn hold(
        &mut self,
        seat: u8,
        row: RowId,
        asteroid: AsteroidId,
        out_meters: f64,
    ) -> EntityId {
        let body = self.state.asteroid_body(asteroid);
        let radial = body.pos.normalized().expect("a radius");
        self.free(
            seat,
            row,
            asteroid,
            Body::new(body.pos + radial * out_meters, body.vel),
        )
    }

    pub fn free(&mut self, seat: u8, row: RowId, asteroid: AsteroidId, body: Body) -> EntityId {
        self.state
            .spawn(SeatId(seat), row, asteroid, Motion::Steered { body })
    }

    pub fn send(&mut self, entity: EntityId, to: AsteroidId) {
        self.state.re_home(&BTreeMap::from([(entity, to)]));
    }

    pub fn count(&self, seat: u8, asteroid: AsteroidId, row: RowId) -> u32 {
        self.state.count(Post::of(seat, asteroid), row)
    }

    pub fn progress(&self, seat: u8, asteroid: AsteroidId) -> f64 {
        self.state
            .frames_at(Post::of(seat, asteroid))
            .map(|frame| frame.progress())
            .sum()
    }

    pub fn frames(&self, seat: u8, asteroid: AsteroidId, row: RowId) -> usize {
        self.state
            .frames_at(Post::of(seat, asteroid))
            .filter(|frame| frame.row() == row)
            .count()
    }

    pub fn still_holds(&self, entity: EntityId) -> bool {
        self.state.entities().any(|held| held.id() == entity)
    }

    pub fn body(&self, entity: EntityId) -> Body {
        self.state.body_of(self.state.entity(entity))
    }

    pub fn off_asteroid(&self, entity: EntityId, asteroid: AsteroidId) -> f64 {
        self.body(entity)
            .pos
            .distance(self.state.asteroid_body(asteroid).pos)
    }

    pub(crate) fn shots(&self) -> Shots {
        Fire::of(&self.state, &Rolls::called(&self.state)).run()
    }

    pub fn shot_at(&self, target: EntityId, ticks: u64) -> Option<Hit> {
        let mut world = World {
            state: self.state.clone(),
        };
        for _ in 0..ticks {
            let hit = world
                .shots()
                .hits
                .into_iter()
                .find(|hit| hit.target == target);
            if hit.is_some() {
                return hit;
            }
            world.tick(&[]);
        }
        None
    }

    pub(crate) fn view(&self, seat: u8) -> View {
        View::of(&self.state, SeatId(seat), &Shots::default())
    }

    pub fn present(&self, seat: u8, entity: EntityId) -> Option<view::Present> {
        self.view(seat)
            .present
            .into_iter()
            .find(|present| present.id == entity)
    }
}

impl Batch {
    pub fn of(issued: &[Issued]) -> Batch {
        let mut batch = Batch::new();
        for issued in issued {
            assert_eq!(batch.insert(*issued), Ok(()));
        }
        batch
    }
}

impl Issued {
    pub fn want(seat: u8, asteroid: AsteroidId, row: RowId, count: u32) -> Issued {
        Issued::numbered(seat, 0, asteroid, row, count)
    }

    pub(crate) fn numbered(
        seat: u8,
        seq: u32,
        asteroid: AsteroidId,
        row: RowId,
        count: u32,
    ) -> Issued {
        Issued {
            seat: SeatId(seat),
            seq,
            command: Command::Want {
                asteroid,
                row,
                count,
            },
        }
    }
}

impl Post {
    pub fn of(seat: u8, asteroid: AsteroidId) -> Post {
        Post {
            asteroid,
            seat: SeatId(seat),
        }
    }
}

fn ringed(at: usize, gravity: Gravity) -> Asteroid {
    let radius = RING_RADIUS_METERS + RING_SPACING_METERS * at as f64;
    let speed = (gravity.mu() / radius).sqrt();
    let body = Body::new(Vec3::new(radius, 0.0, 0.0), Vec3::new(0.0, 0.0, -speed));
    let orbit = Orbit::from_body(body, Time::ZERO, gravity).expect("a circular orbit");
    Asteroid::new(orbit, RING_CAPS, ASTEROID_RADIUS_METERS)
}
