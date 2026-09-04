use core::f64::consts::TAU;

use crate::orbit::body::{Body, Gravity};
use crate::orbit::stumpff::{c2, c3};
use crate::vec3::Vec3;

const BISECTIONS: u32 = 80;

const PSI_BOUND: f64 = TAU * TAU;

const COLLINEAR_BOUND: f64 = 1e-9;

const SPAN_TOLERANCE: f64 = 1e-9;

pub fn solve(from: Body, to: Body, dt: f64, gravity: Gravity) -> Option<[Vec3; 2]> {
    if dt <= 0.0 || dt.is_nan() {
        return None;
    }
    let problem = Problem::new(from, to, dt, gravity)?;
    let psi = problem.bisect();
    let (radius, chord) = problem.chord(psi)?;
    let g = problem.chord_coefficient * (radius / problem.mu).sqrt();
    let start = (to.pos - from.pos * (1.0 - radius / problem.from_radius)) * (1.0 / g);
    let end = (to.pos * (1.0 - radius / problem.to_radius) - from.pos) * (1.0 / g);
    ((chord - dt).abs() <= SPAN_TOLERANCE * dt).then_some([start - from.vel, to.vel - end])
}

struct Problem {
    mu: f64,
    from_radius: f64,
    to_radius: f64,
    chord_coefficient: f64,
    span: f64,
}

impl Problem {
    fn new(from: Body, to: Body, dt: f64, gravity: Gravity) -> Option<Problem> {
        let sense = from.pos.cross(to.pos).normalized()?;
        let (from_radius, to_radius) = (from.radius(), to.radius());
        let cosine = from.pos.dot(to.pos) / (from_radius * to_radius);
        if 1.0 + cosine <= COLLINEAR_BOUND {
            return None;
        }
        let way = if sense.y >= 0.0 { 1.0 } else { -1.0 };
        Some(Problem {
            mu: gravity.mu(),
            from_radius,
            to_radius,
            chord_coefficient: way * (from_radius * to_radius * (1.0 + cosine)).sqrt(),
            span: dt,
        })
    }

    fn chord(&self, psi: f64) -> Option<(f64, f64)> {
        let (c2, c3) = (c2(psi), c3(psi));
        let radius = self.from_radius
            + self.to_radius
            + self.chord_coefficient * (psi * c3 - 1.0) / c2.sqrt();
        if radius <= 0.0 || radius.is_nan() {
            return None;
        }
        let chi = (radius / c2).sqrt();
        let span = (chi * chi * chi * c3 + self.chord_coefficient * radius.sqrt()) / self.mu.sqrt();
        Some((radius, span))
    }

    fn bisect(&self) -> f64 {
        let (mut low, mut high) = (-PSI_BOUND, PSI_BOUND);
        let mut psi = 0.0;
        for _ in 0..BISECTIONS {
            psi = 0.5 * (low + high);
            match self.chord(psi) {
                Some((_, span)) if span.is_finite() && span <= self.span => low = psi,
                Some(_) => high = psi,
                None => low = psi,
            }
        }
        psi
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbit::universal::propagate;

    const MU: Gravity = Gravity::new(4.0e13);

    const RADIUS: f64 = 1.0e7;

    fn circular(radius: f64, turn: f64) -> Body {
        let speed = (MU.mu() / radius).sqrt();
        let (sin, cos) = (libm::sin(turn), libm::cos(turn));
        Body::new(
            Vec3::new(radius * cos, 0.0, -radius * sin),
            Vec3::new(-speed * sin, 0.0, -speed * cos),
        )
    }

    #[test]
    fn the_first_impulse_flies_the_body_to_the_target() {
        for (turn, span) in [(0.05, 400.0), (0.5, 4_000.0), (2.0, 12_000.0)] {
            let from = circular(RADIUS, 0.0);
            let to = circular(RADIUS * 1.3, turn);
            let [start, _] = solve(from, to, span, MU).expect("a transfer");
            let flown = propagate(Body::new(from.pos, from.vel + start), MU, span);
            assert!(
                flown.pos.distance(to.pos) < 1e-6 * to.radius(),
                "turn {turn}: {flown:?} is not {to:?}"
            );
        }
    }

    #[test]
    fn the_second_impulse_leaves_the_body_on_the_target_orbit() {
        let from = circular(RADIUS, 0.0);
        let to = circular(RADIUS * 1.3, 0.5);
        let span = 4_000.0;
        let [start, end] = solve(from, to, span, MU).expect("a transfer");
        let flown = propagate(Body::new(from.pos, from.vel + start), MU, span);
        assert!(
            (flown.vel + end).distance(to.vel) < 1e-6 * to.speed(),
            "{flown:?} does not match {to:?}"
        );
    }

    #[test]
    fn a_near_half_turn_transfer_matches_the_hohmann_impulses() {
        let (inner, outer) = (RADIUS, RADIUS * 2.0);
        let semi_major = 0.5 * (inner + outer);
        let period = TAU * (semi_major * semi_major * semi_major / MU.mu()).sqrt();
        let first = (MU.mu() / inner).sqrt() * ((2.0 * outer / (inner + outer)).sqrt() - 1.0);
        let second = (MU.mu() / outer).sqrt() * (1.0 - (2.0 * inner / (inner + outer)).sqrt());
        for turn in [0.99, 1.01] {
            let from = circular(inner, 0.0);
            let to = circular(outer, turn * core::f64::consts::PI);
            let [start, end] = solve(from, to, 0.5 * period, MU).expect("a transfer");
            assert!(
                (start.length() - first).abs() < 1e-2 * first,
                "{turn}: {start:?}"
            );
            assert!(
                (end.length() - second).abs() < 1e-2 * second,
                "{turn}: {end:?}"
            );
        }
    }

    #[test]
    fn a_collinear_case_has_no_transfer() {
        let from = circular(RADIUS, 0.0);
        let same = circular(RADIUS * 1.3, 0.0);
        let opposite = circular(RADIUS * 1.3, core::f64::consts::PI);
        assert_eq!(solve(from, same, 1_000.0, MU), None);
        assert_eq!(solve(from, opposite, 1_000.0, MU), None);
        assert_eq!(solve(from, opposite, 4_000.0, MU), None);
    }

    #[test]
    fn a_span_no_single_revolution_transfer_takes_has_no_answer() {
        let from = circular(RADIUS, 0.0);
        let to = circular(RADIUS * 1.3, 0.5);
        assert_eq!(solve(from, to, 0.0, MU), None);
        assert_eq!(solve(from, to, -1_000.0, MU), None);
        assert_eq!(solve(from, to, 1e-6, MU), None);
    }
}
