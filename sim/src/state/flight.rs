use std::collections::BTreeMap;

use super::State;
use crate::TICKS_PER_SECOND;
use crate::ids::RowId;
use crate::orbit::body::{Body, Gravity};
use crate::orbit::lambert;
use crate::orbit::universal::propagate;
use crate::place::Place;
use crate::time::Tick;
use crate::vec3::Vec3;

const SEARCH_STEP: u64 = TICKS_PER_SECOND as u64;

const SEARCH_BOUND: u64 = 600 * TICKS_PER_SECOND as u64;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Flight {
    from: Body,
    impulses: [Vec3; 2],
    depart: Tick,
    arrive: Tick,
    source: Place,
    destination: Place,
    burns: BTreeMap<RowId, [Burn; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Burn {
    from: Tick,
    to: Tick,
    accel: Vec3,
}

impl Flight {
    pub const ARRIVAL_DISTANCE: f64 = 3.0;

    pub fn plan(
        state: &State,
        from: Place,
        to: Place,
        rows: impl Iterator<Item = RowId>,
    ) -> Option<Flight> {
        let gravity = state.gravity();
        let depart = state.tick();
        let source = state.anchor(from).at(depart, gravity);
        let rows: Vec<RowId> = rows.collect();
        for step in (SEARCH_STEP..=SEARCH_BOUND).step_by(SEARCH_STEP as usize) {
            let arrive = Tick(depart.0 + step);
            let target = state.anchor(to).at(arrive, gravity);
            let span = Tick(step).seconds();
            let Some(impulses) = lambert::solve(source, target, span, gravity) else {
                continue;
            };
            let Some(burns) = Flight::spread(state, &rows, impulses, depart, arrive) else {
                continue;
            };
            return Some(Flight {
                from: source,
                impulses,
                depart,
                arrive,
                source: from,
                destination: to,
                burns,
            });
        }
        None
    }

    pub fn depart(&self) -> Tick {
        self.depart
    }

    pub fn arrive(&self) -> Tick {
        self.arrive
    }

    pub fn source(&self) -> Place {
        self.source
    }

    pub fn destination(&self) -> Place {
        self.destination
    }

    pub fn anchor(&self, tick: Tick, gravity: Gravity) -> Body {
        let departed = Body::new(self.from.pos, self.from.vel + self.impulses[0]);
        propagate(departed, gravity, tick.seconds() - self.depart.seconds())
    }

    pub fn arrived(&self, state: &State, body: Body, tick: Tick) -> bool {
        let anchor = state.anchor(self.destination).at(tick, state.gravity());
        tick >= self.arrive && body.pos.distance(anchor.pos) <= Flight::ARRIVAL_DISTANCE
    }

    pub fn thrust(&self, row: RowId, tick: Tick) -> Vec3 {
        self.burns
            .get(&row)
            .and_then(|burns| burns.iter().find(|burn| burn.covers(tick)))
            .map_or(Vec3::ZERO, Burn::accel)
    }

    pub fn burns(&self, row: RowId) -> Option<&[Burn; 2]> {
        self.burns.get(&row)
    }

    fn spread(
        state: &State,
        rows: &[RowId],
        impulses: [Vec3; 2],
        depart: Tick,
        arrive: Tick,
    ) -> Option<BTreeMap<RowId, [Burn; 2]>> {
        let mut burns = BTreeMap::new();
        for &row in rows {
            let limit = state.roster().get(row)?.accel.0;
            let first = Burn::spread(impulses[0], limit, depart, depart, arrive)?;
            let second = Burn::spread(impulses[1], limit, arrive, depart, arrive)?;
            if first.overlaps(second) {
                return None;
            }
            burns.insert(row, [first, second]);
        }
        Some(burns)
    }
}

impl Burn {
    pub(crate) fn spread(
        impulse: Vec3,
        limit: f64,
        centre: Tick,
        from: Tick,
        to: Tick,
    ) -> Option<Burn> {
        if limit <= 0.0 || limit.is_nan() {
            return None;
        }
        let ticks = Burn::ticks(impulse.length() / limit);
        let window = to.0.checked_sub(from.0)?;
        if ticks > window {
            return None;
        }
        let start = centre
            .0
            .saturating_sub(ticks / 2)
            .clamp(from.0, to.0 - ticks);

        let accel = impulse * (1.0 / Tick(ticks).seconds());
        Some(Burn {
            from: Tick(start),
            to: Tick(start + ticks),
            accel,
        })
    }

    pub fn from(&self) -> Tick {
        self.from
    }

    pub fn to(&self) -> Tick {
        self.to
    }

    pub fn accel(&self) -> Vec3 {
        self.accel
    }

    pub fn covers(&self, tick: Tick) -> bool {
        self.from <= tick && tick < self.to
    }

    pub fn overlaps(&self, other: Burn) -> bool {
        self.from < other.to && other.from < self.to
    }

    fn ticks(seconds: f64) -> u64 {
        let ticks = libm::ceil(seconds * f64::from(TICKS_PER_SECOND));
        (ticks as u64).max(1)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::ids::TeamId;
    use crate::materials::Materials;
    use crate::orbit::elements::Orbit;
    use crate::place::Band;
    use crate::real::Real;
    use crate::roster::{EXTRACTOR, FRIGATE, Roster, Row, SCOUT};
    use crate::state::rock::Rock;
    use crate::state::seat::Seat;
    use crate::{RockId, Vec3};

    const MU: Gravity = Gravity::new(4.4e17);

    const RADIUS: f64 = 1.0e7;

    fn inner() -> Place {
        Place {
            rock: RockId(0),
            band: Band::Inner,
        }
    }

    fn outer() -> Place {
        Place {
            rock: RockId(0),
            band: Band::Outer,
        }
    }

    fn state_with(roster: Roster) -> State {
        let speed = (MU.mu() / RADIUS).sqrt();
        let body = Body::new(Vec3::new(RADIUS, 0.0, 0.0), Vec3::new(0.0, 0.0, -speed));
        let orbit = Orbit::from_body(body, Tick::ZERO, MU).expect("a circular orbit");
        let rock = Rock::new(orbit, Materials::new(1.0, 1.0, 1.0), 100.0);
        let seat = Seat::new(TeamId(0), Materials::new(1e3, 1e3, 1e3), BTreeMap::new());
        State::new(Tick(120_000), 0, MU, roster, vec![rock], vec![seat])
    }

    fn state() -> State {
        state_with(Roster::shipped())
    }

    #[test]
    fn a_plan_fits_both_burns_of_every_row_inside_the_flight() {
        let state = state();
        let rows = [FRIGATE, SCOUT];
        let flight = Flight::plan(&state, inner(), outer(), rows.into_iter()).expect("a plan");
        assert!(flight.arrive() > flight.depart());
        assert_eq!(flight.destination(), outer());
        for row in rows {
            let [first, second] = *flight.burns(row).expect("the row's burns");
            assert!(!first.overlaps(second), "{row:?} burns overlap");
            assert!(
                first.from() >= flight.depart(),
                "{row:?} burns before departing"
            );
            assert!(
                second.to() <= flight.arrive(),
                "{row:?} burns after arriving"
            );
            let limit = state[row].accel.0;
            for burn in [first, second] {
                assert!(
                    burn.accel().length() <= limit,
                    "{row:?}: {burn:?} over {limit}"
                );
            }
        }
    }

    #[test]
    fn no_arrival_before_the_plans_fits_the_same_burns() {
        let state = state();
        let rows = [FRIGATE, SCOUT];
        let flight = Flight::plan(&state, inner(), outer(), rows.into_iter()).expect("a plan");
        let source = state.anchor(inner()).at(state.tick(), MU);
        let chosen = flight.arrive().0 - flight.depart().0;
        assert!(
            chosen > SEARCH_STEP,
            "the first candidate cannot be the answer"
        );
        for step in (SEARCH_STEP..chosen).step_by(SEARCH_STEP as usize) {
            let arrive = Tick(flight.depart().0 + step);
            let target = state.anchor(outer()).at(arrive, MU);
            let fits =
                lambert::solve(source, target, Tick(step).seconds(), MU).is_some_and(|impulses| {
                    Flight::spread(&state, &rows, impulses, flight.depart(), arrive).is_some()
                });
            assert!(!fits, "arriving at {arrive:?} would have fitted");
        }
    }

    #[test]
    fn a_row_that_cannot_thrust_has_no_plan() {
        let state = state();
        assert_eq!(
            Flight::plan(&state, inner(), outer(), [EXTRACTOR].into_iter()),
            None
        );
    }

    #[test]
    fn a_row_too_slow_to_arrive_within_the_bound_has_no_plan() {
        let mut roster = Roster::shipped();
        let crawler = roster.add(Row {
            name: "crawler",
            accel: Real(1e-6),
            maneuver: Real(2.5e-7),
            ..roster[FRIGATE].clone()
        });
        let state = state_with(roster);
        assert_eq!(
            Flight::plan(&state, inner(), outer(), [crawler].into_iter()),
            None
        );
    }

    #[test]
    fn a_burn_never_thrusts_above_its_limit_and_stays_in_its_window() {
        let window = (Tick(1_000), Tick(1_240));
        for speed in [0.0, 1e-3, 0.5, 1.0, 1.9] {
            let impulse = Vec3::new(0.0, speed, 0.0);
            let burn = Burn::spread(impulse, 1.0, window.0, window.0, window.1).expect("a burn");
            assert!(burn.accel().length() <= 1.0, "{speed}: {burn:?}");
            assert!(burn.from() >= window.0 && burn.to() <= window.1, "{burn:?}");
            let delivered = burn.accel() * (Tick(burn.to().0 - burn.from().0).seconds());
            assert!(
                delivered.distance(impulse) < 1e-12,
                "{speed}: {delivered:?}"
            );
        }
    }

    #[test]
    fn a_burn_too_long_for_its_window_does_not_exist() {
        let window = (Tick(0), Tick(120));
        assert_eq!(
            Burn::spread(Vec3::new(2.0, 0.0, 0.0), 1.0, window.0, window.0, window.1),
            None
        );
        assert!(
            Burn::spread(Vec3::new(0.9, 0.0, 0.0), 1.0, window.0, window.0, window.1).is_some()
        );
        assert_eq!(
            Burn::spread(Vec3::ZERO, 0.0, window.0, window.0, window.1),
            None
        );
    }

    #[test]
    fn a_burn_covers_its_own_ticks_only() {
        let burn = Burn::spread(
            Vec3::new(1.0, 0.0, 0.0),
            1.0,
            Tick(600),
            Tick(0),
            Tick(1_200),
        )
        .expect("a burn");
        assert!(burn.covers(burn.from()));
        assert!(!burn.covers(burn.to()));
        assert!(!burn.covers(Tick(burn.from().0 - 1)));
    }
}
