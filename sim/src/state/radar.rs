use super::State;
use super::sight::Sight;
use super::sweep::Sweep;
use crate::ids::{EntityId, SeatId};

#[derive(Clone, Debug)]
pub struct Radar {
    contacts: Vec<EntityId>,
}

impl Radar {
    pub fn beyond(state: &State, seat: SeatId, sweep: &Sweep, sight: &Sight) -> Radar {
        let Some(team) = state.seat(seat).map(|seat| seat.team()) else {
            return Radar {
                contacts: Vec::new(),
            };
        };
        let mut contacts: Vec<EntityId> = state
            .entities()
            .filter(|sensor| state[sensor.seat()].team() == team)
            .flat_map(|sensor| sweep.within(state.body_of(sensor).pos, state[sensor.row()].radar.0))
            .filter(|contact| !sight.sees(*contact))
            .collect();
        contacts.sort_unstable();
        contacts.dedup();
        Radar { contacts }
    }

    pub fn iter(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.contacts.iter().copied()
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
    use crate::roster::{LANCER, Roster, SCOUT};
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
            vec![seat(0), seat(1)],
        )
    }

    #[test]
    fn radar_reports_only_what_sight_does_not_give() {
        let mut state = state();
        let anchor = state.anchor(place()).at(Tick::ZERO, MU);
        let at = |offset: f64| Motion::Free {
            body: Body::new(anchor.pos + Vec3::new(offset, 0.0, 0.0), anchor.vel),
            flight: None,
        };
        state.spawn(SeatId(0), SCOUT, place(), at(0.0));
        let seen = state.spawn(SeatId(1), LANCER, place(), at(15.0));
        let contact = state.spawn(SeatId(1), LANCER, place(), at(40.0));
        state.spawn(SeatId(1), LANCER, place(), at(80.0));
        let sweep = state.sweep();
        let sight = Sight::of(&state, SeatId(0), &sweep);

        let radar = Radar::beyond(&state, SeatId(0), &sweep, &sight);

        assert_eq!(radar.iter().collect::<Vec<_>>(), vec![contact]);
        assert!(sight.sees(seen), "what sight gives is not a contact");
    }

    #[test]
    fn a_seat_the_match_lacks_has_no_radar() {
        let state = state();
        let sweep = state.sweep();
        let sight = Sight::of(&state, SeatId(9), &sweep);

        assert_eq!(
            Radar::beyond(&state, SeatId(9), &sweep, &sight)
                .iter()
                .count(),
            0
        );
    }
}
