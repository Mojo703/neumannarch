use core::f64::consts::TAU;

use crate::orbit::body::{Body, Gravity};
use crate::real::Real;
use crate::time::Tick;
use crate::vec3::Vec3;

const MAX_ITERATIONS: u32 = 40;

const TOLERANCE: f64 = 1e-14;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Orbit {
    a: Real,
    h: Real,
    k: Real,
    p: Real,
    q: Real,
    lambda0: Real,
    epoch: Tick,
}

impl Orbit {
    pub fn new(a: f64, h: f64, k: f64, p: f64, q: f64, lambda0: f64, epoch: Tick) -> Option<Orbit> {
        let elliptic = a > 0.0 && h * h + k * k < 1.0;
        (elliptic && p.is_finite() && q.is_finite() && lambda0.is_finite()).then(|| Orbit {
            a: Real(a),
            h: Real(h),
            k: Real(k),
            p: Real(p),
            q: Real(q),
            lambda0: Real(lambda0.rem_euclid(TAU)),
            epoch,
        })
    }

    pub fn from_body(body: Body, tick: Tick, gravity: Gravity) -> Option<Orbit> {
        if body.specific_energy(gravity) >= 0.0 {
            return None;
        }
        let mu = gravity.mu();
        let (pos, vel) = (to_reference(body.pos), to_reference(body.vel));
        let normal = pos.cross(vel).normalized()?;
        let tilt = 1.0 + normal.z;
        if tilt <= 0.0 {
            return None;
        }
        let (p, q) = (normal.x / tilt, -normal.y / tilt);
        let (f, g) = axes(p, q);
        let radius = pos.length();
        let eccentricity =
            (pos * (vel.length_squared() - mu / radius) - vel * pos.dot(vel)) * (1.0 / mu);
        let (h, k) = (eccentricity.dot(g), eccentricity.dot(f));
        let a = body.semi_major_axis(gravity);
        let longitude = eccentric_longitude_of(pos.dot(f) / a + k, pos.dot(g) / a + h, h, k);
        Orbit::new(a, h, k, p, q, mean_longitude(longitude, h, k), tick)
    }

    pub fn at(&self, tick: Tick, gravity: Gravity) -> Body {
        let (a, h, k) = (self.a.0, self.h.0, self.k.0);
        let mu = gravity.mu();
        let rate = (mu / (a * a * a)).sqrt();
        let span = tick.seconds() - self.epoch.seconds();
        let longitude = self.eccentric_longitude(self.lambda0.0 + rate * span);
        let (sin, cos) = (libm::sin(longitude), libm::cos(longitude));
        let beta = beta(h, k);
        let (f, g) = axes(self.p.0, self.q.0);
        let along = a * ((1.0 - h * h * beta) * cos + h * k * beta * sin - k);
        let across = a * ((1.0 - k * k * beta) * sin + h * k * beta * cos - h);
        let radius = a * (1.0 - k * cos - h * sin);
        let speed = (mu * a).sqrt() / radius;
        let along_rate = speed * (h * k * beta * cos - (1.0 - h * h * beta) * sin);
        let across_rate = speed * ((1.0 - k * k * beta) * cos - h * k * beta * sin);
        Body::new(
            to_belt(f * along + g * across),
            to_belt(f * along_rate + g * across_rate),
        )
    }

    pub fn shifted(&self, along: f64) -> Orbit {
        Orbit {
            lambda0: Real((self.lambda0.0 + along / self.a.0).rem_euclid(TAU)),
            ..*self
        }
    }

    pub fn period(&self, gravity: Gravity) -> f64 {
        let a = self.a.0;
        TAU * (a * a * a / gravity.mu()).sqrt()
    }

    pub fn semi_major_axis(&self) -> f64 {
        self.a.0
    }

    pub fn eccentricity(&self) -> f64 {
        let (h, k) = (self.h.0, self.k.0);
        (h * h + k * k).sqrt()
    }

    pub fn epoch(&self) -> Tick {
        self.epoch
    }

    fn eccentric_longitude(&self, lambda: f64) -> f64 {
        let (h, k) = (self.h.0, self.k.0);
        let mut longitude = lambda;
        for _ in 0..MAX_ITERATIONS {
            let (sin, cos) = (libm::sin(longitude), libm::cos(longitude));
            let step = (longitude + h * cos - k * sin - lambda) / (1.0 - h * sin - k * cos);
            longitude -= step;
            if step.abs() <= TOLERANCE {
                break;
            }
        }
        longitude
    }
}

