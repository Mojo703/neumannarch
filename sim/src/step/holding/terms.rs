use core::f64::consts::TAU;

use super::field::Sample;
use crate::belt::Belt;
use crate::ids::EntityId;
use crate::orbit::body::Body;
use crate::real::Real;
use crate::roster::Row;
use crate::time::Time;
use crate::vec3::Vec3;

const DRIFT: [(f64, f64); 3] = [(0.037, 0.0), (0.053, 1.0), (0.071, 2.0)];

const DRIFT_PHASES: u32 = 1024;

pub fn wander(row: &Row, at: Time, id: EntityId) -> Vec3 {
    drift(at, id) * row.steering.wander.0
}

pub fn separation(body: Body, row: &Row, neighbours: impl Iterator<Item = Vec3>) -> Vec3 {
    neighbours
        .filter_map(|pos| {
            let offset = pos - body.pos;
            let apart = offset.length();
            let away = (-offset).normalized()?;
            (apart < Belt::SPACING_METERS).then(|| away * (1.0 - apart / Belt::SPACING_METERS))
        })
        .fold(Vec3::ZERO, |sum, push| sum + push)
        * row.steering.separation.0
}

pub(crate) fn cohesion(row: &Row, sample: Sample) -> Vec3 {
    sample.own_lean() * row.steering.cohesion.0
}

pub(crate) fn caution(row: &Row, sample: Sample) -> Vec3 {
    sample.retreat() * row.steering.caution.0
}

pub fn returning(body: Body, row: &Row, asteroid: Body, strayed: f64) -> Vec3 {
    toward(body, row, row.steering.returning, asteroid, strayed)
}

pub fn chase(body: Body, row: &Row, target: Body) -> Vec3 {
    toward(
        body,
        row,
        row.steering.chase,
        target,
        target.pos.distance(body.pos),
    )
}

fn toward(body: Body, row: &Row, weight: Real, place: Body, miss: f64) -> Vec3 {
    let speed = (2.0 * row.manoeuvring.0 * miss.abs().min(Belt::ARRIVAL_METERS)).sqrt();
    let wanted = (place.pos - body.pos)
        .normalized()
        .map_or(Vec3::ZERO, |inward| {
            inward * if miss < 0.0 { -speed } else { speed }
        });
    (place.vel + wanted - body.vel) * weight.0
}

