use core::f64::consts::TAU;
use std::collections::BTreeMap;

use crate::materials::{Material, Materials};
use crate::noise::{self, Noise};
use crate::orbit::body::Gravity;
use crate::orbit::elements::Orbit;
use crate::pattern::EntityPattern;
use crate::setup::Setup;
use crate::state::hash::digest;
use crate::state::{Asteroid, Seat, State};
use crate::time::Time;
use crate::vec3::Vec3;

const REFERENCE_PERIOD_SECONDS: f64 = 40.0 * 60.0;

const REFERENCE_MATCH_SECONDS: f64 = 15.0 * 60.0;

const SHEAR_TURNS: f64 = 0.5;

const CANDIDATE_ASTEROIDS: u64 = 200;

const THINNEST_DENSITY: f64 = 0.5;

const EXCURSION_METERS: f64 = 1_000.0;

const GREATEST_ECCENTRICITY: f64 = 0.1;

const ASTEROID_RADIUS_METERS: f64 = 2.0;

const REGION_CELL_METERS: f64 = 8_000.0;

const RICHEST_CAP_PER_SECOND: f64 = 8.0;

const POOREST_CAP_PER_SECOND: f64 = 1.0;

const STARTING_STOCK: Materials = Materials::new(300.0, 100.0, 100.0);

#[derive(Hash)]
enum Drawn {
    Density,
    Eccentricity,
    Caps(Material),
    Radius,
    Longitude,
    Node,
    Periapsis,
    Kept,
}

pub struct Belt;

impl Belt {
    pub const OUTER_RADIUS_METERS: f64 = 25_000.0;

    pub const GRAVITY: Gravity = Gravity::new(
        TAU * TAU
            * Self::OUTER_RADIUS_METERS
            * Self::OUTER_RADIUS_METERS
            * Self::OUTER_RADIUS_METERS
            / (REFERENCE_PERIOD_SECONDS * REFERENCE_PERIOD_SECONDS),
    );

    pub const MOVEMENT_LIMIT_METERS_PER_SECOND_SQUARED: f64 = 80.0;

    pub const STAR_RADIUS_METERS: f64 = 2_500.0;

    pub const STAR_LIGHT_RANGE_METERS: f64 = 75_000.0;

    pub const ZONE_RADIUS_METERS: f64 = 30.0;

    pub(crate) const ARRIVAL_METERS: f64 = 7.5;

    pub const SPACING_METERS: f64 = 0.5;

    pub(crate) const STATION_SPACING_METERS: f64 = 2.0;

    pub(crate) const STATIONS_EACH_WAY: usize = 5;

    pub(crate) const RANKS_BEHIND: usize = 5;

    pub(crate) const LINES_INSIDE_RANGE_METERS: f64 = 1.0;

    pub(crate) const REACHED_METERS: f64 = 0.5;

    #[cfg(test)]
    pub(crate) fn furthest_station_meters(
        longest_damage_range_meters: f64,
        standoff_meters: f64,
    ) -> f64 {
        let outward = 0.5 * longest_damage_range_meters;
        let rear = standoff_meters + Self::RANKS_BEHIND as f64 * Self::STATION_SPACING_METERS;
        let aside = Self::STATIONS_EACH_WAY as f64 * Self::STATION_SPACING_METERS;
        let lift = ASTEROID_RADIUS_METERS + Self::SPACING_METERS;
        libm::hypot(lift, outward + libm::hypot(rear, aside))
    }

    pub fn inner_radius_meters() -> f64 {
        let gained = 1.0 + SHEAR_TURNS * REFERENCE_PERIOD_SECONDS / REFERENCE_MATCH_SECONDS;
        Self::OUTER_RADIUS_METERS / libm::cbrt(gained * gained)
    }

    pub fn from_seed(seed: u64) -> Vec<Asteroid> {
        let field = |drawn: Drawn| Noise::keyed(digest(&(seed, drawn)), REGION_CELL_METERS);
        let density = field(Drawn::Density);
        let eccentricity = field(Drawn::Eccentricity);
        let caps: Vec<Noise> = Material::EVERY
            .into_iter()
            .map(Drawn::Caps)
            .map(field)
            .collect();

        let inner = Belt::inner_radius_meters();
        let (nearest, farthest) = (
            inner * inner,
            Belt::OUTER_RADIUS_METERS * Belt::OUTER_RADIUS_METERS,
        );

        (0..CANDIDATE_ASTEROIDS)
            .filter_map(|draw| {
                let drawn = |what: Drawn| noise::fraction(&(seed, what, draw));
                let over_area = drawn(Drawn::Radius);
                let radius = (nearest + (farthest - nearest) * over_area).sqrt();
                let longitude = TAU * drawn(Drawn::Longitude);
                let node = TAU * drawn(Drawn::Node);
                let at = Belt::orbit(radius, 0.0, 0.0, node, longitude)
                    .at(Time::ZERO, Belt::GRAVITY)
                    .pos;
                let kept = THINNEST_DENSITY + (1.0 - THINNEST_DENSITY) * density.at(at);
                (drawn(Drawn::Kept) < kept).then(|| {
                    Asteroid::new(
                        Belt::orbit(
                            radius,
                            GREATEST_ECCENTRICITY * eccentricity.at(at),
                            TAU * drawn(Drawn::Periapsis),
                            node,
                            longitude,
                        ),
                        Belt::caps_at(&caps, at),
                        ASTEROID_RADIUS_METERS,
                    )
                })
            })
            .collect()
    }

