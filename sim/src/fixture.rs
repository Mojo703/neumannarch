use std::collections::BTreeMap;

use crate::TICKS_PER_SECOND;
use crate::belt::Belt;
use crate::ids::{EntityId, RockId, RowId, SeatId, TeamId};
use crate::materials::Materials;
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::post::Post;
use crate::roster::Roster;
use crate::setup::Setup;
use crate::state::view::View;
use crate::state::{Batch, Command, Flight, Issued, Motion, Rejected, Rock, Seat, State, view};
use crate::step::fire::{Fire, Hit, Shots};
use crate::step::holding::Holding;
use crate::step::propagation::Propagation;
use crate::time::Tick;
use crate::vec3::Vec3;

pub(crate) const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

const RING_RADIUS_METERS: f64 = 1.0e7;

const RING_SPACING_METERS: f64 = 1_000.0;

const ROCK_RADIUS_METERS: f64 = 1.5;

const RING_CAPS: Materials = Materials::new(1.0, 1.0, 1.0);

pub(crate) struct World {
    pub state: State,
}

impl World {
    pub fn started(teams: &[TeamId]) -> World {
        World::timed(teams, CLOCK)
    }

    pub fn timed(teams: &[TeamId], clock: Tick) -> World {
        let setup = Setup::new(teams.to_vec(), 0, clock).expect("a match of these teams");
        World {
            state: State::start(&setup),
        }
    }

    pub fn seated(seats: Vec<Seat>) -> World {
        World::crewed(Roster::shipped(), seats)
    }

    pub fn crewed(roster: Roster, seats: Vec<Seat>) -> World {
        World {
            state: State::new(
                CLOCK,
                0,
                Belt::GRAVITY,
                roster,
                Belt::fixed(Belt::GRAVITY),
                seats,
            ),
        }
    }

    pub fn stocked(stock: Materials, reserve: BTreeMap<RowId, u32>) -> World {
        World::seated(vec![Seat::new(TeamId(0), stock, reserve)])
    }

    pub fn ring(gravity: Gravity, rocks: usize, teams: &[TeamId]) -> World {
        World {
            state: State::new(
                CLOCK,
                0,
                gravity,
                Roster::shipped(),
                (0..rocks).map(|at| ringed(at, gravity)).collect(),
                teams
                    .iter()
                    .map(|team| Seat::new(*team, Materials::ZERO, BTreeMap::new()))
                    .collect(),
            ),
        }
    }

    pub fn period(&self, rock: RockId) -> u64 {
        let seconds = self.state[rock].orbit().period(self.state.gravity());
        (seconds * f64::from(TICKS_PER_SECOND)) as u64
    }

    pub fn tick(&mut self, issued: &[Issued]) {
        let (next, outcome) = self.state.step(&Batch::of(issued));
        assert_eq!(outcome.rejected, Vec::new(), "the commands were rejected");
        self.state = next;
    }

    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.tick(&[]);
        }
    }

    pub fn steers(&mut self, ticks: u64) {
        for _ in 0..ticks {
            let sweep = self.state.sweep();
            let thrusts = Holding::of(&self.state, &sweep).run();
            let moved = Propagation::of(&self.state, &thrusts).run();
            for step in moved.iter() {
                self.state.set_motion(
                    step.entity,
                    Motion::Steered {
                        body: step.body,
                        flight: step.flight,
                    },
                );
            }
            self.state.advance();
        }
    }

    pub fn refusal(&self, issued: Issued) -> Option<Rejected> {
        let (_, outcome) = self.state.step(&Batch::of(&[issued]));
        outcome.rejected.first().map(|(_, why)| *why)
    }

    pub fn fix(&mut self, seat: u8, row: RowId, rock: RockId) -> EntityId {
        self.state.spawn(SeatId(seat), row, rock, Motion::Fixed)
    }

    pub fn hold(&mut self, seat: u8, row: RowId, rock: RockId, out_meters: f64) -> EntityId {
        let body = self.state.rock_body(rock);
        let radial = body.pos.normalized().expect("a radius");
        self.free(
            seat,
            row,
            rock,
            Body::new(body.pos + radial * out_meters, body.vel),
        )
    }

    pub fn free(&mut self, seat: u8, row: RowId, rock: RockId, body: Body) -> EntityId {
        self.state.spawn(
            SeatId(seat),
            row,
            rock,
            Motion::Steered { body, flight: None },
        )
    }

    pub fn launch(&mut self, entity: EntityId, from: RockId, out_meters: f64, flight: Flight) {
        let body = self.state.rock_body(from);
        let radial = body.pos.normalized().expect("a radius");
        self.state.set_motion(
            entity,
            Motion::Steered {
                body: Body::new(body.pos + radial * out_meters, body.vel),
                flight: Some(flight),
            },
        );
    }

    pub fn count(&self, seat: u8, rock: RockId, row: RowId) -> u32 {
        self.state.count(Post::of(seat, rock), row)
    }

    pub fn progress(&self, seat: u8, rock: RockId) -> f64 {
        self.state
            .frames_at(Post::of(seat, rock))
            .map(|frame| frame.progress())
            .sum()
    }

    pub fn frames(&self, seat: u8, rock: RockId, row: RowId) -> usize {
        self.state
            .frames_at(Post::of(seat, rock))
            .filter(|frame| frame.row() == row)
            .count()
    }

    pub fn body(&self, entity: EntityId) -> Body {
        self.state.body_of(&self.state[entity])
    }

    pub fn off_rock(&self, entity: EntityId, rock: RockId) -> f64 {
        self.body(entity)
            .pos
            .distance(self.state.rock_body(rock).pos)
    }

    pub fn shots(&self) -> Shots {
        Fire::of(&self.state, &self.state.sweep()).run()
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

    pub fn view(&self, seat: u8) -> View {
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
    pub(crate) fn of(issued: &[Issued]) -> Batch {
        let mut batch = Batch::new();
        for issued in issued {
            assert_eq!(batch.insert(*issued), Ok(()));
        }
        batch
    }
}

impl Issued {
    pub(crate) fn want(seat: u8, rock: RockId, row: RowId, count: u32) -> Issued {
        Issued::numbered(seat, 0, rock, row, count)
    }

    pub(crate) fn numbered(seat: u8, seq: u32, rock: RockId, row: RowId, count: u32) -> Issued {
        Issued {
            seat: SeatId(seat),
            seq,
            command: Command::Want { rock, row, count },
        }
    }
}

impl Post {
    pub(crate) fn of(seat: u8, rock: RockId) -> Post {
        Post {
            rock,
            seat: SeatId(seat),
        }
    }
}

fn ringed(at: usize, gravity: Gravity) -> Rock {
    let radius = RING_RADIUS_METERS + RING_SPACING_METERS * at as f64;
    let speed = (gravity.mu() / radius).sqrt();
    let body = Body::new(Vec3::new(radius, 0.0, 0.0), Vec3::new(0.0, 0.0, -speed));
    let orbit = Orbit::from_body(body, Tick::ZERO, gravity).expect("a circular orbit");
    Rock::new(orbit, RING_CAPS, ROCK_RADIUS_METERS)
}
