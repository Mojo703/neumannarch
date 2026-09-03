//! The belt the game ships with: rocks laid by hand, and the state a match
//! starts from.

use core::f64::consts::TAU;
use std::collections::BTreeMap;

use crate::materials::Materials;
use crate::orbit::body::{Body, Gravity};
use crate::orbit::elements::Orbit;
use crate::roster::{CONSTRUCTOR, Roster, SHIPYARD};
use crate::setup::Setup;
use crate::state::{Rock, Seat, State};
use crate::time::Tick;
use crate::vec3::Vec3;

/// The radius the shipped belt is laid around, in meters.
const RADIUS: f64 = 1.0e7;

/// How far apart neighbouring rocks are laid along the belt, in meters.
/// A row crosses it in about a minute of thrust, so a send is a decision
/// and not a wait. A hypothesis the harness confirms or kills.
const SPACING: f64 = 2_000.0;

/// How far a rock's radius may sit from [`RADIUS`], in meters. Orbits
/// this far apart drift past each other over a match, so a rock's
/// neighbours change. A hypothesis.
const SPREAD: f64 = 2_000.0;

/// How far a rock swings from its mean radius, in meters, so no orbit is a
/// perfect circle. A hypothesis.
const EXCURSION: f64 = 500.0;

/// How far a rock swings out of the belt plane, in meters. A hypothesis.
const THICKNESS: f64 = 300.0;

/// Rocks in the shipped belt: three regions of seven.
const REGION: usize = 7;

/// Every rock's visual radius, in meters. Small against the band
/// amplitudes, so both bands read clear of it. A hypothesis.
const ROCK_RADIUS: f64 = 1.5;

/// What each material's cap is at a rock of the region that is rich in it,
/// in units per second. A hypothesis.
const RICH: f64 = 8.0;

/// What each material's cap is at a rock of a region that is poor in it,
/// in units per second. A hypothesis.
const POOR: f64 = 1.0;

/// What a seat starts with, in materials, which is also its base capacity.
const STARTING_STOCK: Materials = Materials::new(300.0, 100.0, 100.0);

/// The shipped belt.
pub struct Belt;

impl Belt {
    /// The gravitational parameter the shipped belt is laid for, in m³/s²:
    /// a rock at [`RADIUS`] takes forty minutes to go round. A hypothesis
    /// the harness confirms or kills.
    pub const GRAVITY: Gravity =
        Gravity::new(TAU * TAU * RADIUS * RADIUS * RADIUS / (2_400.0 * 2_400.0));

    /// Twenty-one rocks on near-circular, near-planar orbits under
    /// `gravity`, in three regions: one rich in metals, one in volatiles,
    /// one in energy.
    pub fn fixed(gravity: Gravity) -> Vec<Rock> {
        (0..3 * REGION).map(|at| rock(at, gravity)).collect()
    }
}

impl State {
    /// The start of the match `setup` names: nothing on the map, one seat
    /// per team it seats, each holding one shipyard and one constructor in
    /// reserve and [`STARTING_STOCK`] to spend.
    ///
    /// The rocks are [`Belt::fixed`] whatever the seed says; the seed is
    /// carried and hashed, and map generation lays the belt from it when
    /// that lands.
    pub fn start(setup: &Setup) -> State {
        let reserve = BTreeMap::from([(SHIPYARD, 1), (CONSTRUCTOR, 1)]);
        let seats = setup
            .teams()
            .iter()
            .map(|team| Seat::new(*team, STARTING_STOCK, reserve.clone()))
            .collect();
        State::new(
            setup.clock(),
            setup.seed(),
            Belt::GRAVITY,
            Roster::shipped(),
            Belt::fixed(Belt::GRAVITY),
            seats,
        )
    }
}

/// The rock at `at` around the belt: its own longitude, radius, tilt and
/// region's caps.
fn rock(at: usize, gravity: Gravity) -> Rock {
    let turn = SPACING * at as f64 / RADIUS;
    let radius = RADIUS + SPREAD * offset(at, 3);
    let speed = (gravity.mu() / radius).sqrt();
    let (sin, cos) = (libm::sin(turn), libm::cos(turn));
    let along = Vec3::new(-sin, 0.0, -cos);
    let outward = Vec3::new(cos, 0.0, -sin);
    // Half the excursion of a small eccentricity is `e·a`, and an
    // inclination `i` swings `i·a` out of plane; both come from the speed.
    let body = Body::new(
        outward * radius,
        along * (speed * (1.0 + 0.5 * EXCURSION / radius * offset(at, 5)))
            + Vec3::new(0.0, speed * (THICKNESS / radius) * offset(at, 7), 0.0),
    );
    let orbit = Orbit::from_body(body, Tick::ZERO, gravity)
        .expect("a rock of the shipped belt is on a bound orbit");
    Rock::new(orbit, caps(at), ROCK_RADIUS)
}

/// A spread of values in `-1.0..=1.0` for the rock at `at`, repeating every
/// `over` rocks, so no two neighbours share an orbit.
fn offset(at: usize, over: usize) -> f64 {
    2.0 * (at % over) as f64 / (over - 1) as f64 - 1.0
}

/// The rock's caps: rich in its region's material, poor in the other two,
/// and thinning with the rock's place in the region.
fn caps(at: usize) -> Materials {
    let thinning = 1.0 - 0.5 * (at % REGION) as f64 / REGION as f64;
    let (rich, poor) = (RICH * thinning, POOR * thinning);
    match at / REGION {
        0 => Materials::new(rich, poor, poor),
        1 => Materials::new(poor, rich, poor),
        _ => Materials::new(poor, poor, rich),
    }
}