    fn orbit(
        semi_major_axis_meters: f64,
        eccentricity: f64,
        periapsis_radians: f64,
        node_radians: f64,
        longitude_radians: f64,
    ) -> Orbit {
        Orbit::tilted_ellipse(
            semi_major_axis_meters,
            eccentricity,
            periapsis_radians,
            node_radians,
            EXCURSION_METERS,
            longitude_radians,
            Time::ZERO,
        )
        .expect("an orbit of a radius inside the belt and an eccentricity below one")
    }

    fn caps_at(fields: &[Noise], at: Vec3) -> Materials {
        let mut caps = Materials::ZERO;
        for (material, field) in Material::EVERY.into_iter().zip(fields) {
            caps[material] = POOREST_CAP_PER_SECOND
                + (RICHEST_CAP_PER_SECOND - POOREST_CAP_PER_SECOND) * field.at(at);
        }
        caps
    }
}

impl State {
    pub fn start(setup: &Setup) -> State {
        let reserve = BTreeMap::from([
            (EntityPattern::Shipyard, 1),
            (EntityPattern::Constructor, 1),
        ]);
        let seats = setup
            .teams()
            .iter()
            .map(|team| Seat::new(*team, STARTING_STOCK, reserve.clone()))
            .collect();
        State::new(
            setup.clock(),
            setup.seed(),
            Belt::GRAVITY,
            Belt::from_seed(setup.seed()),
            seats,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEWEST_ASTEROIDS: usize = 100;

    const MOST_ASTEROIDS: usize = 200;

    const SEEDS: u64 = 8;

    const SAMPLES_PER_ORBIT: u32 = 64;

    const MOST_OF_A_RANGE: f64 = 0.7;

    fn spans_most_of(read: impl Iterator<Item = f64>, least: f64, most: f64) -> bool {
        let (poorest, richest) = read
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
                (low.min(value), high.max(value))
            });
        richest - poorest >= MOST_OF_A_RANGE * (most - least)
    }

    #[test]
    fn every_seed_keeps_between_a_hundred_and_two_hundred_asteroids() {
        for seed in 0..64 {
            let kept = Belt::from_seed(seed).len();
            assert!(
                (FEWEST_ASTEROIDS..=MOST_ASTEROIDS).contains(&kept),
                "seed {seed} keeps {kept} asteroids"
            );
        }
    }

    #[test]
    fn one_seed_lays_one_belt() {
        assert_eq!(Belt::from_seed(3), Belt::from_seed(3));
    }

    #[test]
    fn two_seeds_lay_two_belts() {
        assert_ne!(Belt::from_seed(3), Belt::from_seed(4));
    }

    #[test]
    fn every_asteroid_of_a_seeded_belt_has_a_floor_well_inside_its_zone() {
        for seed in 0..SEEDS {
            for asteroid in Belt::from_seed(seed) {
                let floor = asteroid.radius() + Belt::SPACING_METERS;
                assert!(
                    floor < Belt::ZONE_RADIUS_METERS,
                    "an asteroid of {} leaves no shell",
                    asteroid.radius()
                );
            }
        }
    }

    #[test]
    fn the_inner_edge_gains_half_a_turn_on_the_outer_over_a_reference_match() {
        let rate =
            |radius: f64| TAU / Belt::orbit(radius, 0.0, 0.0, 0.0, 0.0).period(Belt::GRAVITY);
        let gained = (rate(Belt::inner_radius_meters()) - rate(Belt::OUTER_RADIUS_METERS))
            * REFERENCE_MATCH_SECONDS
            / TAU;
        assert!(
            (gained - SHEAR_TURNS).abs() < 1e-9,
            "the inner edge gains {gained} turns, not {SHEAR_TURNS}"
        );
    }

    #[test]
    fn a_seeded_belts_caps_span_most_of_the_poorest_to_the_richest() {
        let belt = Belt::from_seed(0);
        for material in Material::EVERY {
            assert!(
                spans_most_of(
                    belt.iter().map(|asteroid| asteroid.caps()[material]),
                    POOREST_CAP_PER_SECOND,
                    RICHEST_CAP_PER_SECOND
                ),
                "{material:?} keeps to the middle of the belt's caps"
            );
        }
    }

    #[test]
    fn a_seeded_belt_holds_both_stirred_and_calm_regions() {
        let belt = Belt::from_seed(0);
        assert!(
            spans_most_of(
                belt.iter().map(|asteroid| asteroid.orbit().eccentricity()),
                0.0,
                GREATEST_ECCENTRICITY
            ),
            "every orbit of the belt is stirred alike"
        );
    }

    #[test]
    fn no_asteroid_of_a_seeded_belt_leaves_the_belts_thickness() {
        let thickness = EXCURSION_METERS * (1.0 + GREATEST_ECCENTRICITY);
        for seed in 0..SEEDS {
            for asteroid in Belt::from_seed(seed) {
                let ticks =
                    asteroid.orbit().period(Belt::GRAVITY) * f64::from(crate::TICKS_PER_SECOND);
                for sample in 0..SAMPLES_PER_ORBIT {
                    let at =
                        Time((ticks * f64::from(sample) / f64::from(SAMPLES_PER_ORBIT)) as u64);
                    let rise = asteroid.orbit().at(at, Belt::GRAVITY).pos.y.abs();
                    assert!(rise <= thickness, "seed {seed} rises {rise} of {thickness}");
                }
            }
        }
    }
}
