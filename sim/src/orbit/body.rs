use core::f64::consts::TAU;

use crate::Vec3;
use crate::orbit::universal::propagate;
use crate::real::Real;
use crate::time::Time;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Body {
    pub pos: Vec3,
    pub(crate) vel: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gravity(Real);

impl Body {
    pub const fn new(pos: Vec3, vel: Vec3) -> Body {
        Body { pos, vel }
    }

    pub fn after_tick(self, thrust: Vec3, gravity: Gravity) -> Body {
        let span = Time(1).seconds();
        propagate(Body::new(self.pos, self.vel + thrust * span), gravity, span)
    }

    pub(crate) fn radius(self) -> f64 {
        self.pos.length()
    }

    #[cfg(test)]
    pub(crate) fn speed(self) -> f64 {
        self.vel.length()
    }

    pub fn specific_energy(self, gravity: Gravity) -> f64 {
        self.vel.length_squared() / 2.0 - gravity.mu() / self.radius()
    }

    pub(crate) fn angular_momentum(self) -> Vec3 {
        self.pos.cross(self.vel)
    }

    pub fn semi_major_axis(self, gravity: Gravity) -> f64 {
        -gravity.mu() / (2.0 * self.specific_energy(gravity))
    }

    pub fn period(self, gravity: Gravity) -> Option<f64> {
        let a = self.semi_major_axis(gravity);
        (self.specific_energy(gravity) < 0.0).then(|| TAU * (a * a * a / gravity.mu()).sqrt())
    }
}

impl Gravity {
    pub const fn new(mu: f64) -> Gravity {
        Gravity(Real(mu))
    }

    pub fn mu(self) -> f64 {
        self.0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MU: Gravity = Gravity::new(1.0);

    #[test]
    fn a_circular_orbit_has_energy_minus_half_mu_over_r() {
        let body = Body::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 0.5f64.sqrt(), 0.0));
        let energy = body.specific_energy(MU);
        assert!((energy + 0.25).abs() < 1e-15);
        assert_eq!(body.radius(), 2.0);
        assert_eq!(body.speed(), 0.5f64.sqrt());
    }

    #[test]
    fn a_tick_under_thrust_is_the_free_fall_tick_plus_the_thrusts_delta_v() {
        let orbiting = Gravity::new(4.4e17);
        let body = Body::new(Vec3::new(1.0e7, 0.0, 0.0), Vec3::new(0.0, 0.0, -6.63e3));
        let span = Time(1).seconds();
        let thrust = Vec3::new(0.0, 4.0, 3.0);

        let falling = body.after_tick(Vec3::ZERO, orbiting);
        let thrusting = body.after_tick(thrust, orbiting);

        assert_eq!(falling, propagate(body, orbiting, span));
        let gained = thrusting.vel - falling.vel;
        assert!(
            gained.distance(thrust * span) < 1e-9,
            "a tick of {thrust:?} gained {gained:?}"
        );
    }

    #[test]
    fn angular_momentum_is_normal_to_the_orbit_plane() {
        let body = Body::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(body.angular_momentum(), Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn only_bound_bodies_have_a_period() {
        let circular = Body::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
        let hyperbolic = Body::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0 * 2f64.sqrt()),
        );
        let parabolic = Body::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 2f64.sqrt()));
        assert!((circular.period(MU).expect("bound") - TAU).abs() < 1e-12);
        assert_eq!(hyperbolic.period(MU), None);
        assert_eq!(parabolic.period(MU), None);
        assert!(hyperbolic.semi_major_axis(MU) < 0.0);
        assert!((circular.semi_major_axis(MU) - 1.0).abs() < 1e-15);
    }
}