fn axes(p: f64, q: f64) -> (Vec3, Vec3) {
    let scale = 1.0 / (1.0 + p * p + q * q);
    (
        Vec3::new(1.0 - p * p + q * q, 2.0 * p * q, -2.0 * p) * scale,
        Vec3::new(2.0 * p * q, 1.0 + p * p - q * q, 2.0 * q) * scale,
    )
}

fn beta(h: f64, k: f64) -> f64 {
    1.0 / (1.0 + (1.0 - h * h - k * k).sqrt())
}

fn eccentric_longitude_of(along: f64, across: f64, h: f64, k: f64) -> f64 {
    let root = (1.0 - h * h - k * k).sqrt();
    let beta = beta(h, k);
    let cos = ((1.0 - k * k * beta) * along - h * k * beta * across) / root;
    let sin = ((1.0 - h * h * beta) * across - h * k * beta * along) / root;
    libm::atan2(sin, cos)
}

fn mean_longitude(longitude: f64, h: f64, k: f64) -> f64 {
    longitude + h * libm::cos(longitude) - k * libm::sin(longitude)
}

fn to_belt(vector: Vec3) -> Vec3 {
    Vec3::new(vector.x, vector.z, -vector.y)
}

fn to_reference(vector: Vec3) -> Vec3 {
    Vec3::new(vector.x, -vector.z, vector.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbit::universal::propagate;

    const MU: Gravity = Gravity::new(4.0e13);

    const RADIUS: f64 = 1.0e7;

    const EPOCH: Tick = Tick(7_000);

    fn circular_speed() -> f64 {
        (MU.mu() / RADIUS).sqrt()
    }

    fn circular_equatorial() -> Body {
        Body::new(
            Vec3::new(RADIUS, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -circular_speed()),
        )
    }

    fn inclined_eccentric() -> Body {
        Body::new(
            Vec3::new(RADIUS, 0.0, 0.0),
            Vec3::new(0.1, 0.2, -1.0) * (0.9 * circular_speed()),
        )
    }

    fn tilted_circle() -> Body {
        Body::new(
            Vec3::new(0.0, 0.0, -RADIUS),
            Vec3::new(-1.0, 0.05, 0.0)
                .normalized()
                .expect("a direction")
                * circular_speed(),
        )
    }

    fn assert_same_body(found: Body, expected: Body, what: &str) {
        assert!(
            found.pos.distance(expected.pos) < 1e-8 * expected.radius(),
            "{what}: position {found:?} is not {expected:?}"
        );
        assert!(
            found.vel.distance(expected.vel) < 1e-8 * expected.speed(),
            "{what}: velocity {found:?} is not {expected:?}"
        );
    }

    #[test]
    fn a_body_read_back_at_its_own_tick_is_itself() {
        for (name, body) in [
            ("circular equatorial", circular_equatorial()),
            ("inclined eccentric", inclined_eccentric()),
            ("tilted circle", tilted_circle()),
        ] {
            let orbit = Orbit::from_body(body, EPOCH, MU).expect("a bound body");
            assert_eq!(orbit.epoch(), EPOCH);
            assert_same_body(orbit.at(EPOCH, MU), body, name);
        }
    }

    #[test]
    fn a_circular_equatorial_orbit_has_no_singular_element() {
        let orbit = Orbit::from_body(circular_equatorial(), EPOCH, MU).expect("a bound body");
        assert!(orbit.eccentricity() < 1e-12, "{orbit:?}");
        assert!(
            orbit.p.0.abs() < 1e-12 && orbit.q.0.abs() < 1e-12,
            "{orbit:?}"
        );
        assert!((orbit.semi_major_axis() - RADIUS).abs() < 1e-6 * RADIUS);
    }

    #[test]
    fn an_inclined_eccentric_orbit_is_neither_circular_nor_flat() {
        let orbit = Orbit::from_body(inclined_eccentric(), EPOCH, MU).expect("a bound body");
        assert!(orbit.eccentricity() > 0.1, "{orbit:?}");
        assert!(orbit.p.0.abs() + orbit.q.0.abs() > 0.05, "{orbit:?}");
    }

    #[test]
    fn an_orbit_at_one_period_is_back_at_its_start() {
        let period = 30_000.0;
        let gravity = Gravity::new(TAU * TAU * RADIUS * RADIUS * RADIUS / (period * period));
        let orbit = Orbit::new(RADIUS, 0.1, -0.05, 0.02, 0.03, 1.0, Tick::ZERO)
            .expect("an eccentric inclined ellipse");
        assert!((orbit.period(gravity) - period).abs() < 1e-6 * period);
        let start = orbit.at(Tick::ZERO, gravity);
        let ticks = period as u64 * u64::from(crate::TICKS_PER_SECOND);
        let round = orbit.at(Tick(ticks), gravity);
        assert_same_body(round, start, "one period on");
    }

    #[test]
    fn kepler_agrees_with_the_universal_propagator() {
        for (name, body) in [
            ("circular equatorial", circular_equatorial()),
            ("inclined eccentric", inclined_eccentric()),
            ("tilted circle", tilted_circle()),
        ] {
            let orbit = Orbit::from_body(body, EPOCH, MU).expect("a bound body");
            for from in [Tick(0), EPOCH, Tick(500_000)] {
                let start = orbit.at(from, MU);
                for ticks in [1, 1_000, 120_000, 1_200_000, 3_000_000] {
                    let span = Tick(ticks).seconds();
                    let flown = propagate(start, MU, span);
                    let read = orbit.at(Tick(from.0 + ticks), MU);
                    assert_same_body(read, flown, &format!("{name} from {from:?} for {ticks}"));
                }
            }
        }
    }

    #[test]
    fn an_unbound_radial_or_retrograde_body_has_no_orbit() {
        let escape = Body::new(
            Vec3::new(RADIUS, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -circular_speed() * 2f64.sqrt()),
        );
        let radial = Body::new(
            Vec3::new(RADIUS, 0.0, 0.0),
            Vec3::new(0.5 * circular_speed(), 0.0, 0.0),
        );
        let retrograde = Body::new(
            Vec3::new(RADIUS, 0.0, 0.0),
            Vec3::new(0.0, 0.0, circular_speed()),
        );
        for body in [escape, radial, retrograde] {
            assert_eq!(Orbit::from_body(body, EPOCH, MU), None, "{body:?}");
        }
    }

    #[test]
    fn elements_that_are_not_an_ellipse_are_not_an_orbit() {
        assert!(Orbit::new(RADIUS, 0.1, 0.1, 0.0, 0.0, 0.0, EPOCH).is_some());
        assert_eq!(Orbit::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, EPOCH), None);
        assert_eq!(Orbit::new(-RADIUS, 0.0, 0.0, 0.0, 0.0, 0.0, EPOCH), None);
        assert_eq!(Orbit::new(RADIUS, 0.8, 0.8, 0.0, 0.0, 0.0, EPOCH), None);
        assert_eq!(
            Orbit::new(RADIUS, 0.0, 0.0, 0.0, 0.0, f64::NAN, EPOCH),
            None
        );
    }

    #[test]
    fn a_shifted_orbit_leads_by_the_shift_and_keeps_station() {
        let orbit = Orbit::from_body(circular_equatorial(), Tick::ZERO, MU).expect("bound");
        let lead = 30.0;
        let shifted = orbit.shifted(lead);
        assert_eq!(shifted.period(MU), orbit.period(MU));
        for tick in [Tick::ZERO, Tick(120), Tick(1_000_000)] {
            let (rock, anchor) = (orbit.at(tick, MU), shifted.at(tick, MU));
            let along = anchor.pos.distance(rock.pos);
            assert!((along - lead).abs() < 1e-3 * lead, "{tick:?}: {along}");
            let ahead = (anchor.pos - rock.pos).dot(rock.vel);
            assert!(ahead > 0.0, "{tick:?}: the anchor is not ahead");
        }
    }

    #[test]
    fn the_mean_longitude_is_stated_within_one_turn() {
        let orbit = Orbit::new(RADIUS, 0.0, 0.0, 0.0, 0.0, -TAU * 3.25, EPOCH).expect("elliptic");
        assert!((orbit.lambda0.0 - 0.75 * TAU).abs() < 1e-12, "{orbit:?}");
    }
}
