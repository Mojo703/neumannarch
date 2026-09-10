use core::f64::consts::TAU;

use super::asteroid::Asteroid;
use crate::belt::Belt;
use crate::ids::EntityId;
use crate::noise;
use crate::orbit::body::Body;
use crate::pattern::EntityPattern;
use crate::time::Time;
use crate::vec3::Vec3;

const TURNS_OF_THE_LIMIT: f64 = 0.25;

#[derive(Hash)]
enum Drawn {
    Tilt,
    Turn,
    Phase,
}

pub(crate) struct Circle {
    centre: Vec3,
    across: Vec3,
    along: Vec3,
    radius_meters: f64,
    turns_per_second: f64,
    phase_at_the_start: f64,
}

impl Circle {
    pub(crate) fn of(
        id: EntityId,
        pattern: EntityPattern,
        asteroid: &Asteroid,
        body: Body,
    ) -> Circle {
        let drawn = |what: Drawn| noise::fraction(&(what, id));
        let rise = 2.0 * drawn(Drawn::Tilt) - 1.0;
        let turn = TAU * drawn(Drawn::Turn);
        let spread = (1.0 - rise * rise).sqrt();
        let normal = Vec3::new(spread * libm::cos(turn), rise, spread * libm::sin(turn));
        let across = Vec3::new(-libm::sin(turn), 0.0, libm::cos(turn));
        let radius_meters = asteroid.floor_meters() + Belt::STATION_SPACING_METERS;
        Circle {
            centre: body.pos,
            across,
            along: normal.cross(across),
            radius_meters,
            turns_per_second: TURNS_OF_THE_LIMIT * (pattern.manoeuvring().0 / radius_meters).sqrt()
                / TAU,
            phase_at_the_start: drawn(Drawn::Phase),
        }
    }

    pub(crate) fn station(&self, at: Time) -> Vec3 {
        let turn = TAU * self.phase(at);
        let along_the_rim = self.across * libm::cos(turn) + self.along * libm::sin(turn);
        self.centre + along_the_rim * self.radius_meters
    }

    fn phase(&self, at: Time) -> f64 {
        self.phase_at_the_start + self.turns_per_second * at.seconds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::{AsteroidId, TeamId};
    use crate::orbit::body::Gravity;
    use crate::pattern::Kind;
    use crate::state::State;

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: AsteroidId = AsteroidId(0);

    const SETTLED_SECONDS: f64 = 60.0;

    fn state() -> State {
        World::ring(GRAVITY, 1, &[TeamId(0)]).state
    }

    fn circle(state: &State, id: EntityId) -> Circle {
        Circle::of(
            id,
            EntityPattern::Constructor,
            &state[HOME],
            state.asteroid_body(HOME),
        )
    }

    #[test]
    fn a_station_stands_on_the_circle_about_the_asteroids_body() {
        let state = state();
        let circle = circle(&state, EntityId(4));
        let body = state.asteroid_body(HOME);

        for tick in 0..600 {
            let at = Time(tick * 17);
            let out = circle.station(at) - body.pos;
            assert!(
                (out.length() - circle.radius_meters).abs() < 1e-9,
                "a station stands {} out, not {}",
                out.length(),
                circle.radius_meters
            );
            assert!(
                out.dot(circle.across.cross(circle.along)).abs() < 1e-9,
                "a station left the plane at {at:?}"
            );
        }
    }

    #[test]
    fn the_circle_clears_the_rock_it_turns_about_and_stands_inside_the_fight_stage() {
        let reach = EntityPattern::LONGEST_DAMAGE_RANGE_METERS;
        let state = state();
        let circle = circle(&state, EntityId(0));
        let floor = state[HOME].floor_meters();

        assert!(
            circle.radius_meters > floor,
            "a circle of {} turns inside the floor of {floor}",
            circle.radius_meters
        );
        assert!(
            circle.radius_meters < 0.5 * reach,
            "a circle of {} reaches the stage standing {} out",
            circle.radius_meters,
            0.5 * reach
        );
        assert!(
            circle.radius_meters < Belt::ZONE_RADIUS_METERS,
            "a circle of {} leaves the zone",
            circle.radius_meters
        );
    }

    #[test]
    fn the_circle_turns_at_a_quarter_of_the_speed_the_limit_holds_on_it() {
        let state = state();
        let circle = circle(&state, EntityId(1));
        let body = state.asteroid_body(HOME);
        let turned = |at: Time| {
            (circle.station(at) - body.pos)
                .normalized()
                .expect("a point on the rim")
        };

        let over = turned(Time::ZERO).dot(turned(Time(u64::from(crate::TICKS_PER_SECOND))));
        let limit = EntityPattern::Constructor.manoeuvring().0;
        let held = (limit * circle.radius_meters).sqrt();
        let wanted = TURNS_OF_THE_LIMIT * held / circle.radius_meters;

        assert!(
            (libm::acos(over.clamp(-1.0, 1.0)) - wanted).abs() < 1e-9,
            "it turns {} radians a second, not {wanted}",
            libm::acos(over)
        );
    }

    #[test]
    fn every_unit_that_does_no_damage_circles_a_radius_a_pattern_of_its_limit_can_hold() {
        let state = state();
        for pattern in EntityPattern::EVERY {
            if pattern.kind() != Kind::Unit || pattern.does_damage() {
                continue;
            }
            let circle = Circle::of(
                EntityId(0),
                pattern,
                &state[HOME],
                state.asteroid_body(HOME),
            );
            let speed = TAU * circle.turns_per_second * circle.radius_meters;
            assert!(
                speed * speed / circle.radius_meters < pattern.manoeuvring().0,
                "a {} cannot hold {speed} on a circle of {}",
                pattern.name(),
                circle.radius_meters
            );
            assert_eq!(
                pattern,
                EntityPattern::Constructor,
                "{} is a second unit that does no damage",
                pattern.name()
            );
        }
    }

    #[test]
    fn two_units_of_one_pattern_take_two_planes_and_two_starting_phases() {
        let state = state();
        let one = circle(&state, EntityId(3));
        let other = circle(&state, EntityId(4));

        let aligned = one
            .across
            .cross(one.along)
            .dot(other.across.cross(other.along));
        assert!(aligned.abs() < 0.99, "the pair shares a plane, {aligned}");
        assert_ne!(one.phase_at_the_start, other.phase_at_the_start);
        assert!(
            one.station(Time::ZERO).distance(other.station(Time::ZERO)) > Belt::SPACING_METERS,
            "the pair starts within a spacing of one point"
        );
    }

    #[test]
    fn a_unit_keeps_its_circle_from_one_tick_to_the_next() {
        let state = state();
        let again = circle(&state, EntityId(7));
        let circle = circle(&state, EntityId(7));
        let at = Time((SETTLED_SECONDS * f64::from(crate::TICKS_PER_SECOND)) as u64);

        assert_eq!(circle.station(at), again.station(at));
        assert_eq!(circle.across, again.across);
        assert_eq!(circle.along, again.along);
    }
}
