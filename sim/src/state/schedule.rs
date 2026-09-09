use core::num::NonZeroU32;

use crate::ids::AsteroidId;
use crate::orbit::body::{Body, Gravity};
use crate::orbit::lambert;
use crate::orbit::universal::propagate;
use crate::time::Time;
use crate::vec3::Vec3;

const CORRECTIONS: u32 = 3;

const BURN_SHARE_OF_SPAN: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Flight {
    source: AsteroidId,
    schedule: Schedule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Schedule {
    burns: [Burn; 2],
    arrive: Time,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Burn {
    from: Time,
    ticks: NonZeroU32,
    accel: Vec3,
}

impl Flight {
    pub(crate) fn new(source: AsteroidId, schedule: Schedule) -> Flight {
        Flight { source, schedule }
    }

    pub(crate) fn source(self) -> AsteroidId {
        self.source
    }

    pub(crate) fn schedule(self) -> Schedule {
        self.schedule
    }

    pub(crate) fn arrive(self) -> Time {
        self.schedule.arrive
    }

    pub(crate) fn departs(self) -> Time {
        self.schedule.departs()
    }

    pub(crate) fn has_departed(self, now: Time) -> bool {
        self.departs() <= now
    }

    pub(crate) fn thrust(self, tick: Time) -> Vec3 {
        self.schedule.thrust(tick)
    }
}

impl Schedule {
    pub(crate) const ARRIVAL_POSITION_METERS: f64 = 0.25;

    pub(crate) const ARRIVAL_SPEED_METERS_PER_SECOND: f64 = 0.02;

    pub(crate) fn between(
        source: Body,
        target: Body,
        depart: Time,
        arrive: Time,
        limit: f64,
        gravity: Gravity,
    ) -> Option<Schedule> {
        let span = Time(arrive.0.checked_sub(depart.0)?).seconds();
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

    pub(crate) fn burns_fit(
        source: Body,
        target: Body,
        depart: Time,
        arrive: Time,
        limit: f64,
        gravity: Gravity,
    ) -> bool {
        let Some(span) = arrive.0.checked_sub(depart.0) else {
            return false;
        };
        lambert::solve(source, target, Time(span).seconds(), gravity)
            .and_then(|impulses| Schedule::coasting(impulses, limit, depart, arrive))
            .is_some()
    }

    #[cfg(test)]
    pub(crate) fn arrive(self) -> Time {
        self.arrive
    }

    pub(crate) fn departs(self) -> Time {
        self.burns[0].from
    }

    pub(crate) fn thrust(self, tick: Time) -> Vec3 {
        self.burns
            .iter()
            .find(|burn| burn.covers(tick))
            .map_or(Vec3::ZERO, |burn| burn.accel)
    }

    fn coasting(impulses: [Vec3; 2], limit: f64, depart: Time, arrive: Time) -> Option<Schedule> {
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
        let coast = Time(last.from.0 - first.end().0).seconds();
        last.flown(propagate(departed, gravity, coast), gravity)
    }
}

impl Burn {
    fn starting(delta_v: Vec3, limit: f64, from: Time) -> Option<Burn> {
        let ticks = Burn::ticks(delta_v, limit)?;
        Some(Burn {
            from,
            ticks,
            accel: Burn::held(delta_v, ticks),
        })
    }

    fn ending(delta_v: Vec3, limit: f64, at: Time) -> Option<Burn> {
        let ticks = Burn::ticks(delta_v, limit)?;
        Some(Burn {
            from: Time(at.0.checked_sub(u64::from(ticks.get()))?),
            ticks,
            accel: Burn::held(delta_v, ticks),
        })
    }

    fn ticks(delta_v: Vec3, limit: f64) -> Option<NonZeroU32> {
        NonZeroU32::new(libm::ceil(delta_v.length() / (limit * Time(1).seconds())) as u32)
    }

    fn held(delta_v: Vec3, ticks: NonZeroU32) -> Vec3 {
        delta_v * (1.0 / Time(u64::from(ticks.get())).seconds())
    }

    fn flown(self, from: Body, gravity: Gravity) -> Body {
        (0..self.ticks.get()).fold(from, |body, _| body.after_tick(self.accel, gravity))
    }

    fn end(self) -> Time {
        Time(self.from.0 + u64::from(self.ticks.get()))
    }

    fn covers(self, tick: Time) -> bool {
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

    const DEPART: Time = Time(120_000);

    const LIMIT: f64 = 2.0;

    fn orbit(along: f64) -> Orbit {
        let speed = (MU.mu() / RADIUS).sqrt();
        let turn = along / RADIUS;
        let (sin, cos) = (libm::sin(turn), libm::cos(turn));
        let body = Body::new(
            Vec3::new(RADIUS * cos, 0.0, -RADIUS * sin),
            Vec3::new(-speed * sin, 0.0, -speed * cos),
        );
        Orbit::from_body(body, Time::ZERO, MU).expect("a circular orbit")
    }

    fn transfer(limit: f64, seconds: u64) -> Option<(Schedule, Body, Body)> {
        let arrive = Time(DEPART.0 + seconds * u64::from(TICKS_PER_SECOND));
        let source = orbit(0.0).at(DEPART, MU);
        let target = orbit(2_000.0).at(arrive, MU);
        Schedule::between(source, target, DEPART, arrive, limit, MU)
            .map(|schedule| (schedule, source, target))
    }

    fn solved() -> (Schedule, Body, Body) {
        (60..=600)
            .find_map(|seconds| transfer(LIMIT, seconds))
            .expect("a transfer inside ten minutes")
    }

    #[test]
    fn a_schedule_flown_tick_by_tick_ends_on_the_destination_orbit_within_the_tolerance() {
        let (schedule, source, target) = solved();

        let mut body = source;
        for at in DEPART.0..schedule.arrive().0 {
            body = body.after_tick(schedule.thrust(Time(at)), MU);
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
    fn no_thrust_of_a_schedule_exceeds_the_movement_limit() {
        let (schedule, _, _) = solved();

        for at in DEPART.0..schedule.arrive().0 {
            let thrust = schedule.thrust(Time(at)).length();
            assert!(thrust <= LIMIT, "{thrust} at {at}");
        }
        assert!(
            schedule.thrust(Time(schedule.arrive().0)) == Vec3::ZERO,
            "a schedule thrusts past its arrival tick"
        );
    }

    #[test]
    fn a_movement_limit_of_zero_has_no_schedule() {
        assert!((60..=600).all(|seconds| transfer(0.0, seconds).is_none()));
    }

    #[test]
    fn a_transfer_too_short_for_its_burns_has_no_schedule() {
        assert_eq!(transfer(LIMIT, 1).map(|(schedule, _, _)| schedule), None);
    }
}
