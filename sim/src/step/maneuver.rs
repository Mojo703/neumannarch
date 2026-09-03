//! The manoeuvring rule: one thrust per free unit, a pull toward its
//! attractor plus a bounded term with every ship nearby. It stores nothing
//! and reads only the snapshot, so it can be replaced whole.

use std::collections::BTreeMap;

use crate::ids::{EntityId, SeatId};
use crate::orbit::body::Body;
use crate::place::Place;
use crate::state::sweep::Sweep;
use crate::state::{Attractor, Entity, Motion, Sight, State};
use crate::time::Tick;
use crate::vec3::Vec3;

/// Pull toward the attractor's position, in m/s² per meter of offset. A
/// hypothesis the harness and the display confirm or kill.
const STIFFNESS: f64 = 0.25;

/// Pull toward the attractor's velocity, in m/s² per m/s of difference.
/// Twice the root of the stiffness, so the pull is critically damped and a
/// ship does not orbit its attractor. A hypothesis.
const DAMPING: f64 = 1.0;

/// The separation a pair of ships settles at, in meters. A hypothesis.
const SPACING: f64 = 0.5;

/// The separation past which a pair of ships ignores each other, in
/// meters. Twice this plus the longest weapon range stays below the gap
/// between the two bands' amplitudes. A hypothesis.
const CUTOFF: f64 = 2.0;

/// The strongest a pair term pushes apart, in m/s². Above every row's
/// manoeuvring limit, so a ship pressed on always has the thrust to give
/// ground. A hypothesis.
const PAIR_STRENGTH: f64 = 8.0;

/// The strongest a pair term pulls together, as a fraction of
/// `PAIR_STRENGTH`. Below one, so a crowd cannot compress itself past the
/// spacing. A hypothesis.
const ATTRACTION_SHARE: f64 = 0.05;

/// One tick's manoeuvring thrusts, computed from the snapshot.
pub struct Maneuver<'a> {
    state: &'a State,
    sweep: Sweep,
    sights: BTreeMap<SeatId, Sight>,
}

/// One free unit's manoeuvring thrust this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thrust {
    pub entity: EntityId,
    /// In m/s², at most the row's manoeuvring limit.
    pub accel: Vec3,
}

/// Every free unit's manoeuvring thrust this tick, in id order.
#[derive(Clone, Debug, PartialEq)]
pub struct Thrusts(Vec<Thrust>);

