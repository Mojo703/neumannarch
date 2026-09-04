use std::collections::BTreeMap;

use super::power::Power;
use crate::belt::Belt;
use crate::ids::{EntityId, RockId};
use crate::state::{Entity, Motion, State};
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub own: f64,
    pub enemy: f64,
    pub own_gradient: Vec3,
    pub enemy_gradient: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Fraction(f64);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields(BTreeMap<EntityId, Sample>);

impl Fields {
    pub fn of(state: &State) -> Fields {
        let mut rolls: BTreeMap<RockId, Vec<&Entity>> = BTreeMap::new();
        for entity in state.entities() {
            if entity.motion() == Motion::Fixed {
                continue;
            }
            if let Some(rock) = entity.standing(state.tick()) {
                rolls.entry(rock).or_default().push(entity);
            }
        }
        let mut fields = Fields(BTreeMap::new());
        for roll in rolls.values_mut() {
            roll.sort_by(|a, b| {
                state
                    .body_of(a)
                    .pos
                    .x
                    .total_cmp(&state.body_of(b).pos.x)
                    .then(a.id().cmp(&b.id()))
            });
            fields.over(state, roll);
        }
        fields
    }

    pub fn at(&self, id: EntityId) -> Sample {
        self.0.get(&id).copied().unwrap_or_default()
    }

    fn over(&mut self, state: &State, roll: &[&Entity]) {
        for entity in roll {
            self.0.insert(
                entity.id(),
                Sample {
                    own: power(state, entity).get(),
                    ..Sample::default()
                },
            );
        }
        for (at, source) in roll.iter().enumerate() {
            let from = state.body_of(source).pos;
            for other in roll[at + 1..]
                .iter()
                .take_while(|other| state.body_of(other).pos.x - from.x <= Belt::FIELD_SCALE_METERS)
            {
                let offset = state.body_of(other).pos - from;
                let (weight, slope) = kernel(offset.length());
                let friendly = state[source.seat()].team() == state[other.seat()].team();
                self.add(
                    source.id(),
                    friendly,
                    power(state, other),
                    weight,
                    offset * slope,
                );
                self.add(
                    other.id(),
                    friendly,
                    power(state, source),
                    weight,
                    offset * -slope,
                );
            }
        }
    }

    fn add(&mut self, id: EntityId, friendly: bool, power: Power, weight: f64, toward: Vec3) {
        let Some(sample) = self.0.get_mut(&id) else {
            return;
        };
        let (strength, gradient) = match friendly {
            true => (&mut sample.own, &mut sample.own_gradient),
            false => (&mut sample.enemy, &mut sample.enemy_gradient),
        };
        let power = power.get();
        *strength += power * weight;
        *gradient += toward * power;
    }
}

fn power(state: &State, entity: &Entity) -> Power {
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
    pub fn hostile(&self) -> Option<Fraction> {
        Fraction::of(self.enemy, self.own + self.enemy)
    }

    pub fn own_lean(&self) -> Vec3 {
        match self.own > 0.0 {
            true => (self.own_gradient * (Belt::FIELD_SCALE_METERS / self.own)).capped(1.0),
            false => Vec3::ZERO,
        }
    }

    pub fn retreat(&self) -> Vec3 {
        self.hostile().map_or(Vec3::ZERO, |hostile| {
            (-self.enemy_gradient)
                .normalized()
                .map_or(Vec3::ZERO, |away| away * hostile.get())
        })
    }
}

impl Fraction {
    pub fn of(part: f64, whole: f64) -> Option<Fraction> {
        (whole > 0.0).then(|| Fraction((part / whole).clamp(0.0, 1.0)))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::{RockId, SeatId, TeamId};
    use crate::orbit::body::Gravity;
    use crate::roster::{CONSTRUCTOR, FRIGATE};
    use crate::state::{Flight, Send};

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const HOME: RockId = RockId(0);

    fn world() -> World {
        World::ring(GRAVITY, 2, &[TeamId(0), TeamId(1)])
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

        let sample = Fields::of(&world.state).at(alone);

        assert!((sample.own - power * (1.0 + kernel(4.0))).abs() < 1e-9);
        assert!((sample.enemy - power * kernel(6.0)).abs() < 1e-9);
        assert_eq!(world.state[ally].hp(), full);
        assert_eq!(world.state[enemy].hp(), full);
    }

    #[test]
    fn a_unit_beyond_the_scale_adds_nothing() {
        let mut world = world();
        let alone = world.hold(0, FRIGATE, HOME, 0.0);
        world.hold(1, FRIGATE, HOME, Belt::FIELD_SCALE_METERS + 1.0);

        let sample = Fields::of(&world.state).at(alone);

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

        let gradient = Fields::of(&world.state).at(reader).own_gradient;

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
            .rock_body(HOME)
            .pos
            .normalized()
            .expect("a radius");

        let gradient = Fields::of(&world.state).at(alone).own_gradient;

        assert!(gradient.dot(outward) > 0.0, "{gradient:?}");
    }

    #[test]
    fn the_hostile_fraction_is_none_where_no_power_stands() {
        let mut world = world();
        let alone = world.hold(0, CONSTRUCTOR, HOME, 0.0);

        assert_eq!(Fields::of(&world.state).at(alone).hostile(), None);
    }

    #[test]
    fn the_hostile_fraction_rises_as_the_enemy_outweighs_the_unit() {
        let mut world = world();
        let alone = world.hold(0, FRIGATE, HOME, 0.0);
        let even = Fields::of(&world.state).at(alone).hostile();
        assert_eq!(even, Fraction::of(0.0, 1.0));

        world.hold(1, FRIGATE, HOME, 1.0);
        let outnumbered = Fields::of(&world.state)
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
        let together = Fields::of(&world.state).at(reader).own;
        let send = Send::joining(&world.state, HOME, RockId(1), SeatId(0), &[flier])
            .expect("a send across the ring");
        world.launch(flier, HOME, 2.0, Flight::new(HOME, send.schedule));

        assert_eq!(
            Fields::of(&world.state).at(reader).own,
            together,
            "a unit whose send is forming stands where it is"
        );

        while !world.state[flier].is_flying(world.state.tick()) {
            world.state.advance();
        }

        assert!(Fields::of(&world.state).at(reader).own < together);
        assert_eq!(Fields::of(&world.state).at(flier), Sample::default());
    }

    #[test]
    fn a_structure_is_neither_a_source_nor_a_reader() {
        let mut world = world();
        let reader = world.hold(0, FRIGATE, HOME, 0.0);
        let alone = Fields::of(&world.state).at(reader).own;

        let fixed = world.fix(0, FRIGATE, HOME);

        assert_eq!(Fields::of(&world.state).at(reader).own, alone);
        assert_eq!(Fields::of(&world.state).at(fixed), Sample::default());
    }
}
