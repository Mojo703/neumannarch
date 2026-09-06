use core::f64::consts::TAU;
use std::collections::BTreeMap;

use crate::materials::Materials;
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::roster::{CONSTRUCTOR, Roster, SHIPYARD};
use crate::setup::Setup;
use crate::state::{Asteroid, Seat, State};
use crate::time::Time;
use crate::vec3::Vec3;

const RADIUS: f64 = 1.0e7;

const SPACING: f64 = 2_000.0;

const SPREAD: f64 = 2_000.0;

const EXCURSION: f64 = 500.0;

const THICKNESS: f64 = 300.0;

const REGION: usize = 7;

const ASTEROID_RADIUS: f64 = 1.5;

const RICH: f64 = 8.0;

const POOR: f64 = 1.0;

const STARTING_STOCK: Materials = Materials::new(300.0, 100.0, 100.0);

pub struct Belt;

impl Belt {
    pub const GRAVITY: Gravity =
        Gravity::new(TAU * TAU * RADIUS * RADIUS * RADIUS / (2_400.0 * 2_400.0));

    pub const ZONE_RADIUS_METERS: f64 = 30.0;

    pub const FIELD_SCALE_METERS: f64 = 15.0;

    pub const ARRIVAL_METERS: f64 = 7.5;

    pub const SPACING_METERS: f64 = 0.5;

    pub fn fixed(gravity: Gravity) -> Vec<Asteroid> {
        (0..3 * REGION).map(|at| asteroid(at, gravity)).collect()
    }
}

impl State {
    pub fn start(setup: &Setup) -> State {
        let reserve = BTreeMap::from([(SHIPYARD, 1), (CONSTRUCTOR, 1)]);
        let seats = setup
            .teams()
            .iter()
            .map(|team| Seat::new(*team, STARTING_STOCK, reserve.clone()))
            .collect();
        State::new(
            Time(setup.clock().0),
            setup.seed(),
            Belt::GRAVITY,
            Roster::shipped(),
            Belt::fixed(Belt::GRAVITY),
            seats,
        )
    }
}

fn asteroid(at: usize, gravity: Gravity) -> Asteroid {
    let turn = SPACING * at as f64 / RADIUS;
    let radius = RADIUS + SPREAD * offset(at, 3);
    let speed = (gravity.mu() / radius).sqrt();
    let (sin, cos) = (libm::sin(turn), libm::cos(turn));
    let along = Vec3::new(-sin, 0.0, -cos);
    let outward = Vec3::new(cos, 0.0, -sin);

    let body = Body::new(
        outward * radius,
        along * (speed * (1.0 + 0.5 * EXCURSION / radius * offset(at, 5)))
            + Vec3::new(0.0, speed * (THICKNESS / radius) * offset(at, 7), 0.0),
    );
    let orbit = Orbit::from_body(body, Time::ZERO, gravity)
        .expect("an asteroid of the shipped belt is on a bound orbit");
    Asteroid::new(orbit, caps(at), ASTEROID_RADIUS)
}

fn offset(at: usize, over: usize) -> f64 {
    2.0 * (at % over) as f64 / (over - 1) as f64 - 1.0
}

fn caps(at: usize) -> Materials {
    let thinning = 1.0 - 0.5 * (at % REGION) as f64 / REGION as f64;
    let (rich, poor) = (RICH * thinning, POOR * thinning);
    match at / REGION {
        0 => Materials::new(rich, poor, poor),
        1 => Materials::new(poor, rich, poor),
        _ => Materials::new(poor, poor, rich),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS_PER_SECOND;

    #[test]
    fn every_asteroid_of_the_shipped_belt_has_a_floor_well_inside_its_zone() {
        for asteroid in Belt::fixed(Belt::GRAVITY) {
            let floor = asteroid.radius() + Belt::SPACING_METERS;
            assert!(
                floor < Belt::ZONE_RADIUS_METERS,
                "an asteroid of {} leaves no shell",
                asteroid.radius()
            );
        }
    }

    #[test]
    fn no_two_zones_of_the_shipped_belt_overlap() {
        let asteroids = Belt::fixed(Belt::GRAVITY);
        let apart = 2.0 * Belt::ZONE_RADIUS_METERS;
        for minutes in 0..15 {
            let at = Time(minutes * 60 * u64::from(TICKS_PER_SECOND));
            let bodies: Vec<Body> = asteroids
                .iter()
                .map(|asteroid| asteroid.orbit().at(at, Belt::GRAVITY))
                .collect();
            for (at, body) in bodies.iter().enumerate() {
                for other in &bodies[at + 1..] {
                    let between = body.pos.distance(other.pos);
                    assert!(between > apart, "{between} meters apart at {at:?}");
                }
            }
        }
    }
}