fn drift(tick: Time, id: EntityId) -> Vec3 {
    let spread = f64::from(id.0.wrapping_mul(2_654_435_761) % DRIFT_PHASES)
        * (TAU / f64::from(DRIFT_PHASES));
    let along =
        |(turns, offset): (f64, f64)| libm::sin(spread + offset + tick.seconds() * turns * TAU);
    Vec3::new(along(DRIFT[0]), along(DRIFT[1]), along(DRIFT[2]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS_PER_SECOND;
    use crate::materials::Materials;
    use crate::roster::Weights;
    use crate::roster::{Role, Tier};

    const LIMIT: f64 = 1.25;

    const RADIUS: f64 = 1.5;

    fn weights() -> Weights {
        Weights {
            wander: Real(1.0),
            returning: Real(1.0),
            separation: Real(1.0),
            cohesion: Real(1.0),
            caution: Real(1.0),
            chase: Real(1.0),
        }
    }

    fn row() -> Row {
        Row {
            name: "test",
            role: Role::Scout,
            tier: Tier::ONE,
            cost: Materials::new(1.0, 0.0, 0.0),
            manoeuvring: Real(LIMIT),
            steering: weights(),
            hp: Real(1.0),
            plating: Real(0.0),
            capacity: Materials::ZERO,
            weapons: vec![],
        }
    }

    fn asteroid() -> Body {
        Body::new(Vec3::ZERO, Vec3::ZERO)
    }

    fn floor() -> f64 {
        RADIUS + Belt::SPACING_METERS
    }

    fn out(meters: f64) -> Vec3 {
        Vec3::new(0.0, 0.0, meters)
    }

    fn strayed(body: Body) -> f64 {
        let distance = body.pos.distance(Vec3::ZERO);
        (distance - Belt::ZONE_RADIUS_METERS).max(0.0) - (floor() - distance).max(0.0)
    }

    fn cruise(over: f64) -> f64 {
        (2.0 * LIMIT * over.min(Belt::ARRIVAL_METERS)).sqrt()
    }

    #[test]
    fn the_desired_speed_is_what_the_limit_can_stop_from_within_the_arrival_distance() {
        let row = row();
        let far = Body::new(out(3.0 * Belt::ZONE_RADIUS_METERS), Vec3::ZERO);
        assert!(
            (returning(far, &row, asteroid(), strayed(far)).length()
                - cruise(Belt::ARRIVAL_METERS))
            .abs()
                < 1e-9
        );

        let close = Body::new(out(Belt::ZONE_RADIUS_METERS + 1.0), Vec3::ZERO);
        assert!(
            (returning(close, &row, asteroid(), strayed(close)).length() - cruise(1.0)).abs()
                < 1e-9
        );
    }

    #[test]
    fn return_inside_the_zone_is_the_damping_that_stops_a_drift() {
        let row = row();
        let drifting = Body::new(
            out(Belt::ZONE_RADIUS_METERS - 1.0),
            Vec3::new(0.3, 0.0, 0.0),
        );
        let braking = returning(drifting, &row, asteroid(), strayed(drifting));
        assert!(
            braking.distance(Vec3::new(-0.3, 0.0, 0.0)) < 1e-12,
            "{braking:?}"
        );

        let still = Body::new(out(Belt::ZONE_RADIUS_METERS - 1.0), Vec3::ZERO);
        assert!(returning(still, &row, asteroid(), strayed(still)).length() < 1e-12);
    }

    #[test]
    fn return_pushes_out_of_the_asteroid_the_deeper_the_harder() {
        let row = row();
        let shallow = Body::new(out(floor() - 0.2), Vec3::ZERO);
        let deep = Body::new(out(0.1 * floor()), Vec3::ZERO);
        let at_floor = Body::new(out(floor()), Vec3::ZERO);

        let shallow_pull = returning(shallow, &row, asteroid(), strayed(shallow));
        let deep_pull = returning(deep, &row, asteroid(), strayed(deep));
        assert!(
            shallow_pull.z > 0.0,
            "it is pushed into the asteroid: {shallow_pull:?}"
        );
        assert!(
            deep_pull.z > shallow_pull.z,
            "{deep_pull:?} against {shallow_pull:?}"
        );
        assert!(
            returning(at_floor, &row, asteroid(), strayed(at_floor)).length() < 1e-12,
            "the floor itself is pushed"
        );
    }

    #[test]
    fn a_term_toward_a_place_matches_the_places_own_velocity_once_it_is_there() {
        let row = row();
        let carried = out(7.0);
        let arrived = Body::new(Vec3::ZERO, carried);
        let here = Body::new(Vec3::ZERO, carried);
        assert!(chase(arrived, &row, here).length() < 1e-12);
    }

    #[test]
    fn separation_pushes_only_inside_the_spacing_and_is_bounded_by_its_weight() {
        let row = row();
        let unit = Body::new(Vec3::ZERO, Vec3::ZERO);
        let near = Vec3::new(0.25 * Belt::SPACING_METERS, 0.0, 0.0);
        let push = separation(unit, &row, [near].into_iter());
        assert!(push.x < 0.0, "{push:?}");
        assert!(push.length() <= 1.0);

        let apart = Vec3::new(Belt::SPACING_METERS, 0.0, 0.0);
        assert_eq!(separation(unit, &row, [apart].into_iter()), Vec3::ZERO);
        assert_eq!(separation(unit, &row, [Vec3::ZERO].into_iter()), Vec3::ZERO);
    }

    #[test]
    fn caution_is_nothing_where_no_power_stands_and_grows_as_the_enemy_dominates() {
        let row = row();
        assert_eq!(caution(&row, Sample::default()), Vec3::ZERO);

        let outnumbered = Sample {
            own: 1.0,
            enemy: 9.0,
            own_gradient: Vec3::ZERO,
            enemy_gradient: out(4.0),
        };
        let evened = Sample {
            enemy: 1.0,
            ..outnumbered
        };
        let away = caution(&row, outnumbered);
        assert!(away.z < 0.0, "{away:?}");
        assert!(away.length() > caution(&row, evened).length());
    }

    #[test]
    fn cohesion_climbs_the_own_gradient_by_how_lopsided_the_field_is() {
        let row = row();
        let even = Sample {
            own: 100.0,
            own_gradient: out(1e-9),
            ..Sample::default()
        };
        let lopsided = Sample {
            own_gradient: out(4.0),
            ..even
        };
        assert!(cohesion(&row, even).length() < 1e-6);
        assert!(cohesion(&row, lopsided).z > cohesion(&row, even).z);
        assert!(cohesion(&row, lopsided).length() <= 1.0);
    }

    #[test]
    fn cohesion_is_nothing_where_no_power_of_its_own_stands_and_never_exceeds_its_weight() {
        let row = row();
        assert_eq!(cohesion(&row, Sample::default()), Vec3::ZERO);

        let steep = Sample {
            own: 1e-6,
            own_gradient: out(1e6),
            ..Sample::default()
        };
        assert!((cohesion(&row, steep).length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_drift_is_a_bounded_smooth_function_of_the_tick_and_the_id() {
        let id = EntityId(7);
        assert_eq!(drift(Time(41), id), drift(Time(41), id));
        assert_ne!(drift(Time(41), id), drift(Time(41), EntityId(8)));
        for step in 0..2000 {
            let tick = Time(step * 13);
            let here = drift(tick, id);
            assert!(here.length() <= 3.0_f64.sqrt(), "{here:?}");
            assert!(here.distance(drift(tick.next(), id)) < 0.01, "{here:?}");
        }
    }

    #[test]
    fn the_drift_turns_all_the_way_round_within_a_minute() {
        let id = EntityId(3);
        let start = drift(Time::ZERO, id);
        let minute = 60 * u64::from(TICKS_PER_SECOND);
        let away = (0..minute)
            .map(|at| drift(Time(at), id).distance(start))
            .fold(0.0_f64, f64::max);
        assert!(
            away > 1.0,
            "the drift only reached {away} from where it began"
        );
    }
}
