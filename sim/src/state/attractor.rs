use super::State;
use super::entity::Entity;
use super::sight::Sight;
use super::sweep::Sweep;
use crate::orbit::body::Body;
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attractor {
    pub pos: Vec3,
    pub vel: Vec3,
}

impl Attractor {
    pub fn pulling(
        state: &State,
        entity: &Entity,
        sight: &Sight,
        sweep: &Sweep,
    ) -> Option<Attractor> {
        if entity.is_flying() {
            return None;
        }
        let home = Attractor::at(
            state
                .anchor(entity.home())
                .at(state.tick(), state.gravity()),
        );
        Some(Attractor::chase(state, entity, sight, sweep, home).unwrap_or(home))
    }

    fn at(body: Body) -> Attractor {
        Attractor {
            pos: body.pos,
            vel: body.vel,
        }
    }

    fn chase(
        state: &State,
        entity: &Entity,
        sight: &Sight,
        sweep: &Sweep,
        home: Attractor,
    ) -> Option<Attractor> {
        let row = &state[entity.row()];
        if !row.is_armed() {
            return None;
        }
        let team = state[entity.seat()].team();
        let body = state.body_of(entity);
        let enemy = sweep
            .within(home.pos, row.sight.0)
            .filter_map(|id| state.entity(id))
            .filter(|other| {
                state[other.seat()].team() != team && !other.is_flying() && sight.sees(other.id())
            })
            .map(|other| (state.body_of(other), other.id()))
            .min_by(|(a, first), (b, second)| {
                a.pos
                    .distance(body.pos)
                    .total_cmp(&b.pos.distance(body.pos))
                    .then(first.cmp(second))
            })
            .map(|(body, _)| body)?;
        let toward = (body.pos - enemy.pos).normalized()?;
        Some(Attractor {
            pos: enemy.pos + toward * (0.5 * row.max_damage_range()),
            vel: enemy.vel,
        })
    }
}
