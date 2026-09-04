use crate::ids::EntityId;
use crate::orbit::body::Body;
use crate::state::{Entity, Flight, Motion, State};
use crate::step::maneuver::Thrusts;
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub entity: EntityId,
    pub body: Body,
    pub flight: Option<Flight>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Moved(Vec<Move>);

pub struct Propagation<'a> {
    state: &'a State,
    thrusts: &'a Thrusts,
}

impl<'a> Propagation<'a> {
    pub fn of(state: &'a State, thrusts: &'a Thrusts) -> Propagation<'a> {
        Propagation { state, thrusts }
    }

    pub fn run(self) -> Moved {
        Moved(
            self.state
                .entities()
                .filter_map(|entity| self.moved(entity))
                .collect(),
        )
    }

    fn moved(&self, entity: &Entity) -> Option<Move> {
        let Motion::Free { body, flight } = entity.motion() else {
            return None;
        };
        let tick = self.state.tick();
        let scheduled = flight.map_or(Vec3::ZERO, |flight| flight.thrust(tick));
        let thrust = scheduled + self.thrusts.of(entity.id());
        Some(Move {
            entity: entity.id(),
            body: body.after_tick(thrust, self.state.gravity()),
            flight: flight.filter(|flight| tick.next() < flight.arrive()),
        })
    }
}

impl Moved {
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
    use crate::state::{Rock, Seat, Send};
    use crate::step::maneuver::Maneuver;
    use crate::time::Tick;
    use crate::vec3::Vec3;

    const MU: Gravity = Gravity::new(4.4e17);

    const SLOW: Gravity = Gravity::new(4.0e13);

    const RADIUS: f64 = 1.0e7;

    const CLEAR: f64 = 3.0;

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

        fn spawn(&mut self, seat: SeatId, row: RowId, place: Place, offset: f64) -> EntityId {
            let anchor = self.state.anchor(place).at(self.state.tick(), self.gravity);
            let radial = anchor.pos.normalized().expect("a radius");
            let motion = Motion::Free {
                body: Body::new(anchor.pos + radial * offset, anchor.vel),
                flight: None,
            };
            self.state.spawn(seat, row, place, motion)
        }

        fn send(&mut self, entity: EntityId, from: Place, offset: f64, flight: Flight) {
            let anchor = self.anchor(from);
            let radial = anchor.pos.normalized().expect("a radius");
            let motion = Motion::Free {
                body: Body::new(anchor.pos + radial * offset, anchor.vel),
                flight: Some(flight),
            };
            self.state.set_motion(entity, motion);
        }

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
    fn a_send_of_two_rows_stops_flying_together_and_holds_at_its_destination_anchor() {
        let mut world = World::with(SLOW);
        let (from, to) = (inner(0), inner(1));
        let units: Vec<EntityId> = [FRIGATE, SCOUT]
            .into_iter()
            .map(|row| world.spawn(SeatId(0), row, to, 0.0))
            .collect();
        let send =
            Send::solved(&world.state, from, to, &units).expect("a send of a frigate and a scout");
        for (at, unit) in units.iter().enumerate() {
            world.send(
                *unit,
                from,
                at as f64 * CLEAR,
                Flight::new(from, send.schedule),
            );
        }
        let arrive = send.schedule.arrive();

        world.run(arrive.0 - world.state.tick().0);

        for &unit in &units {
            assert!(!world.state[unit].is_flying(), "{unit:?} is still flying");
        }

        world.run(60 * u64::from(crate::TICKS_PER_SECOND));

        for &unit in &units {
            let off = world.off_anchor(unit, to);
            assert!(off < 1.0, "{unit:?} holds {off} meters off its destination");
        }
        let apart = world.body(units[0]).pos.distance(world.body(units[1]).pos);
        assert!(apart > 0.25, "the pair holds {apart} meters apart");
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
