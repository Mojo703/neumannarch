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
    use super::*;
    use crate::belt::Belt;
    use crate::fixture::World;
    use crate::ids::{RockId, SeatId, TeamId};
    use crate::orbit::body::Gravity;
    use crate::roster::{CONSTRUCTOR, FRIGATE};
    use crate::state::{Attractor, Send};

    const FAST: Gravity = Gravity::new(4.4e17);

    const SLOW: Gravity = Gravity::new(4.0e13);

    const CLEAR: f64 = 3.0;

    fn world() -> World {
        World::ring(FAST, 2, &[TeamId(0), TeamId(1)])
    }

    #[test]
    fn a_lone_unit_holds_by_its_rock_for_a_whole_rock_period() {
        let mut world = world();
        let home = RockId(0);
        let unit = world.hold(0, FRIGATE, home, 2.0);
        world.moves(20 * u64::from(crate::TICKS_PER_SECOND));
        let settled = world.off_rock(unit, home);
        assert!(settled < 0.05, "settled {settled} meters off");
        for _ in 0..100 {
            world.moves(world.period(home) / 100);
            let off = world.off_rock(unit, home);
            assert!(off < 0.05, "drifted {off} meters off");
        }
    }

    #[test]
    fn a_force_released_together_settles_and_keeps_its_spacing() {
        let mut world = world();
        let home = RockId(0);
        let units: Vec<EntityId> = (0..20)
            .map(|i| world.hold(0, FRIGATE, home, f64::from(i) * 0.01))
            .collect();

        world.moves(60 * u64::from(crate::TICKS_PER_SECOND));

        let body = world.state.rock_body(home);
        for &unit in &units {
            let speed = world.body(unit).vel.distance(body.vel);
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
        let mut world = world();
        let home = RockId(0);
        let heavy = world.hold(0, FRIGATE, home, 0.0);
        let light = world.hold(0, CONSTRUCTOR, home, 0.05);

        world.moves(30 * u64::from(crate::TICKS_PER_SECOND));

        let apart = world.body(heavy).pos.distance(world.body(light).pos);
        assert!(apart > 0.25, "the pair is only {apart} meters apart");
        let (heavy_off, light_off) = (world.off_rock(heavy, home), world.off_rock(light, home));
        assert!(
            light_off > heavy_off,
            "the light ship gave {light_off} meters and the heavy {heavy_off}"
        );
    }

    #[test]
    fn a_send_of_two_rows_stops_flying_together_and_holds_at_its_destination_rock() {
        let mut world = World::ring(SLOW, 2, &[TeamId(0), TeamId(1)]);
        let (from, to) = (RockId(0), RockId(1));
        let units: Vec<EntityId> = [FRIGATE, CONSTRUCTOR]
            .into_iter()
            .map(|row| world.hold(0, row, to, 0.0))
            .collect();
        let send = Send::joining(&world.state, from, to, SeatId(0), &units)
            .expect("a send of a frigate and a constructor");
        for (at, unit) in units.iter().enumerate() {
            world.launch(
                *unit,
                from,
                at as f64 * CLEAR,
                Flight::new(from, send.schedule),
            );
        }
        let arrive = send.schedule.arrive();

        world.moves(arrive.0 - world.state.tick().0);

        for &unit in &units {
            assert!(
                world.state[unit].flight().is_none(),
                "{unit:?} is still flying"
            );
        }

        world.moves(60 * u64::from(crate::TICKS_PER_SECOND));

        for &unit in &units {
            let off = world.off_rock(unit, to);
            assert!(off < 1.0, "{unit:?} holds {off} meters off its destination");
        }
        let apart = world.body(units[0]).pos.distance(world.body(units[1]).pos);
        assert!(apart > 0.25, "the pair holds {apart} meters apart");
    }

    #[test]
    fn a_unit_chases_an_enemy_inside_the_zone_to_half_its_range() {
        let mut world = world();
        let home = RockId(0);
        let hunter = world.hold(0, FRIGATE, home, 0.0);
        let prey = world.hold(1, CONSTRUCTOR, home, Belt::ZONE_RADIUS_METERS - 1.0);

        world.moves(60 * u64::from(crate::TICKS_PER_SECOND));

        let range = world.state[FRIGATE].max_damage_range();
        let apart = world.body(hunter).pos.distance(world.body(prey).pos);
        assert!(
            apart < Belt::ZONE_RADIUS_METERS,
            "the hunter never closed: {apart} meters"
        );
        assert!(
            (apart - 0.5 * range).abs() < 1.0,
            "the hunter holds {apart} meters off, not {}",
            0.5 * range
        );
    }

    #[test]
    fn a_unit_leaves_an_enemy_outside_the_zone_alone() {
        let mut world = world();
        let home = RockId(0);
        let hunter = world.hold(0, FRIGATE, home, 0.0);
        world.hold(1, CONSTRUCTOR, home, Belt::ZONE_RADIUS_METERS + 5.0);

        let sweep = world.state.sweep();
        let pull = Attractor::pulling(&world.state, &world.state[hunter], &sweep)
            .expect("a held unit is pulled home");

        assert_eq!(pull.pos, world.state.rock_body(home).pos, "it gave chase");
    }
}
