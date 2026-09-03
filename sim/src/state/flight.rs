//! One send: the solved transfer between two anchors, and the burns that
//! fly it.

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

/// Ticks between the arrival ticks a plan tries: one second. Every ship of
/// a send shares the tick, so a finer grain buys nothing.
const SEARCH_STEP: u64 = TICKS_PER_SECOND as u64;

/// The longest flight a plan will consider, in ticks: ten minutes, past
/// which no send inside one match is worth flying.
const SEARCH_BOUND: u64 = 600 * TICKS_PER_SECOND as u64;

/// One send in progress: the transfer from the source anchor to the
/// destination anchor, and the burn pair each row flies it with. Equal and
/// hashed field by field.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Flight {
    from: Body,
    impulses: [Vec3; 2],
    depart: Tick,
    arrive: Tick,
    destination: Place,
    burns: BTreeMap<RowId, [Burn; 2]>,
}

/// A constant thrust over a span of ticks. Its magnitude is at most the
/// limit it was built with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Burn {
    from: Tick,
    to: Tick,
    accel: Vec3,
}

impl Flight {
    /// How near its destination anchor a ship must be for its flight to
    /// end, in meters. A hypothesis the harness and the display confirm or
    /// kill.
    pub const ARRIVAL_DISTANCE: f64 = 3.0;

    /// The send carrying `rows` from `from` to `to`, departing this tick.
    /// The arrival tick is the first, a second at a time, at which every
    /// row's two burns fit inside the flight without overlapping; `None`
    /// once the search passes ten minutes.
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
                destination: to,
                burns,
            });
        }
        None
    }

    /// The tick the send departed.
    pub fn depart(&self) -> Tick {
        self.depart
    }

    /// The tick every ship of the send is due at its destination anchor.
    pub fn arrive(&self) -> Tick {
        self.arrive
    }

    /// The place the send is bound for.
    pub fn destination(&self) -> Place {
        self.destination
    }

    /// The flight's anchor at `tick`: the departure state under the first
    /// impulse, propagated. Every ship of the send manoeuvres about it.
    pub fn anchor(&self, tick: Tick, gravity: Gravity) -> Body {
        let departed = Body::new(self.from.pos, self.from.vel + self.impulses[0]);
        propagate(departed, gravity, tick.seconds() - self.depart.seconds())
    }

    /// Whether a ship at `body` at `tick` has reached this send's
    /// destination anchor, which ends its flight.
    pub fn arrived(&self, state: &State, body: Body, tick: Tick) -> bool {
        let anchor = state.anchor(self.destination).at(tick, state.gravity());
        tick >= self.arrive && body.pos.distance(anchor.pos) <= Flight::ARRIVAL_DISTANCE
    }

    /// What `row`'s burn thrusts at `tick`, in m/s²; zero when no burn of
    /// that row covers the tick.
    pub fn thrust(&self, row: RowId, tick: Tick) -> Vec3 {
        self.burns
            .get(&row)
            .and_then(|burns| burns.iter().find(|burn| burn.covers(tick)))
            .map_or(Vec3::ZERO, Burn::accel)
    }

    /// The burn pair `row` flies this send with, if the send carries it.
    pub fn burns(&self, row: RowId) -> Option<&[Burn; 2]> {
        self.burns.get(&row)
    }

    /// One burn pair per row, or `None` when a row cannot fit both burns
    /// inside the flight without overlapping.
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
    /// The burn delivering `impulse`, in m/s, at no more than `limit`, in
    /// m/s²: as many whole ticks as that needs, centred on `centre` and
    /// moved to lie inside `from..to`. `None` when the span does not fit
    /// inside that window or the row cannot thrust.
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
        // A whole number of ticks holds the thrust, so it is the impulse
        // spread over them, which is the limit only when the span divides.
        let accel = impulse * (1.0 / Tick(ticks).seconds());
        Some(Burn {
            from: Tick(start),
            to: Tick(start + ticks),
            accel,
        })
    }

    /// The first tick of the burn.
    pub fn from(&self) -> Tick {
        self.from
    }

    /// The tick after the burn's last.
    pub fn to(&self) -> Tick {
        self.to
    }

    /// The thrust while the burn runs, in m/s².
    pub fn accel(&self) -> Vec3 {
        self.accel
    }

    /// Whether the burn thrusts through `tick`.
    pub fn covers(&self, tick: Tick) -> bool {
        self.from <= tick && tick < self.to
    }

    /// Whether the two burns thrust through a tick in common.
    pub fn overlaps(&self, other: Burn) -> bool {
        self.from < other.to && other.from < self.to
    }

    /// The whole ticks covering `seconds` of thrust, at least one.
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

    /// A central mass whose belt orbit at [`RADIUS`] takes five minutes, so
    /// a test spans a whole rock period in a few thousand ticks.
    const MU: Gravity = Gravity::new(4.4e17);

    /// The fixture rock's orbit radius, in meters.
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
