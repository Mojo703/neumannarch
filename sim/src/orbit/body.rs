//! A body in free fall about the central mass.

use core::f64::consts::TAU;

use crate::Vec3;
use crate::real::Real;

/// A body's state in the inertial frame centered on the central mass.
/// Equal and hashed by bit pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Body {
    /// Position, in meters.
    pub pos: Vec3,
    /// Velocity, in meters per second.
    pub vel: Vec3,
}

/// The central mass's gravitational parameter μ, in m³/s². A world
/// constant, held in state as it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gravity(Real);

impl Body {
    pub const fn new(pos: Vec3, vel: Vec3) -> Body {
        Body { pos, vel }
    }

    /// Distance from the central mass, in meters.
    pub fn radius(self) -> f64 {
        self.pos.length()
    }

    /// Speed, in meters per second.
    pub fn speed(self) -> f64 {
        self.vel.length()
    }

    /// Specific orbital energy, in J/kg: negative when bound, zero when
    /// parabolic, positive when hyperbolic.
    pub fn specific_energy(self, gravity: Gravity) -> f64 {
        self.vel.length_squared() / 2.0 - gravity.mu() / self.radius()
    }

    /// Specific angular momentum, in m²/s.
    pub fn angular_momentum(self) -> Vec3 {
        self.pos.cross(self.vel)
    }

    /// The semi-major axis in meters: negative for a hyperbola, infinite
    /// for a parabola.
    pub fn semi_major_axis(self, gravity: Gravity) -> f64 {
        -gravity.mu() / (2.0 * self.specific_energy(gravity))
    }

    /// The orbital period in seconds, or `None` when the body is unbound.
    pub fn period(self, gravity: Gravity) -> Option<f64> {
        let a = self.semi_major_axis(gravity);
        (self.specific_energy(gravity) < 0.0).then(|| TAU * (a * a * a / gravity.mu()).sqrt())
    }
}

impl Gravity {
    /// The gravitational parameter `mu`, in m³/s².
    pub const fn new(mu: f64) -> Gravity {
        Gravity(Real(mu))
    }

    /// The gravitational parameter μ, in m³/s².
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
