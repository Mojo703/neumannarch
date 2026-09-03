//! The propagation phase: every free unit thrusts once and moves one tick
//! of two-body motion, and a flight ends for a ship that has arrived.

use crate::ids::{EntityId, FlightId};
use crate::orbit::body::Body;
use crate::orbit::universal::propagate;
use crate::state::{Entity, Motion, State};
use crate::step::maneuver::Thrusts;
use crate::time::Tick;
use crate::vec3::Vec3;

/// One free unit's motion one tick on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub entity: EntityId,
    pub body: Body,
    /// The send it is still flying, if it has not arrived.
    pub flight: Option<FlightId>,
}

/// Every free unit's motion one tick on, in id order.
#[derive(Clone, Debug, PartialEq)]
pub struct Moved(Vec<Move>);

/// One tick of motion, computed from the snapshot and the tick's thrusts.
pub struct Propagation<'a> {
    state: &'a State,
    thrusts: &'a Thrusts,
}

impl<'a> Propagation<'a> {
    /// Reads `state` and the manoeuvring thrusts of the same tick.
    pub fn of(state: &'a State, thrusts: &'a Thrusts) -> Propagation<'a> {
        Propagation { state, thrusts }
    }

    /// One move per free unit, in id order.
    pub fn run(self) -> Moved {
        Moved(
            self.state
                .entities()
                .filter_map(|entity| self.moved(entity))
                .collect(),
        )
    }

    /// `entity` one tick on, or `None` for a structure.
    fn moved(&self, entity: &Entity) -> Option<Move> {
        let Motion::Free { body, flight } = entity.motion() else {
            return None;
        };
        let span = Tick(1).seconds();
        let tick = self.state.tick();
        let burn = self.burn(entity, flight, tick);
        let thrust = burn + self.thrusts.of(entity.id());
        let thrust = Body::new(body.pos, body.vel + thrust * span);
        let moved = propagate(thrust, self.state.gravity(), span);
        Some(Move {
            entity: entity.id(),
            body: moved,
            flight: flight.filter(|id| !self.arrived(*id, moved, tick.next())),
        })
    }

    /// What `entity`'s flight has its row thrust at `tick`, in m/s².
    fn burn(&self, entity: &Entity, flight: Option<FlightId>, tick: Tick) -> Vec3 {
        flight
            .and_then(|id| self.state.flight(id))
            .map_or(Vec3::ZERO, |flight| flight.thrust(entity.row(), tick))
    }

    /// Whether a ship at `body` at `tick` has reached the send's
    /// destination anchor. A send the state has lost counts as arrived.
    fn arrived(&self, flight: FlightId, body: Body, tick: Tick) -> bool {
        self.state
            .flight(flight)
            .is_none_or(|flight| flight.arrived(self.state, body, tick))
    }
}

impl Moved {
    /// Every move, in id order.
    pub fn iter(&self) -> impl Iterator<Item = Move> + '_ {
        self.0.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::ids::{RockId, RowId, SeatId, TeamId};
    use crate::materials::Materials;
    use crate::orbit::body::Gravity;
    use crate::orbit::elements::Orbit;
    use crate::place::{Band, Place};
    use crate::roster::{FRIGATE, Roster, SCOUT, SHIPYARD};
    use crate::state::{Flight, Rock, Seat};
    use crate::step::maneuver::Maneuver;

    /// A central mass whose belt orbit at [`RADIUS`] takes five minutes, so
    /// a test spans a whole rock period in a few thousand ticks.
    const MU: Gravity = Gravity::new(4.4e17);

    /// A central mass whose belt orbit at [`RADIUS`] takes nine hours, the
    /// scale a match is played at.
    const SLOW: Gravity = Gravity::new(4.0e13);

    /// The near rock's orbit radius, in meters.
    const RADIUS: f64 = 1.0e7;

    /// A match of one seat per team over two rocks a kilometer apart.
    struct World {
        state: State,
        gravity: Gravity,
    }

