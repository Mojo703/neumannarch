use std::collections::BTreeMap;

use super::power::Power;
use crate::belt::Belt;
use crate::ids::{EntityId, TeamId};
use crate::state::{Entity, Roll, Rolls, State};
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Sample {
    pub(crate) own: f64,
    pub(crate) enemy: f64,
    pub(crate) own_gradient: Vec3,
    pub(crate) enemy_gradient: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(crate) struct Fraction(f64);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields(BTreeMap<EntityId, Sample>);

impl Fields {
    pub(crate) fn among(state: &State, rolls: &Rolls) -> Fields {
        let mut fields = Fields(BTreeMap::new());
        for roll in rolls.iter() {
            fields.over(state, roll);
        }
        fields
    }

    pub fn at(&self, id: EntityId) -> Sample {
        self.0.get(&id).copied().unwrap_or_default()
    }

    fn over(&mut self, state: &State, roll: &Roll) {
        let mut along_x: Vec<Entity> = roll
            .standing()
            .filter(|entity| entity.steered().is_some())
            .collect();
        along_x.sort_by(|first, second| {
            roll.body_of(*first)
                .pos
                .x
                .total_cmp(&roll.body_of(*second).pos.x)
                .then(first.id().cmp(&second.id()))
        });
        let places: Vec<Vec3> = along_x.iter().map(|at| roll.body_of(*at).pos).collect();
        let powers: Vec<f64> = along_x.iter().map(|at| power(state, *at).get()).collect();
        let teams: Vec<TeamId> = along_x.iter().map(|at| state[at.seat()].team()).collect();
        let mut samples: Vec<Sample> = powers
            .iter()
            .map(|own| Sample {
                own: *own,
                ..Sample::default()
            })
            .collect();
        for source in 0..along_x.len() {
            for other in source + 1..along_x.len() {
                let offset = places[other] - places[source];
                if offset.x > Belt::FIELD_SCALE_METERS {
                    break;
                }
                let (weight, slope) = kernel(offset.length());
                let friendly = teams[source] == teams[other];
                samples[source].add(friendly, powers[other], weight, offset * slope);
                samples[other].add(friendly, powers[source], weight, offset * -slope);
            }
        }
        self.0.extend(
            along_x
                .iter()
                .zip(samples)
                .map(|(entity, sample)| (entity.id(), sample)),
        );
    }
}

impl Sample {
    fn add(&mut self, friendly: bool, power: f64, weight: f64, toward: Vec3) {
        let (strength, gradient) = match friendly {
            true => (&mut self.own, &mut self.own_gradient),
            false => (&mut self.enemy, &mut self.enemy_gradient),
        };
        *strength += power * weight;
        *gradient += toward * power;
    }
}

fn power(state: &State, entity: Entity) -> Power {
    Power::of(&state[entity.row()], entity.hp())
}

fn kernel(distance: f64) -> (f64, f64) {
    let reach = (distance / Belt::FIELD_SCALE_METERS).min(1.0);
    let falling = 1.0 - reach * reach;
    (
        falling * falling,
        4.0 * falling / (Belt::FIELD_SCALE_METERS * Belt::FIELD_SCALE_METERS),
    )
}

impl Sample {
    pub(crate) fn hostile(&self) -> Option<Fraction> {
        Fraction::of(self.enemy, self.own + self.enemy)
    }

    pub(crate) fn own_lean(&self) -> Vec3 {
        match self.own > 0.0 {
            true => (self.own_gradient * (Belt::FIELD_SCALE_METERS / self.own)).capped(1.0),
            false => Vec3::ZERO,
        }
    }

    pub(crate) fn retreat(&self) -> Vec3 {
        self.hostile().map_or(Vec3::ZERO, |hostile| {
            (-self.enemy_gradient)
                .normalized()
                .map_or(Vec3::ZERO, |away| away * hostile.get())
        })
    }
}

impl Fraction {
    pub(crate) fn of(part: f64, whole: f64) -> Option<Fraction> {
        (whole > 0.0).then(|| Fraction((part / whole).clamp(0.0, 1.0)))
    }

    pub(crate) fn get(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::{AsteroidId, SeatId, TeamId};
    use crate::orbit::body::Gravity;
    use crate::roster::{CONSTRUCTOR, FRIGATE};
    use crate::state::{Flight, Rolls, Route, Send};

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: AsteroidId = AsteroidId(0);

    fn world() -> World {
        World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)])
    }

    fn fields(state: &State) -> Fields {
        Fields::among(state, &Rolls::called(state))
    }

    fn kernel(distance: f64) -> f64 {
        let reach = distance / Belt::FIELD_SCALE_METERS;
        let falling = 1.0 - reach * reach;
        falling * falling
    }

    #[test]
    fn a_sample_is_the_sum_of_power_times_the_kernel_over_its_side() {
        let mut world = world();
        let alone = world.hold(0, FRIGATE, HOME, 0.0);
        let ally = world.hold(0, FRIGATE, HOME, 4.0);
        let enemy = world.hold(1, FRIGATE, HOME, 6.0);
        let full = world.state[FRIGATE].hp.0;
        let power = Power::of(&world.state[FRIGATE], full).get();

        let sample = fields(&world.state).at(alone);

        assert!((sample.own - power * (1.0 + kernel(4.0))).abs() < 1e-9);
        assert!((sample.enemy - power * kernel(6.0)).abs() < 1e-9);
        assert_eq!(world.state.entity(ally).hp(), full);
        assert_eq!(world.state.entity(enemy).hp(), full);
    }

    #[test]
    fn a_unit_beyond_the_scale_adds_nothing() {
        let mut world = world();
        let alone = world.hold(0, FRIGATE, HOME, 0.0);
        world.hold(1, FRIGATE, HOME, Belt::FIELD_SCALE_METERS + 1.0);

        let sample = fields(&world.state).at(alone);

        assert_eq!(sample.enemy, 0.0);
        assert_eq!(sample.enemy_gradient, Vec3::ZERO);
    }

    #[test]
    fn the_gradient_matches_a_central_difference_of_the_field() {
        let mut world = world();
        let reader = world.hold(0, FRIGATE, HOME, 5.0);
        for out in [0.0, 2.0, 9.0, 12.0] {
            world.hold(0, FRIGATE, HOME, out);
        }
        let at = world.body(reader).pos;
        let sources: Vec<(Vec3, f64)> = world
            .state
            .entities()
            .map(|entity| {
                (
                    world.state.body_of(entity).pos,
                    Power::of(&world.state[entity.row()], entity.hp()).get(),
                )
            })
            .collect();
        let field = |point: Vec3| -> f64 {
            sources
                .iter()
                .map(|(pos, power)| power * kernel(pos.distance(point).min(15.0)))
                .sum()
        };
        let step = 1e-4;

        let gradient = fields(&world.state).at(reader).own_gradient;

        for (along, analytic) in [
            (Vec3::new(1.0, 0.0, 0.0), gradient.x),
            (Vec3::new(0.0, 1.0, 0.0), gradient.y),
            (Vec3::new(0.0, 0.0, 1.0), gradient.z),
        ] {
            let difference = (field(at + along * step) - field(at - along * step)) / (2.0 * step);
            assert!(
                (analytic - difference).abs() < 1e-3 * difference.abs().max(1.0),
                "along {along:?}: {analytic} against {difference}"
            );
        }
    }

    #[test]
    fn the_own_gradient_points_toward_where_the_allies_are() {
        let mut world = world();
        let alone = world.hold(0, FRIGATE, HOME, 0.0);
        world.hold(0, FRIGATE, HOME, 5.0);
        let outward = world
            .state
            .asteroid_body(HOME)
            .pos
            .normalized()
            .expect("a radius");

        let gradient = fields(&world.state).at(alone).own_gradient;

        assert!(gradient.dot(outward) > 0.0, "{gradient:?}");
    }

    #[test]
    fn the_hostile_fraction_is_none_where_no_power_stands() {
        let mut world = world();
        let alone = world.hold(0, CONSTRUCTOR, HOME, 0.0);

        assert_eq!(fields(&world.state).at(alone).hostile(), None);
    }

    #[test]
    fn the_hostile_fraction_rises_as_the_enemy_outweighs_the_unit() {
        let mut world = world();
        let alone = world.hold(0, FRIGATE, HOME, 0.0);
        let even = fields(&world.state).at(alone).hostile();
        assert_eq!(even, Fraction::of(0.0, 1.0));

        world.hold(1, FRIGATE, HOME, 1.0);
        let outnumbered = fields(&world.state)
            .at(alone)
            .hostile()
            .expect("power stands here");

        assert!(outnumbered.get() > 0.4, "{outnumbered:?}");
    }

    #[test]
    fn a_flying_unit_is_neither_a_source_nor_a_reader() {
        let mut world = world();
        let reader = world.hold(0, FRIGATE, HOME, 0.0);
        let flier = world.hold(0, FRIGATE, HOME, 2.0);
        let together = fields(&world.state).at(reader).own;
        let send = Send::joining(
            &world.state,
            Route {
                source: HOME,
                destination: AsteroidId(1),
                seat: SeatId(0),
            },
            &[flier],
        )
        .expect("a send across the ring");
        world.launch(flier, HOME, 2.0, Flight::new(HOME, send.schedule));

        assert_eq!(
            fields(&world.state).at(reader).own,
            together,
            "a unit whose send is forming stands where it is"
        );

        while !world.state.entity(flier).is_flying() {
            world.state.advance();
        }

        assert!(fields(&world.state).at(reader).own < together);
        assert_eq!(fields(&world.state).at(flier), Sample::default());
    }

    #[test]
    fn a_structure_is_neither_a_source_nor_a_reader() {
        let mut world = world();
        let reader = world.hold(0, FRIGATE, HOME, 0.0);
        let alone = fields(&world.state).at(reader).own;

        let fixed = world.fix(0, FRIGATE, HOME);

        assert_eq!(fields(&world.state).at(reader).own, alone);
        assert_eq!(fields(&world.state).at(fixed), Sample::default());
    }
}
