use super::State;
use super::entity::Entity;
use super::sweep::Sweep;
use crate::belt::Belt;
use crate::ids::RockId;
use crate::orbit::body::Body;
use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attractor {
    pub pos: Vec3,
    pub vel: Vec3,
}

impl Attractor {
    pub fn pulling(state: &State, entity: &Entity, sweep: &Sweep) -> Option<Attractor> {
        let standing = entity.standing(state.tick())?;
        let home = Attractor::at(state.rock_body(standing));
        Some(Attractor::chase(state, entity, sweep, standing, home).unwrap_or(home))
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
        sweep: &Sweep,
        here: RockId,
        home: Attractor,
    ) -> Option<Attractor> {
        let row = &state[entity.row()];
        if !row.is_armed() {
            return None;
        }
        let team = state[entity.seat()].team();
        let body = state.body_of(entity);
        let enemy = sweep
            .within(home.pos, Belt::ZONE_RADIUS_METERS)
            .filter_map(|id| state.entity(id))
            .filter(|other| {
                state[other.seat()].team() != team && other.standing(state.tick()) == Some(here)
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