    impl World {
        fn new() -> World {
            World::with(MU)
        }

        fn with(gravity: Gravity) -> World {
            let seat = |team| Seat::new(TeamId(team), Materials::ZERO, BTreeMap::new());
            World {
                state: State::new(
                    Tick(1_000_000),
                    0,
                    gravity,
                    Roster::shipped(),
                    vec![rock(RADIUS, gravity), rock(RADIUS + 1_000.0, gravity)],
                    vec![seat(0), seat(1)],
                ),
                gravity,
            }
        }

        /// A unit of `row` for `seat`, homed at `place`, `offset` meters
        /// along the rock's radial direction from the place's anchor.
        fn spawn(&mut self, seat: SeatId, row: RowId, place: Place, offset: f64) -> EntityId {
            let anchor = self.state.anchor(place).at(self.state.tick(), self.gravity);
            let radial = anchor.pos.normalized().expect("a radius");
            let motion = Motion::Free {
                body: Body::new(anchor.pos + radial * offset, anchor.vel),
                flight: None,
            };
            self.state.spawn(seat, row, place, motion)
        }

        /// Puts the entity on `flight` at its source anchor, as a send
        /// does.
        fn send(&mut self, entity: EntityId, flight: FlightId, from: Place) {
            let motion = Motion::Free {
                body: self.anchor(from),
                flight: Some(flight),
            };
            self.state.set_motion(entity, motion);
        }

        /// Runs the manoeuvring and propagation phases for `ticks` ticks.
        fn run(&mut self, ticks: u64) {
            for _ in 0..ticks {
                let thrusts = Maneuver::of(&self.state).run();
                let moved = Propagation::of(&self.state, &thrusts).run();
                for step in moved.iter() {
                    self.state.set_motion(
                        step.entity,
                        Motion::Free {
                            body: step.body,
                            flight: step.flight,
                        },
                    );
                }
                self.state.advance();
            }
        }

        fn body(&self, entity: EntityId) -> Body {
            self.state.body_of(&self.state[entity])
        }

        fn anchor(&self, place: Place) -> Body {
            self.state.anchor(place).at(self.state.tick(), self.gravity)
        }

        /// How far `entity` is from the anchor of `place`, in meters.
        fn off_anchor(&self, entity: EntityId, place: Place) -> f64 {
            self.body(entity).pos.distance(self.anchor(place).pos)
        }
    }

    fn rock(radius: f64, gravity: Gravity) -> Rock {
        let speed = (gravity.mu() / radius).sqrt();
        let body = Body::new(Vec3::new(radius, 0.0, 0.0), Vec3::new(0.0, 0.0, -speed));
        let orbit = Orbit::from_body(body, Tick::ZERO, gravity).expect("a circular orbit");
        Rock::new(orbit, Materials::new(1.0, 1.0, 1.0), 100.0)
    }

