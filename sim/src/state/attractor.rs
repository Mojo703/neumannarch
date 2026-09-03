//! What a unit is pulled toward this tick.

use super::State;
use super::entity::Entity;
use super::sight::Sight;
use super::sweep::Sweep;
use crate::orbit::body::Body;
use crate::vec3::Vec3;

/// The state a unit is pulled toward this tick. Decided from the snapshot
/// alone, so nothing about it is stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attractor {
    /// In meters.
    pub pos: Vec3,
    /// In meters per second.
    pub vel: Vec3,
}

impl Attractor {
    /// What pulls `entity` this tick: its flight's anchor until the arrival
    /// tick, then its home anchor, unless an enemy its seat sees is inside
    /// the leash, in which case the point at half its longest weapon range
    /// from that enemy on the line toward the ship. `sight` is `entity`'s
    /// seat's.
    pub fn of(state: &State, entity: &Entity, sight: &Sight, sweep: &Sweep) -> Attractor {
        let home = Attractor::at(
            state
                .anchor(entity.home())
                .at(state.tick(), state.gravity()),
        );
        match entity.flight().and_then(|id| state.flight(id)) {
            Some(flight) if state.tick() < flight.arrive() => {
                Attractor::at(flight.anchor(state.tick(), state.gravity()))
            }
            Some(_) => home,
            None => Attractor::chase(state, entity, sight, sweep, home).unwrap_or(home),
        }
    }

    /// The attractor holding a body where it is.
    fn at(body: Body) -> Attractor {
        Attractor {
            pos: body.pos,
            vel: body.vel,
        }
    }

    /// The point half a weapon range off the nearest enemy the seat sees
    /// inside the leash; `None` when the row is unarmed, no such enemy is
    /// there, or the ship is on top of it.
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
