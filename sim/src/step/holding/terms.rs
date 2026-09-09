use core::f64::consts::TAU;

use crate::belt::Belt;
use crate::ids::EntityId;
use crate::orbit::body::Body;
use crate::real::Real;
use crate::roster::Row;
use crate::time::Time;
use crate::vec3::Vec3;

const DRIFT: [(f64, f64); 3] = [(0.037, 0.0), (0.053, 1.0), (0.071, 2.0)];

const DRIFT_PHASES: u32 = 1024;

pub(crate) fn wander(row: &Row, at: Time, id: EntityId) -> Vec3 {
    drift(at, id) * row.steering.wander.0
}

pub(crate) fn separation(body: Body, row: &Row, neighbours: impl Iterator<Item = Vec3>) -> Vec3 {
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

pub(crate) fn returning(body: Body, row: &Row, asteroid: Body, into_shell: Vec3) -> Vec3 {
    toward(body, row, row.steering.returning, asteroid, into_shell)
}

pub(crate) fn stationing(body: Body, row: &Row, station: Body) -> Vec3 {
    toward(
        body,
        row,
        row.steering.station,
        station,
        station.pos - body.pos,
    )
}

fn toward(body: Body, row: &Row, weight: Real, place: Body, offset: Vec3) -> Vec3 {
    let speed = (2.0 * row.manoeuvring.0 * offset.length().min(Belt::ARRIVAL_METERS)).sqrt();
    let wanted = offset
        .normalized()
        .map_or(Vec3::ZERO, |along| along * speed);
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
    use crate::fixture::World;
    use crate::ids::{AsteroidId, TeamId};
    use crate::materials::Materials;
    use crate::orbit::body::Gravity;
    use crate::roster::Weights;
    use crate::roster::{Role, Tier};

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: AsteroidId = AsteroidId(0);

    const LIMIT: f64 = 1.25;

    const WANDER: f64 = 0.25;

    const RETURNING: f64 = 2.0;

    const SEPARATION: f64 = 0.5;

    const STATION: f64 = 4.0;

    fn world() -> World {
        World::ring(GRAVITY, 1, &[TeamId(0)])
    }

    fn row() -> Row {
        Row {
            name: "test",
            role: Role::Scout,
            tier: Tier::ONE,
            cost: Materials::new(1.0, 0.0, 0.0),
            manoeuvring: Real(LIMIT),
            steering: Weights {
                wander: Real(WANDER),
                returning: Real(RETURNING),
                separation: Real(SEPARATION),
                station: Real(STATION),
            },
            hp: Real(1.0),
            plating: Real(0.0),
            capacity: Materials::ZERO,
            effects: vec![],
        }
    }

    fn floor(world: &World) -> f64 {
        world.state[HOME].radius() + Belt::SPACING_METERS
    }

    fn out(world: &World, meters: f64) -> Body {
        let asteroid = world.state.asteroid_body(HOME);
        Body::new(
            asteroid.pos + Vec3::new(0.0, meters, 0.0),
            world.state.asteroid_body(HOME).vel,
        )
    }

    fn homeward(world: &World, unit: Body) -> Vec3 {
        let asteroid = world.state.asteroid_body(HOME);
        returning(
            unit,
            &row(),
            asteroid,
            world.state[HOME].toward_shell(asteroid, unit.pos),
        )
    }

    fn cruise(over: f64) -> f64 {
        (2.0 * LIMIT * over.min(Belt::ARRIVAL_METERS)).sqrt()
    }

    #[test]
    fn the_desired_speed_is_what_the_limit_can_stop_from_within_the_arrival_distance() {
        let world = world();

        let far = homeward(&world, out(&world, 3.0 * Belt::ZONE_RADIUS_METERS));
        assert!(
            (far.length() - RETURNING * cruise(Belt::ARRIVAL_METERS)).abs() < 1e-9,
            "{far:?}"
        );

        let close = homeward(&world, out(&world, Belt::ZONE_RADIUS_METERS + 1.0));
        assert!(
            (close.length() - RETURNING * cruise(1.0)).abs() < 1e-9,
            "{close:?}"
        );
    }

    #[test]
    fn return_inside_the_zone_is_the_damping_that_stops_a_drift() {
        let world = world();
        let held = out(&world, Belt::ZONE_RADIUS_METERS - 1.0);
        let adrift = Body::new(held.pos, held.vel + Vec3::new(0.3, 0.0, 0.0));

        let braking = homeward(&world, adrift);

        assert!(
            braking.distance(Vec3::new(-0.3 * RETURNING, 0.0, 0.0)) < 1e-12,
            "{braking:?}"
        );
        assert!(homeward(&world, held).length() < 1e-12);
    }

    #[test]
    fn return_pushes_out_of_the_asteroid_the_deeper_the_harder() {
        let world = world();
        let floor = floor(&world);
        let shallow = homeward(&world, out(&world, floor - 0.2));
        let deep = homeward(&world, out(&world, 0.1 * floor));

        assert!(
            shallow.y > 0.0,
            "it is pushed into the asteroid: {shallow:?}"
        );
        assert!(deep.y > shallow.y, "{deep:?} against {shallow:?}");
        assert!(
            homeward(&world, out(&world, floor)).length() < 1e-12,
            "the floor itself is pushed"
        );
    }

    #[test]
    fn a_unit_at_its_station_matches_the_stations_own_velocity_and_asks_for_nothing_more() {
        let carried = Vec3::new(0.0, 0.0, 7.0);
        let station = Body::new(Vec3::ZERO, carried);

        assert!(stationing(Body::new(Vec3::ZERO, carried), &row(), station).length() < 1e-12);

        let away = Body::new(Vec3::new(0.0, 0.0, -2.0), carried);
        let pull = stationing(away, &row(), station);
        assert!(pull.z > 0.0, "{pull:?}");
        assert!(
            (pull.length() - STATION * cruise(2.0)).abs() < 1e-9,
            "{pull:?}"
        );
    }

    #[test]
    fn separation_pushes_away_by_how_far_inside_the_spacing_a_neighbour_is() {
        let unit = Body::new(Vec3::ZERO, Vec3::ZERO);
        let near = Vec3::new(0.25 * Belt::SPACING_METERS, 0.0, 0.0);

        let push = separation(unit, &row(), [near].into_iter());

        assert!(push.x < 0.0, "{push:?}");
        assert!(
            (push.length() - SEPARATION * 0.75).abs() < 1e-12,
            "{push:?}"
        );
        assert_eq!(
            separation(
                unit,
                &row(),
                [Vec3::new(Belt::SPACING_METERS, 0.0, 0.0)].into_iter()
            ),
            Vec3::ZERO
        );
        assert_eq!(
            separation(unit, &row(), [Vec3::ZERO].into_iter()),
            Vec3::ZERO
        );
    }

    #[test]
    fn each_term_is_scaled_by_the_rows_own_weight() {
        let world = world();
        let row = row();
        let doubled = Row {
            steering: Weights {
                wander: Real(2.0 * WANDER),
                returning: Real(2.0 * RETURNING),
                separation: Real(2.0 * SEPARATION),
                station: Real(2.0 * STATION),
            },
            ..row.clone()
        };
        let unit = out(&world, 2.0 * Belt::ZONE_RADIUS_METERS);
        let asteroid = world.state.asteroid_body(HOME);
        let into_shell = world.state[HOME].toward_shell(asteroid, unit.pos);
        let station = Body::new(asteroid.pos, asteroid.vel);
        let near = [unit.pos + Vec3::new(0.1, 0.0, 0.0)];

        let twice = |once: Vec3, twice: Vec3| twice.distance(once * 2.0) < 1e-12;

        assert!(twice(
            wander(&row, Time(11), EntityId(3)),
            wander(&doubled, Time(11), EntityId(3))
        ));
        assert!(twice(
            returning(unit, &row, asteroid, into_shell),
            returning(unit, &doubled, asteroid, into_shell)
        ));
        assert!(twice(
            separation(unit, &row, near.into_iter()),
            separation(unit, &doubled, near.into_iter())
        ));
        assert!(twice(
            stationing(unit, &row, station),
            stationing(unit, &doubled, station)
        ));
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
    fn the_drift_pushes_every_way_within_a_minute() {
        let id = EntityId(3);
        let minute = 60 * u64::from(TICKS_PER_SECOND);
        let drifts: Vec<Vec3> = (0..minute).map(|at| drift(Time(at), id)).collect();
        let along = [
            |push: &Vec3| push.x,
            |push: &Vec3| push.y,
            |push: &Vec3| push.z,
        ];

        for (axis, read) in along.into_iter().enumerate() {
            let most = drifts.iter().map(read).fold(f64::MIN, f64::max);
            let least = drifts.iter().map(read).fold(f64::MAX, f64::min);
            assert!(most > 0.99, "axis {axis} only reached {most} one way");
            assert!(least < -0.99, "axis {axis} only reached {least} the other");
        }
    }
}
