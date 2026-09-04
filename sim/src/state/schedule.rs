use core::num::NonZeroU32;

use crate::orbit::body::{Body, Gravity};
use crate::orbit::lambert;
use crate::orbit::universal::propagate;
use crate::place::Place;
use crate::time::Tick;
use crate::vec3::Vec3;

const CORRECTIONS: u32 = 3;

const BURN_SHARE_OF_SPAN: f64 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Flight {
    source: Place,
    schedule: Schedule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Schedule {
    burns: [Burn; 2],
    arrive: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Burn {
    from: Tick,
    ticks: NonZeroU32,
    accel: Vec3,
}

impl Flight {
    pub(crate) fn new(source: Place, schedule: Schedule) -> Flight {
        Flight { source, schedule }
    }

    pub fn source(self) -> Place {
        self.source
    }

    pub fn arrive(self) -> Tick {
        self.schedule.arrive
    }

    pub fn thrust(self, tick: Tick) -> Vec3 {
        self.schedule.thrust(tick)
    }
}

impl Schedule {
    pub const ARRIVAL_POSITION_METERS: f64 = 0.25;

    pub const ARRIVAL_SPEED_METERS_PER_SECOND: f64 = 0.02;

    pub(crate) fn between(
        source: Body,
        target: Body,
        depart: Tick,
        arrive: Tick,
        limit: f64,
        gravity: Gravity,
    ) -> Option<Schedule> {
        let span = Tick(arrive.0.checked_sub(depart.0)?).seconds();
        let mut aim = target;
        for _ in 0..=CORRECTIONS {
            let impulses = lambert::solve(source, aim, span, gravity)?;
            let schedule = Schedule::coasting(impulses, limit, depart, arrive)?;
            let flown = schedule.flown(source, gravity);
            let (off, drift) = (flown.pos - target.pos, flown.vel - target.vel);
            if off.length() <= Schedule::ARRIVAL_POSITION_METERS
                && drift.length() <= Schedule::ARRIVAL_SPEED_METERS_PER_SECOND
            {
                return Some(schedule);
            }
            aim = Body::new(aim.pos - off, aim.vel - drift);
        }
        None
    }

    pub fn arrive(self) -> Tick {
        self.arrive
    }

    pub(crate) fn thrust(self, tick: Tick) -> Vec3 {
        self.burns
            .iter()
            .find(|burn| burn.covers(tick))
            .map_or(Vec3::ZERO, |burn| burn.accel)
    }

    fn coasting(impulses: [Vec3; 2], limit: f64, depart: Tick, arrive: Tick) -> Option<Schedule> {
        let first = Burn::starting(impulses[0], limit, depart)?;
        let last = Burn::ending(impulses[1], limit, arrive)?;
        let span = arrive.0.checked_sub(depart.0)?;
        let burning = u64::from(first.ticks.get()) + u64::from(last.ticks.get());
        let spare = first.end() <= last.from && burning as f64 <= BURN_SHARE_OF_SPAN * span as f64;
        spare.then_some(Schedule {
            burns: [first, last],
            arrive,
        })
    }

    fn flown(self, from: Body, gravity: Gravity) -> Body {
        let [first, last] = self.burns;
        let departed = first.flown(from, gravity);
        let coast = Tick(last.from.0 - first.end().0).seconds();
        last.flown(propagate(departed, gravity, coast), gravity)
    }
}

impl Burn {
    fn starting(delta_v: Vec3, limit: f64, from: Tick) -> Option<Burn> {
        let ticks = Burn::ticks(delta_v, limit)?;
        Some(Burn {
            from,
            ticks,
            accel: Burn::held(delta_v, ticks),
        })
    }

    fn ending(delta_v: Vec3, limit: f64, at: Tick) -> Option<Burn> {
        let ticks = Burn::ticks(delta_v, limit)?;
        Some(Burn {
            from: Tick(at.0.checked_sub(u64::from(ticks.get()))?),
            ticks,
            accel: Burn::held(delta_v, ticks),
        })
    }

    fn ticks(delta_v: Vec3, limit: f64) -> Option<NonZeroU32> {
        NonZeroU32::new(libm::ceil(delta_v.length() / (limit * Tick(1).seconds())) as u32)
    }

    fn held(delta_v: Vec3, ticks: NonZeroU32) -> Vec3 {
        delta_v * (1.0 / Tick(u64::from(ticks.get())).seconds())
    }

    fn flown(self, from: Body, gravity: Gravity) -> Body {
        (0..self.ticks.get()).fold(from, |body, _| body.after_tick(self.accel, gravity))
    }

    fn end(self) -> Tick {
        Tick(self.from.0 + u64::from(self.ticks.get()))
    }

    fn covers(self, tick: Tick) -> bool {
        self.from <= tick && tick < self.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS_PER_SECOND;
    use crate::orbit::elements::Orbit;

    const MU: Gravity = Gravity::new(4.0e13);

    const RADIUS: f64 = 1.0e7;

    const DEPART: Tick = Tick(120_000);

    const LIMIT: f64 = 2.0;

    fn anchor(along: f64) -> Orbit {
        let speed = (MU.mu() / RADIUS).sqrt();
        let body = Body::new(Vec3::new(RADIUS, 0.0, 0.0), Vec3::new(0.0, 0.0, -speed));
        Orbit::from_body(body, Tick::ZERO, MU)
            .expect("a circular orbit")
            .shifted(along)
    }

    fn transfer(limit: f64, seconds: u64) -> Option<(Schedule, Body, Body)> {
        let arrive = Tick(DEPART.0 + seconds * u64::from(TICKS_PER_SECOND));
        let source = anchor(0.0).at(DEPART, MU);
        let target = anchor(2_000.0).at(arrive, MU);
        Schedule::between(source, target, DEPART, arrive, limit, MU)
            .map(|schedule| (schedule, source, target))
    }

    fn solved() -> (Schedule, Body, Body) {
        (60..=600)
            .find_map(|seconds| transfer(LIMIT, seconds))
            .expect("a transfer inside ten minutes")
    }

    #[test]
    fn a_schedule_flown_tick_by_tick_ends_on_the_destination_anchors_orbit_within_the_tolerance() {
        let (schedule, source, target) = solved();

        let mut body = source;
        for at in DEPART.0..schedule.arrive().0 {
            body = body.after_tick(schedule.thrust(Tick(at)), MU);
        }

        let off = body.pos.distance(target.pos);
        let drift = body.vel.distance(target.vel);
        assert!(off <= Schedule::ARRIVAL_POSITION_METERS, "{off} meters off");
        assert!(
            drift <= Schedule::ARRIVAL_SPEED_METERS_PER_SECOND,
            "{drift} meters per second across"
        );
    }

    #[test]
    fn no_thrust_of_a_schedule_exceeds_its_rows_movement_limit() {
        let (schedule, _, _) = solved();

        for at in DEPART.0..schedule.arrive().0 {
            let thrust = schedule.thrust(Tick(at)).length();
            assert!(thrust <= LIMIT, "{thrust} at {at}");
        }
        assert!(
            schedule.thrust(Tick(schedule.arrive().0)) == Vec3::ZERO,
            "a schedule thrusts past its arrival tick"
        );
    }

    #[test]
    fn a_row_that_cannot_thrust_has_no_schedule() {
        assert!((60..=600).all(|seconds| transfer(0.0, seconds).is_none()));
    }

    #[test]
    fn a_transfer_too_short_for_its_burns_has_no_schedule() {
        assert_eq!(transfer(LIMIT, 1).map(|(schedule, _, _)| schedule), None);
    }
}