impl<'a> Maneuver<'a> {
    /// Reads `state`, building the sweep and one sight per seat.
    pub fn of(state: &'a State) -> Maneuver<'a> {
        let sweep = state.sweep();
        let sights = (0..state.seats().len())
            .map(|seat| SeatId(seat as u8))
            .map(|seat| (seat, Sight::of(state, seat, &sweep)))
            .collect();
        Maneuver {
            state,
            sweep,
            sights,
        }
    }

    /// The body a unit spawning at `place` for `tick` takes: the anchor at
    /// that tick, offset along the rock's radial direction by one spacing
    /// per unit already there, so no two spawn coincident. `tick` is the
    /// first tick the unit exists in, one on from the step that built it,
    /// so it starts on its anchor rather than a tick behind it.
    pub fn spawn_body(state: &State, place: Place, tick: Tick) -> Body {
        let anchor = state.anchor(place).at(tick, state.gravity());
        let already = state
            .entities_at(place)
            .filter(|entity| entity.motion() != Motion::Fixed)
            .count();
        let radial = anchor.pos.normalized().unwrap_or(Vec3::ZERO);
        Body::new(anchor.pos + radial * (SPACING * already as f64), anchor.vel)
    }

    /// One thrust per free unit, in id order.
    pub fn run(self) -> Thrusts {
        Thrusts(
            self.state
                .entities()
                .filter_map(|entity| self.thrust(entity))
                .collect(),
        )
    }

    /// `entity`'s thrust, or `None` for a structure.
    fn thrust(&self, entity: &Entity) -> Option<Thrust> {
        let Motion::Free { body, .. } = entity.motion() else {
            return None;
        };
        let row = &self.state[entity.row()];
        let sight = self.sights.get(&entity.seat())?;
        let attractor = Attractor::of(self.state, entity, sight, &self.sweep);
        let pull = (attractor.pos - body.pos) * STIFFNESS + (attractor.vel - body.vel) * DAMPING;
        let accel = pull + self.pairs(entity, body);
        Some(Thrust {
            entity: entity.id(),
            accel: within(accel, row.maneuver.0),
        })
    }

    /// The sum of `entity`'s pair terms with every ship inside the cutoff.
    fn pairs(&self, entity: &Entity, body: Body) -> Vec3 {
        let mass = self.state[entity.row()].mass.0;
        self.sweep
            .within(body.pos, CUTOFF)
            .filter(|id| *id != entity.id())
            .filter_map(|id| self.state.entity(id))
            .filter(|other| other.motion() != Motion::Fixed)
            .filter_map(|other| {
                let offset = self.state.body_of(other).pos - body.pos;
                let away = -offset.normalized()?;
                let other_mass = self.state[other.row()].mass.0;
                let share = other_mass / (mass + other_mass);
                Some(away * (pair_magnitude(offset.length()) * share))
            })
            .fold(Vec3::ZERO, |sum, term| sum + term)
    }
}

impl Thrusts {
    /// What the entity `id` names thrusts this tick, in m/s²; zero for a
    /// structure or an entity that is not there.
    pub fn of(&self, id: EntityId) -> Vec3 {
        self.0
            .binary_search_by_key(&id, |thrust| thrust.entity)
            .map_or(Vec3::ZERO, |at| self.0[at].accel)
    }
}

/// How hard a pair at `separation` meters pushes apart: positive below the
/// spacing, negative from there to the cutoff, zero at the spacing, at the
/// cutoff, past it, and where the two coincide, which is no line. The push
/// rises to `PAIR_STRENGTH` at half the spacing and holds there, so a crowd
/// meets a wall before it packs that close; nothing here is unbounded.
fn pair_magnitude(separation: f64) -> f64 {
    let peak = 0.5 * (SPACING + CUTOFF);
    if separation <= 0.0 || separation >= CUTOFF {
        0.0
    } else if separation < SPACING {
        PAIR_STRENGTH * (SPACING / separation - 1.0).min(1.0)
    } else {
        -PAIR_STRENGTH * ATTRACTION_SHARE * (1.0 - (separation - peak).abs() / (peak - SPACING))
    }
}

/// `accel` cut to `limit`, both in m/s².
fn within(accel: Vec3, limit: f64) -> Vec3 {
    let length = accel.length();
    if length > limit {
        accel * (limit / length)
    } else {
        accel
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::ids::{RockId, TeamId};
    use crate::materials::Materials;
    use crate::orbit::body::Gravity;
    use crate::orbit::elements::Orbit;
    use crate::place::Band;
    use crate::roster::{FRIGATE, Roster};
    use crate::state::{Rock, Seat};

    const MU: Gravity = Gravity::new(4.0e13);

    fn place() -> Place {
        Place {
            rock: RockId(0),
            band: Band::Inner,
        }
    }

    fn state() -> State {
        let radius = 1.0e7;
        let speed = (MU.mu() / radius).sqrt();
        let body = Body::new(Vec3::new(radius, 0.0, 0.0), Vec3::new(0.0, 0.0, -speed));
        let orbit = Orbit::from_body(body, Tick::ZERO, MU).expect("a circular orbit");
        State::new(
            Tick(120_000),
            0,
            MU,
            Roster::shipped(),
            vec![Rock::new(orbit, Materials::ZERO, 100.0)],
            vec![Seat::new(TeamId(0), Materials::ZERO, BTreeMap::new())],
        )
    }

    #[test]
    fn each_unit_spawns_one_spacing_further_out_than_the_last() {
        let mut state = state();
        let anchor = state.anchor(place()).at(state.tick(), MU);
        let radial = anchor.pos.normalized().expect("a radius");
        for already in 0..3 {
            let spawn = Maneuver::spawn_body(&state, place(), state.tick());
            let expected = anchor.pos + radial * (SPACING * f64::from(already));
            assert!(
                spawn.pos.distance(expected) < 1e-9,
                "unit {already} spawns at {spawn:?}"
            );
            assert_eq!(spawn.vel, anchor.vel);
            state.spawn(
                SeatId(0),
                FRIGATE,
                place(),
                Motion::Free {
                    body: spawn,
                    flight: None,
                },
            );
        }
    }

    #[test]
    fn a_structure_at_the_place_does_not_move_a_spawn() {
        let mut state = state();
        state.spawn(SeatId(0), FRIGATE, place(), Motion::Fixed);
        let anchor = state.anchor(place()).at(state.tick(), MU);
        assert_eq!(
            Maneuver::spawn_body(&state, place(), state.tick()).pos,
            anchor.pos
        );
    }

    #[test]
    fn a_pair_pushes_apart_inside_the_spacing_and_pulls_together_outside_it() {
        assert!(pair_magnitude(0.5 * SPACING) > 0.0);
        assert!(pair_magnitude(0.5 * (SPACING + CUTOFF)) < 0.0);
    }

    #[test]
    fn a_pair_term_is_nothing_at_the_spacing_the_cutoff_and_beyond() {
        assert_eq!(pair_magnitude(SPACING), 0.0);
        assert_eq!(pair_magnitude(CUTOFF), 0.0);
        assert_eq!(pair_magnitude(10.0 * CUTOFF), 0.0);
        assert_eq!(pair_magnitude(0.0), 0.0);
        assert_eq!(pair_magnitude(-1.0), 0.0);
    }

    #[test]
    fn a_pair_term_is_bounded_at_every_separation() {
        for step in 0..1000 {
            let separation = step as f64 * 0.01 * CUTOFF;
            let magnitude = pair_magnitude(separation);
            assert!(
                magnitude.abs() <= PAIR_STRENGTH,
                "{separation}: {magnitude}"
            );
        }
        assert!(pair_magnitude(f64::MIN_POSITIVE).abs() <= PAIR_STRENGTH);
    }

    #[test]
    fn a_thrust_is_cut_to_the_limit_and_keeps_its_direction() {
        let big = Vec3::new(3.0, 4.0, 0.0);
        assert!(within(big, 1.0).distance(Vec3::new(0.6, 0.8, 0.0)) < 1e-15);
        assert_eq!(within(big, 5.0), big);
        assert_eq!(within(Vec3::ZERO, 1.0), Vec3::ZERO);
    }
}
