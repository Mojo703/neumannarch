use std::collections::BTreeMap;

use crate::ids::{AsteroidId, SeatId};
use crate::materials::Materials;
use crate::posting::Posting;
use crate::roster::Roster;
use crate::state::{Command, Issued, Rejected, State};
use crate::step::fulfilment::{Fulfilment, SendSchedules};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preview {
    pub shortfalls: BTreeMap<Posting, ShortfallFilling>,
    pub refund: Materials,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShortfallFilling {
    pub from_reserve: u32,
    pub sent_from: BTreeMap<AsteroidId, u32>,
    pub to_build: u32,
}

impl State {
    pub fn preview(&self, seat: SeatId, wants: &[Command]) -> Result<Preview, Rejected> {
        let mut asked = self.clone();
        for command in wants {
            asked.apply(Issued {
                seat,
                seq: 0,
                command: *command,
            })?;
        }
        Ok(Preview::of(&asked, seat).added_beyond(&Preview::of(self, seat)))
    }
}

impl Preview {
    pub fn cost_to_build(&self, roster: &Roster) -> Materials {
        self.shortfalls
            .iter()
            .fold(Materials::ZERO, |sum, (posting, filling)| {
                sum + roster[posting.row()].cost * f64::from(filling.to_build)
            })
    }

    fn of(state: &State, seat: SeatId) -> Preview {
        let mut fulfilment = Fulfilment::of(state);
        let assignment = fulfilment.assign();
        let assigned = fulfilment.settle(&assignment, SendSchedules::nothing_held_back());
        let mut shortfalls: BTreeMap<Posting, ShortfallFilling> = BTreeMap::new();
        for posting in assignment
            .from_reserve
            .iter()
            .filter(|posting| posting.seat() == seat)
        {
            shortfalls.entry(*posting).or_default().from_reserve += 1;
        }
        for (route, members) in assignment
            .sent_units
            .iter()
            .filter(|(route, _)| route.seat == seat)
        {
            for entity in members.iter().map(|id| state.entity(*id)) {
                let arriving = shortfalls
                    .entry(Posting::of(route.destination, seat, entity.row()))
                    .or_default();
                *arriving.sent_from.entry(route.source).or_default() += 1;
            }
        }
        for (posting, short) in assignment
            .still_short
            .iter()
            .filter(|(posting, _)| posting.seat() == seat)
        {
            shortfalls.entry(*posting).or_default().to_build += short;
        }
        Preview {
            shortfalls,
            refund: assigned
                .cancellations
                .iter()
                .filter(|cancellation| cancellation.posting.seat() == seat)
                .fold(Materials::ZERO, |sum, cancellation| {
                    sum + cancellation.refund(state)
                }),
        }
    }

    fn added_beyond(&self, already: &Preview) -> Preview {
        Preview {
            shortfalls: self
                .shortfalls
                .iter()
                .map(|(posting, filling)| {
                    (
                        *posting,
                        filling.added_beyond(already.shortfalls.get(posting)),
                    )
                })
                .filter(|(_, filling)| filling.fills_something())
                .collect(),
            refund: (self.refund - already.refund).map(|amount| amount.max(0.0)),
        }
    }
}

impl ShortfallFilling {
    fn added_beyond(&self, already: Option<&ShortfallFilling>) -> ShortfallFilling {
        let Some(already) = already else {
            return self.clone();
        };
        ShortfallFilling {
            from_reserve: self.from_reserve.saturating_sub(already.from_reserve),
            sent_from: self
                .sent_from
                .iter()
                .map(|(from, count)| {
                    (
                        *from,
                        count.saturating_sub(
                            already.sent_from.get(from).copied().unwrap_or_default(),
                        ),
                    )
                })
                .filter(|(_, count)| *count > 0)
                .collect(),
            to_build: self.to_build.saturating_sub(already.to_build),
        }
    }

    fn fills_something(&self) -> bool {
        self.from_reserve > 0 || self.to_build > 0 || !self.sent_from.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::fixture::World;
    use crate::ids::{RowId, TeamId};
    use crate::roster::{FRIGATE, SHIPYARD};
    use crate::state::Issued;

    const HERE: AsteroidId = AsteroidId(0);

    const AWAY: AsteroidId = AsteroidId(1);

    const ME: SeatId = SeatId(0);

    fn world() -> World {
        World::started(&[TeamId(0), TeamId(1)])
    }

    fn want(asteroid: AsteroidId, row: RowId, count: u32) -> Command {
        Command::Want {
            asteroid,
            row,
            count,
        }
    }

    fn previewed(world: &World, wants: &[Command]) -> Preview {
        world.state.preview(ME, wants).expect("the wants stand")
    }

    fn mine(asteroid: AsteroidId, row: RowId) -> Posting {
        Posting::of(asteroid, ME, row)
    }

    #[test]
    fn a_want_the_reserve_fills_is_placed_and_costs_nothing() {
        let world = World::stocked(
            Materials::new(1e4, 1e4, 1e4),
            BTreeMap::from([(SHIPYARD, 2)]),
        );

        let preview = previewed(&world, &[want(HERE, SHIPYARD, 2)]);

        let filling = preview
            .shortfalls
            .get(&mine(HERE, SHIPYARD))
            .expect("the shipyard is wanted here");
        assert_eq!(filling.from_reserve, 2, "the reserve holds two");
        assert_eq!(filling.to_build, 0);
        assert_eq!(
            preview.cost_to_build(world.state.roster()),
            Materials::ZERO,
            "a reserve unit is paid for"
        );
        assert_eq!(preview.refund, Materials::ZERO);
    }

    #[test]
    fn a_want_no_reserve_or_surplus_fills_costs_the_rows_cost_for_every_unit_to_build() {
        let mut world = world();
        world.fix(0, SHIPYARD, HERE);

        let preview = previewed(&world, &[want(HERE, FRIGATE, 3)]);

        let filling = preview
            .shortfalls
            .get(&mine(HERE, FRIGATE))
            .expect("three are short");
        assert_eq!(filling.to_build, 3, "a count of units, never of openings");
        assert_eq!(filling.from_reserve, 0);
        assert_eq!(
            preview.cost_to_build(world.state.roster()),
            world.state[FRIGATE].cost * 3.0
        );
    }

    #[test]
    fn a_hover_reports_only_what_it_adds_to_the_shortfall_already_being_filled() {
        let mut world = world();
        world.fix(0, SHIPYARD, HERE);
        world.tick(&[Issued::want(0, HERE, FRIGATE, 2)]);

        let preview = previewed(&world, &[want(HERE, FRIGATE, 3)]);

        let filling = preview
            .shortfalls
            .get(&mine(HERE, FRIGATE))
            .expect("one more is short");
        assert_eq!(
            filling.to_build, 1,
            "the two already short are not the hover's"
        );
        assert_eq!(
            preview.cost_to_build(world.state.roster()),
            world.state[FRIGATE].cost
        );
    }

    #[test]
    fn lowering_a_want_refunds_the_share_of_the_cost_its_frame_consumed() {
        let mut world = world();
        world.fix(0, SHIPYARD, HERE);
        world.tick(&[Issued::want(0, HERE, FRIGATE, 1)]);
        world.run(60);
        let frame = world
            .state
            .frames()
            .iter()
            .find(|frame| frame.row() == FRIGATE)
            .expect("the frigate is building");
        let cost = world.state[FRIGATE].cost;
        let paid = cost * (frame.progress() / cost.total());

        let preview = previewed(&world, &[want(HERE, FRIGATE, 0)]);

        assert!(paid.total() > 0.0, "the frame consumed something");
        assert_eq!(preview.refund, paid);
        assert_eq!(preview.cost_to_build(world.state.roster()), Materials::ZERO);
    }

    #[test]
    fn lowering_a_want_the_state_is_still_short_of_adds_no_filling() {
        let mut world = world();
        world.fix(0, SHIPYARD, HERE);
        world.tick(&[Issued::want(0, HERE, FRIGATE, 3)]);

        let preview = previewed(&world, &[want(HERE, FRIGATE, 2)]);

        assert_eq!(
            preview.shortfalls.get(&mine(HERE, FRIGATE)),
            None,
            "a want coming down adds no filling of its own"
        );
    }

    #[test]
    fn a_send_reports_its_units_arriving_from_the_asteroid_they_leave() {
        let mut world = world();
        world.hold(0, FRIGATE, HERE, 0.0);

        let preview = previewed(&world, &[want(HERE, FRIGATE, 0), want(AWAY, FRIGATE, 1)]);

        let filling = preview
            .shortfalls
            .get(&mine(AWAY, FRIGATE))
            .expect("one is wanted away");
        assert_eq!(filling.sent_from, BTreeMap::from([(HERE, 1)]));
        assert_eq!(filling.to_build, 0, "a unit on its way builds nothing");
        assert_eq!(preview.cost_to_build(world.state.roster()), Materials::ZERO);
    }

    #[test]
    fn a_want_the_state_refuses_previews_nothing_and_says_why() {
        let world = world();

        let refused = world
            .state
            .preview(ME, &[want(HERE, FRIGATE, crate::state::MAX_WANT + 1)]);

        assert_eq!(refused, Err(Rejected::TooMany));
    }

    #[test]
    fn a_preview_of_no_wants_is_empty() {
        let mut world = world();
        world.fix(0, SHIPYARD, HERE);
        world.tick(&[Issued::want(0, HERE, FRIGATE, 2)]);

        assert_eq!(previewed(&world, &[]), Preview::default());
    }
}
