use crate::orbit::body::{Body, Gravity};
use crate::orbit::stumpff::{c2, c3};

const MAX_ITERATIONS: u32 = 60;

const TOLERANCE: f64 = 1e-14;

const PARABOLIC_BOUND: f64 = 1e-6;

struct Problem {
    root_mu: f64,
    r0: f64,
    sigma: f64,
    alpha: f64,
    target: f64,
    p_cubed_root: f64,
}

pub fn propagate(body: Body, gravity: Gravity, dt: f64) -> Body {
    let problem = Problem::new(body, gravity, dt);
    let chi = problem.solve();
    let psi = chi * chi * problem.alpha;
    let (c2, c3) = (c2(psi), c3(psi));
    let f = 1.0 - chi * chi * c2 / problem.r0;
    let g = dt - chi * chi * chi * c3 / problem.root_mu;
    let pos = body.pos * f + body.vel * g;
    let r = pos.length();
    let f_dot = problem.root_mu / (r * problem.r0) * chi * (psi * c3 - 1.0);
    let g_dot = 1.0 - chi * chi * c2 / r;
    Body::new(pos, body.pos * f_dot + body.vel * g_dot)
}

impl Problem {
    fn new(body: Body, gravity: Gravity, dt: f64) -> Problem {
        let root_mu = gravity.mu().sqrt();
        let p = body.angular_momentum().length_squared() / gravity.mu();
        Problem {
            root_mu,
            r0: body.radius(),
            sigma: body.pos.dot(body.vel) / root_mu,
            alpha: -2.0 * body.specific_energy(gravity) / gravity.mu(),
            target: root_mu * dt,
            p_cubed_root: p * p.sqrt(),
        }
    }

    fn solve(&self) -> f64 {
        let mut chi = self.initial_chi();
        for _ in 0..MAX_ITERATIONS {
            let next = self.newton_step(chi);
            let converged = (next - chi).abs() <= TOLERANCE * next.abs();
            chi = next;
            if converged {
                break;
            }
        }
        chi
    }

    fn newton_step(&self, chi: f64) -> f64 {
        let psi = chi * chi * self.alpha;
        let (c2, c3) = (c2(psi), c3(psi));
        let radius =
            chi * chi * c2 + self.sigma * chi * (1.0 - psi * c3) + self.r0 * (1.0 - psi * c2);
        let reached =
            chi * chi * chi * c3 + self.sigma * chi * chi * c2 + self.r0 * chi * (1.0 - psi * c3);
        chi + (self.target - reached) / radius
    }

    fn initial_chi(&self) -> f64 {
        let shape = self.alpha * self.r0;
        if shape > PARABOLIC_BOUND {
            self.target * self.alpha
        } else if shape < -PARABOLIC_BOUND {
            self.hyperbolic_chi(shape)
        } else {
            self.parabolic_chi()
        }
    }

    fn hyperbolic_chi(&self, shape: f64) -> f64 {
        let sign = self.target.signum();
        let root_a = (-1.0 / self.alpha).sqrt();
        let ratio = -self.alpha * self.target / (self.sigma + sign * root_a * (1.0 - shape));
        sign * root_a * libm::asinh(ratio)
    }

    fn parabolic_chi(&self) -> f64 {
        let span = 3.0 * self.target;
        let hypot = (self.p_cubed_root * self.p_cubed_root + span * span).sqrt();
        libm::cbrt(hypot + span) - libm::cbrt(hypot - span)
    }
}

#[cfg(test)]
mod tests {
    use core::f64::consts::TAU;

    use super::*;
    use crate::Vec3;

    const MU: Gravity = Gravity::new(1.0);

    fn circular() -> Body {
        Body::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0))
    }

    fn elliptic() -> Body {
        Body::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.3, 1.2))
    }

    fn hyperbolic() -> Body {
        Body::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0 * 2f64.sqrt()),
        )
    }

    fn parabolic() -> Body {
        Body::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 2f64.sqrt()))
    }

    fn radial_parabolic() -> Body {
        Body::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(2f64.sqrt(), 0.0, 0.0))
    }

    #[test]
    fn a_circular_orbit_returns_to_its_start_after_one_period() {
        let start = circular();
        let period = start.period(MU).expect("bound");
        assert!((period - TAU).abs() < 1e-12);
        let end = propagate(start, MU, period);
        assert!(end.pos.distance(start.pos) < 1e-9 * start.radius());
    }

    #[test]
    fn energy_and_angular_momentum_hold_over_a_thousand_steps() {
        let start = elliptic();
        let step = start.period(MU).expect("bound") / 100.0;
        let energy = start.specific_energy(MU);
        let momentum = start.angular_momentum();
        let mut body = start;
        for _ in 0..1000 {
            body = propagate(body, MU, step);
            assert!((body.specific_energy(MU) - energy).abs() < 1e-10 * energy.abs());
            assert!(body.angular_momentum().distance(momentum) < 1e-10 * momentum.length());
        }
    }

    #[test]
    fn propagating_back_returns_the_start_on_every_conic() {
        for start in [
            circular(),
            elliptic(),
            hyperbolic(),
            parabolic(),
            radial_parabolic(),
        ] {
            for dt in [0.0, 0.01, 1.0, 100.0] {
                let out = propagate(start, MU, dt);
                let back = propagate(out, MU, -dt);
                assert!(back.pos.distance(start.pos) < 1e-9, "{start:?} dt {dt}");
                assert!(back.vel.distance(start.vel) < 1e-9, "{start:?} dt {dt}");
            }
        }
    }

    #[test]
    fn a_hyperbolic_body_recedes_with_constant_positive_energy() {
        let start = hyperbolic();
        let energy = start.specific_energy(MU);
        assert!(energy > 0.0);
        let mut last = start;
        for dt in [0.1, 1.0, 10.0, 1000.0] {
            let body = propagate(start, MU, dt);
            assert!(body.radius() > last.radius());
            assert!((body.specific_energy(MU) - energy).abs() < 1e-10 * energy);
            last = body;
        }
    }
}