    fn inner(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Inner,
        }
    }

    fn outer(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Outer,
        }
    }

    /// One rock period, in ticks.
    fn period() -> u64 {
        let seconds = rock(RADIUS, MU).orbit().period(MU);
        (seconds * f64::from(crate::TICKS_PER_SECOND)) as u64
    }

    #[test]
    fn a_lone_unit_holds_by_its_anchor_for_a_whole_rock_period() {
        let mut world = World::new();
        let home = inner(0);
        let unit = world.spawn(SeatId(0), FRIGATE, home, 2.0);
        world.run(20 * u64::from(crate::TICKS_PER_SECOND));
        let settled = world.off_anchor(unit, home);
        assert!(settled < 0.05, "settled {settled} meters off");
        for _ in 0..100 {
            world.run(period() / 100);
            let off = world.off_anchor(unit, home);
            assert!(off < 0.05, "drifted {off} meters off");
        }
    }

    #[test]
    fn a_force_released_together_settles_and_keeps_its_spacing() {
        let mut world = World::new();
        let home = inner(0);
        let units: Vec<EntityId> = (0..20)
            .map(|i| world.spawn(SeatId(0), FRIGATE, home, f64::from(i) * 0.01))
            .collect();

        world.run(60 * u64::from(crate::TICKS_PER_SECOND));

        let anchor = world.anchor(home);
        for &unit in &units {
            let speed = world.body(unit).vel.distance(anchor.vel);
            assert!(speed < 0.05, "{unit:?} still moving at {speed} m/s");
        }
        for (at, &unit) in units.iter().enumerate() {
            for &other in &units[at + 1..] {
                let apart = world.body(unit).pos.distance(world.body(other).pos);
                assert!(
                    apart > 0.25,
                    "{unit:?} and {other:?} are {apart} meters apart"
                );
            }
        }
    }

    #[test]
    fn the_lighter_of_two_ships_gives_way() {
        let mut world = World::new();
        let home = inner(0);
        let heavy = world.spawn(SeatId(0), FRIGATE, home, 0.0);
        let light = world.spawn(SeatId(0), SCOUT, home, 0.05);

        world.run(30 * u64::from(crate::TICKS_PER_SECOND));

        let apart = world.body(heavy).pos.distance(world.body(light).pos);
        assert!(apart > 0.25, "the pair is only {apart} meters apart");
        let (heavy_off, light_off) = (world.off_anchor(heavy, home), world.off_anchor(light, home));
        assert!(
            light_off > heavy_off,
            "the light ship gave {light_off} meters and the heavy {heavy_off}"
        );
    }

    #[test]
    fn a_send_of_two_rows_arrives_together_at_its_destination_anchor() {
        let mut world = World::with(SLOW);
        let (from, to) = (inner(0), inner(1));
        let rows = [FRIGATE, SCOUT];
        let flight = Flight::plan(&world.state, from, to, rows.into_iter()).expect("a plan");
        let arrive = flight.arrive();
        let id = world.state.add_flight(flight);
        let units: Vec<EntityId> = rows
            .into_iter()
            .map(|row| {
                let unit = world.spawn(SeatId(0), row, to, 0.0);
                world.send(unit, id, from);
                unit
            })
            .collect();

        // A heavy row falls far behind the flight's impulsive anchor and
        // overshoots it before settling; five minutes covers that for the
        // shipped rows at this scale.
        let slack = 300 * u64::from(crate::TICKS_PER_SECOND);
        world.run(arrive.0 - world.state.tick().0 + slack);

        for &unit in &units {
            let off = world.off_anchor(unit, to);
            assert!(
                off <= Flight::ARRIVAL_DISTANCE,
                "{unit:?} is {off} meters from its destination anchor"
            );
            assert!(!world.state[unit].is_flying(), "{unit:?} is still flying");
        }
    }

    #[test]
    fn a_unit_chases_an_enemy_inside_its_leash_to_half_its_range() {
        let mut world = World::new();
        let home = inner(0);
        let hunter = world.spawn(SeatId(0), FRIGATE, home, 0.0);
        let prey = world.state.spawn(SeatId(1), SHIPYARD, home, Motion::Fixed);

        world.run(60 * u64::from(crate::TICKS_PER_SECOND));

        let range = world.state[FRIGATE].max_damage_range();
        let apart = world.body(hunter).pos.distance(world.body(prey).pos);
        assert!(
            (apart - 0.5 * range).abs() < 0.3,
            "the hunter holds {apart} meters off, not {}",
            0.5 * range
        );
    }

    #[test]
    fn a_unit_leaves_an_enemy_outside_its_leash_alone() {
        let mut world = World::new();
        let home = inner(0);
        let hunter = world.spawn(SeatId(0), FRIGATE, home, 0.0);
        world
            .state
            .spawn(SeatId(1), SHIPYARD, outer(1), Motion::Fixed);

        world.run(30 * u64::from(crate::TICKS_PER_SECOND));

        let off = world.off_anchor(hunter, home);
        assert!(off < 0.05, "the hunter left its anchor by {off} meters");
    }
}
