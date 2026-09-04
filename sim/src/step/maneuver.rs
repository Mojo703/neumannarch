use crate::ids::{EntityId, RockId};
use crate::orbit::body::Body;
use crate::state::sweep::Sweep;
use crate::state::{Attractor, Entity, Motion, State};
use crate::time::Tick;
use crate::vec3::Vec3;

const STIFFNESS: f64 = 0.25;

const DAMPING: f64 = 1.0;

const SPACING: f64 = 0.5;

const CUTOFF: f64 = 2.0;

const PAIR_STRENGTH: f64 = 8.0;

const ATTRACTION_SHARE: f64 = 0.05;

pub struct Maneuver<'a> {
    state: &'a State,
    sweep: &'a Sweep,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thrust {
    pub entity: EntityId,
    pub accel: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Thrusts(Vec<Thrust>);

impl<'a> Maneuver<'a> {
    pub fn of(state: &'a State, sweep: &'a Sweep) -> Maneuver<'a> {
        Maneuver { state, sweep }
    }

    pub fn spawn_body(state: &State, rock: RockId, tick: Tick) -> Body {
        let home = state[rock].orbit().at(tick, state.gravity());
        let already = state
            .standing_at(rock)
            .filter(|entity| entity.motion() != Motion::Fixed)
            .count();
        let radial = home.pos.normalized().unwrap_or(Vec3::ZERO);
        Body::new(home.pos + radial * (SPACING * already as f64), home.vel)
    }

    pub fn run(self) -> Thrusts {
        Thrusts(
            self.state
                .entities()
                .filter_map(|entity| self.thrust(entity))
                .collect(),
        )
    }

    fn thrust(&self, entity: &Entity) -> Option<Thrust> {
        let Motion::Free { body, .. } = entity.motion() else {
            return None;
        };
        let row = &self.state[entity.row()];
        let pull =
            Attractor::pulling(self.state, entity, self.sweep).map_or(Vec3::ZERO, |attractor| {
                (attractor.pos - body.pos) * STIFFNESS + (attractor.vel - body.vel) * DAMPING
            });
        let accel = pull + self.pairs(entity, body);
        Some(Thrust {
            entity: entity.id(),
            accel: within(accel, row.manoeuvring.0),
        })
    }

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
    pub fn of(&self, id: EntityId) -> Vec3 {
        self.0
            .binary_search_by_key(&id, |thrust| thrust.entity)
            .map_or(Vec3::ZERO, |at| self.0[at].accel)
    }
}

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
    use super::*;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::orbit::body::Gravity;
    use crate::roster::FRIGATE;

    const ROCK: RockId = RockId(0);

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    fn world() -> World {
        World::ring(GRAVITY, 1, &[TeamId(0)])
    }

    #[test]
    fn each_unit_spawns_one_spacing_further_out_than_the_last() {
        let mut world = world();
        let home = world.state.rock_body(ROCK);
        let radial = home.pos.normalized().expect("a radius");
        for already in 0..3 {
            let spawn = Maneuver::spawn_body(&world.state, ROCK, world.state.tick());
            let expected = home.pos + radial * (SPACING * f64::from(already));
            assert!(
                spawn.pos.distance(expected) < 1e-9,
                "unit {already} spawns at {spawn:?}"
            );
            assert_eq!(spawn.vel, home.vel);
            world.free(0, FRIGATE, ROCK, spawn);
        }
    }

    #[test]
    fn a_structure_at_the_rock_does_not_move_a_spawn() {
        let mut world = world();
        world.fix(0, FRIGATE, ROCK);
        assert_eq!(
            Maneuver::spawn_body(&world.state, ROCK, world.state.tick()).pos,
            world.state.rock_body(ROCK).pos
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
