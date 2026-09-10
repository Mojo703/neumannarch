use super::HeldUnit;
use crate::belt::Belt;
use crate::ids::EntityId;
use crate::state::{AssignedDamage, Pass, Reach, Shooter};
use crate::vec3::Vec3;

impl HeldUnit<'_> {
    pub(super) fn passing(&self) -> Option<(Vec3, Pass)> {
        self.entity.pattern().runs_passes().then_some(())?;
        let station = self.station()?;
        Some(match self.entity.pass() {
            Pass::Running => self.running(station),
            Pass::Returning => self.returning(station),
        })
    }

    fn running(&self, station: Vec3) -> (Vec3, Pass) {
        let Some(prey) = self.prey() else {
            return (station, Pass::Running);
        };
        match self.shots.hit_by(self.entity.id(), prey) {
            true => (station, Pass::Returning),
            false => (
                self.roll.body_of(self.state.entity(prey)).pos,
                Pass::Running,
            ),
        }
    }

    fn returning(&self, station: Vec3) -> (Vec3, Pass) {
        match self.reached(station) {
            true => (station, Pass::Running),
            false => (station, Pass::Returning),
        }
    }

    fn reached(&self, place: Vec3) -> bool {
        place.distance(self.body().pos) <= Belt::REACHED_METERS
    }

    fn prey(&self) -> Option<EntityId> {
        let shooter = Shooter {
            team: self.state[self.entity.seat()].team(),
            plating: self.entity.pattern().plating(),
        };
        self.roll
            .best(
                shooter,
                self.body().pos,
                Reach::WholeZone,
                &AssignedDamage::default(),
                None,
            )
            .map(|aim| aim.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::{AsteroidId, EntityId, TeamId};
    use crate::orbit::body::{Body, Gravity};
    use crate::pattern::EntityPattern as P;
    use crate::pattern::Slot;
    use crate::state::Rolls;
    use crate::step::fire::{Hit, Shots};

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: AsteroidId = AsteroidId(0);

    fn world() -> World {
        World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)])
    }

    fn unshot() -> Shots {
        Shots::default()
    }

    fn hit(shooter: EntityId, target: EntityId) -> Shots {
        Shots {
            hits: vec![Hit {
                shooter,
                slot: Slot::First,
                target,
                damage: 1.0,
            }],
            ..Shots::default()
        }
    }

    fn passed(world: &World, unit: EntityId, shots: &Shots) -> (Vec3, Pass) {
        let rolls = Rolls::called(&world.state);
        let roll = &rolls[HOME];
        HeldUnit::of(&world.state, world.state.entity(unit), roll, shots)
            .expect("a steered unit holds")
            .passing()
            .expect("a stationed unit of a pattern that runs passes is passing")
    }

    fn station_of(world: &World, unit: EntityId) -> Vec3 {
        let rolls = Rolls::called(&world.state);
        rolls[HOME]
            .station(unit)
            .expect("a unit that does damage is stationed")
    }

    fn radial(world: &World) -> Vec3 {
        world
            .state
            .asteroid_body(HOME)
            .pos
            .normalized()
            .expect("a radius")
    }

    fn stood(world: &mut World, unit: EntityId, at: Vec3) {
        let carried = world.state.asteroid_body(HOME).vel;
        world.state.steer(unit, Body::new(at, carried));
    }

    #[test]
    fn a_runner_with_no_enemy_holds_its_station() {
        let mut world = world();
        let runner = world.hold(0, P::Raider, HOME, 0.0);

        let (place, pass) = passed(&world, runner, &unshot());

        assert_eq!(place, station_of(&world, runner));
        assert_eq!(pass, Pass::Running);
    }

    #[test]
    fn a_runner_its_prey_did_not_hit_this_tick_runs_at_it() {
        let mut world = world();
        let runner = world.hold(0, P::Raider, HOME, 0.0);
        let prey = world.hold(1, P::Constructor, HOME, 10.0);

        let (place, pass) = passed(&world, runner, &unshot());

        assert_eq!(place, world.body(prey).pos);
        assert_eq!(pass, Pass::Running);
    }

    #[test]
    fn a_runner_its_prey_hit_this_tick_turns_for_its_station() {
        let mut world = world();
        let runner = world.hold(0, P::Raider, HOME, 0.0);
        let prey = world.hold(1, P::Lancer, HOME, 10.0);

        let (place, pass) = passed(&world, runner, &hit(prey, runner));

        assert_eq!(pass, Pass::Returning, "its prey shot it and it ran on");
        assert_eq!(place, station_of(&world, runner));
    }

    #[test]
    fn a_runner_another_enemy_hit_this_tick_keeps_running_at_its_prey() {
        let mut world = world();
        let runner = world.hold(0, P::Raider, HOME, 0.0);
        let prey = world.hold(1, P::Lancer, HOME, 6.0);
        let plinker = world.hold(1, P::Lancer, HOME, 20.0);

        let (place, pass) = passed(&world, runner, &hit(plinker, runner));

        assert_eq!(
            pass,
            Pass::Running,
            "an enemy that is not its prey turned it"
        );
        assert_eq!(place, world.body(prey).pos);
    }

    #[test]
    fn a_runner_whose_prey_hit_another_unit_keeps_running_at_it() {
        let mut world = world();
        let runner = world.hold(0, P::Raider, HOME, 0.0);
        let prey = world.hold(1, P::Lancer, HOME, 6.0);
        let fellow = world.hold(0, P::Raider, HOME, 1.0);

        let (place, pass) = passed(&world, runner, &hit(prey, fellow));

        assert_eq!(pass, Pass::Running, "a hit on another unit turned it");
        assert_eq!(place, world.body(prey).pos);
    }

    #[test]
    fn a_return_holds_for_its_station_until_it_reaches_it_and_then_runs_again() {
        let mut world = world();
        let runner = world.hold(0, P::Raider, HOME, 0.0);
        world.hold(1, P::Constructor, HOME, 10.0);
        let station = station_of(&world, runner);
        world.state.entities.set_pass(runner, Pass::Returning);

        let adrift = station + radial(&world) * (2.0 * Belt::REACHED_METERS);
        stood(&mut world, runner, adrift);
        let (place, pass) = passed(&world, runner, &unshot());
        assert_eq!(pass, Pass::Returning);
        assert_eq!(place, station, "a return steers anywhere but its station");

        stood(&mut world, runner, station);
        let (place, pass) = passed(&world, runner, &unshot());
        assert_eq!(pass, Pass::Running, "it stood at its station and returned");
        assert_eq!(place, station);
    }

    #[test]
    fn a_return_its_prey_hits_beside_it_keeps_for_its_station() {
        let mut world = world();
        let runner = world.hold(0, P::Raider, HOME, 0.0);
        let prey = world.hold(1, P::Lancer, HOME, 10.0);
        let station = station_of(&world, runner);
        world.state.entities.set_pass(runner, Pass::Returning);
        let beside = world.body(prey).pos;
        stood(&mut world, runner, beside);

        let (place, pass) = passed(&world, runner, &hit(prey, runner));

        assert_eq!(
            pass,
            Pass::Returning,
            "a second hit turned it a second time"
        );
        assert_eq!(place, station, "it ran at its prey part way home");
    }

    #[test]
    fn a_runner_takes_the_highest_threat_in_the_zone_and_nothing_outside_it() {
        let mut world = world();
        let runner = world.hold(0, P::Raider, HOME, 0.0);
        let harmless = world.hold(1, P::Constructor, HOME, 4.0);
        let dangerous = world.hold(1, P::Lancer, HOME, 8.0);

        let (place, _) = passed(&world, runner, &unshot());
        assert_eq!(
            place,
            world.body(dangerous).pos,
            "it ran at the lesser threat"
        );

        let strayed =
            world.state.asteroid_body(HOME).pos + radial(&world) * (Belt::ZONE_RADIUS_METERS + 5.0);
        stood(&mut world, dangerous, strayed);

        let (place, _) = passed(&world, runner, &unshot());

        assert_eq!(
            place,
            world.body(harmless).pos,
            "it ran at an enemy outside the zone"
        );
    }
}
