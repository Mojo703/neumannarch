use std::collections::BTreeMap;

use crate::ids::SeatId;
use crate::pattern::EntityPattern;
use crate::posting::Posting;
use crate::state::State;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Placements(BTreeMap<Posting, u32>);

pub(crate) struct Reserve<'a> {
    state: &'a State,
    shortfalls: &'a BTreeMap<Posting, u32>,
}

impl<'a> Reserve<'a> {
    pub(crate) fn of(state: &'a State, shortfalls: &'a BTreeMap<Posting, u32>) -> Reserve<'a> {
        Reserve { state, shortfalls }
    }

    pub(crate) fn run(self) -> Placements {
        let mut spent: BTreeMap<SeatId, BTreeMap<EntityPattern, u32>> = BTreeMap::new();
        let mut placements = BTreeMap::new();
        for (posting, short) in self.shortfalls {
            let held = self.state[posting.seat()].reserved(posting.pattern());
            let taken = spent
                .entry(posting.seat())
                .or_default()
                .entry(posting.pattern())
                .or_default();
            let filling = (*short).min(held.saturating_sub(*taken));
            *taken += filling;
            if filling > 0 {
                placements.insert(*posting, filling);
            }
        }
        Placements(placements)
    }
}

impl Placements {
    pub(crate) fn iter(&self) -> impl Iterator<Item = Posting> + '_ {
        self.0
            .iter()
            .flat_map(|(posting, filling)| (0..*filling).map(move |_| *posting))
    }

    pub(crate) fn filling(&self, posting: Posting) -> u32 {
        self.0.get(&posting).copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::{AsteroidId, TeamId};
    use crate::materials::Materials;
    use crate::pattern::EntityPattern as P;
    use crate::state::Issued;

    const HERE: AsteroidId = AsteroidId(0);

    const AWAY: AsteroidId = AsteroidId(1);

    const MINE: SeatId = SeatId(0);

    fn wanting(reserve: BTreeMap<EntityPattern, u32>, wants: &[Issued]) -> State {
        let world = World::stocked(Materials::new(1e4, 1e4, 1e4), reserve);
        let mut asked = world.state.clone();
        for issued in wants {
            asked.apply(*issued).expect("the want stands");
        }
        asked
    }

    #[test]
    fn a_shortfall_takes_no_more_of_a_pattern_than_the_seat_holds_in_reserve() {
        let asked = wanting(
            BTreeMap::from([(P::Storage, 2)]),
            &[Issued::want(0, HERE, P::Storage, 5)],
        );

        let placements = Reserve::of(&asked, &asked.shortfalls()).run();

        assert_eq!(placements.iter().count(), 2, "five were wanted, two held");
        assert_eq!(placements.filling(Posting::of(HERE, MINE, P::Storage)), 2);
    }

    #[test]
    fn one_reserve_is_split_across_the_asteroids_that_want_the_pattern() {
        let asked = wanting(
            BTreeMap::from([(P::Storage, 1)]),
            &[
                Issued::numbered(0, 0, HERE, P::Storage, 1),
                Issued::numbered(0, 1, AWAY, P::Storage, 1),
            ],
        );

        let placements = Reserve::of(&asked, &asked.shortfalls()).run();

        assert_eq!(placements.iter().count(), 1, "the reserve holds one");
        assert_eq!(
            placements.filling(Posting::of(HERE, MINE, P::Storage)),
            1,
            "the lower asteroid is filled first"
        );
        assert_eq!(placements.filling(Posting::of(AWAY, MINE, P::Storage)), 0);
    }

    #[test]
    fn a_want_the_entities_standing_already_meet_draws_nothing() {
        let mut world = World::started(&[TeamId(0)]);
        world.fix(0, P::Shipyard, HERE);
        let mut asked = world.state.clone();
        asked
            .apply(Issued::want(0, HERE, P::Shipyard, 1))
            .expect("the want stands");

        assert_eq!(
            Reserve::of(&asked, &asked.shortfalls()).run(),
            Placements::default()
        );
    }
}
