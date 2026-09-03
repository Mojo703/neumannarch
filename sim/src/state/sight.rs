//! What one seat sees: the union of its team's sight ranges.

use super::State;
use super::sweep::Sweep;
use crate::ids::{EntityId, SeatId};

/// The entities one seat sees this tick: every entity of its team, and
/// every entity inside the sight range of one of them. The only query that
/// answers whether a seat sees an entity.
#[derive(Clone, Debug)]
pub struct Sight {
    seen: Vec<EntityId>,
}

impl Sight {
    /// What `seat` sees in the snapshot, over its `sweep`. A seat the match
    /// does not have sees nothing.
    pub fn of(state: &State, seat: SeatId, sweep: &Sweep) -> Sight {
        let mut seen = Vec::new();
        if let Some(team) = state.seat(seat).map(|seat| seat.team()) {
            for entity in state
                .entities()
                .filter(|entity| state[entity.seat()].team() == team)
            {
                seen.push(entity.id());
                let range = state[entity.row()].sight.0;
                seen.extend(sweep.within(state.body_of(entity).pos, range));
            }
        }
        seen.sort_unstable();
        seen.dedup();
        Sight { seen }
    }

    /// Whether the seat sees `entity`.
    pub fn sees(&self, entity: EntityId) -> bool {
        self.seen.binary_search(&entity).is_ok()
    }

    /// Everything the seat sees, in id order.
    pub fn iter(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.seen.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::ids::{RockId, TeamId};
    use crate::materials::Materials;
    use crate::orbit::body::{Body, Gravity};
    use crate::orbit::elements::Orbit;
    use crate::place::{Band, Place};
    use crate::roster::{FRIGATE, Roster, SCOUT};
    use crate::state::rock::Rock;
    use crate::state::seat::Seat;
    use crate::state::{Motion, State};
    use crate::time::Tick;
    use crate::vec3::Vec3;

    const MU: Gravity = Gravity::new(4.4e17);

    fn place() -> Place {
        Place {
            rock: RockId(0),
            band: Band::Inner,
        }
    }

    /// Two seats on one team and one on another, over one rock.
    fn state() -> State {
        let radius = 1.0e7;
        let speed = (MU.mu() / radius).sqrt();
        let body = Body::new(Vec3::new(radius, 0.0, 0.0), Vec3::new(0.0, 0.0, -speed));
        let orbit = Orbit::from_body(body, Tick::ZERO, MU).expect("a circular orbit");
        let rock = Rock::new(orbit, Materials::new(1.0, 1.0, 1.0), 100.0);
        let seat = |team| Seat::new(TeamId(team), Materials::ZERO, BTreeMap::new());
        State::new(
            Tick(120_000),
            0,
            MU,
            Roster::shipped(),
            vec![rock],
            vec![seat(0), seat(0), seat(1)],
        )
    }

    /// The frigate's sight is eight meters, the scout's twenty.
    #[test]
    fn a_seat_sees_its_team_and_what_its_team_is_near() {
        let mut state = state();
        let anchor = state.anchor(place()).at(Tick::ZERO, MU);
        let at = |offset: f64| Motion::Free {
            body: Body::new(anchor.pos + Vec3::new(offset, 0.0, 0.0), anchor.vel),
            flight: None,
        };
        let mine = state.spawn(SeatId(0), FRIGATE, place(), at(0.0));
        let ally = state.spawn(SeatId(1), FRIGATE, place(), at(100.0));
        let near = state.spawn(SeatId(2), FRIGATE, place(), at(6.0));
        let near_the_ally = state.spawn(SeatId(2), FRIGATE, place(), at(104.0));
        let far = state.spawn(SeatId(2), FRIGATE, place(), at(50.0));
        let sweep = state.sweep();

        let sight = Sight::of(&state, SeatId(0), &sweep);

        assert!(sight.sees(mine));
        assert!(sight.sees(ally), "teammates are seen wherever they are");
        assert!(sight.sees(near));
        assert!(sight.sees(near_the_ally), "a teammate's sight is shared");
        assert!(!sight.sees(far));
        assert_eq!(
            sight.iter().collect::<Vec<_>>(),
            vec![mine, ally, near, near_the_ally]
        );
    }

    #[test]
    fn a_longer_ranged_row_widens_what_its_seat_sees() {
        let mut state = state();
        let anchor = state.anchor(place()).at(Tick::ZERO, MU);
        let at = |offset: f64| Motion::Free {
            body: Body::new(anchor.pos + Vec3::new(offset, 0.0, 0.0), anchor.vel),
            flight: None,
        };
        state.spawn(SeatId(0), SCOUT, place(), at(0.0));
        let enemy = state.spawn(SeatId(2), FRIGATE, place(), at(15.0));
        let sweep = state.sweep();
        assert!(Sight::of(&state, SeatId(0), &sweep).sees(enemy));
    }

    #[test]
    fn a_seat_the_match_lacks_sees_nothing() {
        let mut state = state();
        state.spawn(SeatId(0), FRIGATE, place(), Motion::Fixed);
        let sweep = state.sweep();
        assert_eq!(Sight::of(&state, SeatId(9), &sweep).iter().count(), 0);
    }
}
