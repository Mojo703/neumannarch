use std::collections::BTreeMap;

use neumannarch_sim::pattern::{EntityPattern, Kind};
use neumannarch_sim::state::view::View;
use neumannarch_sim::state::{Command, MAX_WANT};
use neumannarch_sim::{AsteroidId, Posting};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sending {
    pub from: AsteroidId,
    pub to: AsteroidId,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moving {
    pub pattern: EntityPattern,
    pub count: u32,
}

impl Sending {
    pub fn present(view: &View, asteroid: AsteroidId) -> u32 {
        standing_units(view, asteroid)
            .iter()
            .map(|moving| moving.count)
            .sum()
    }

    pub fn patterns(&self, view: &View) -> Vec<Moving> {
        let mut left = self.count;
        let mut moving: Vec<Moving> = Vec::new();
        for standing in standing_units(view, self.from).into_iter().rev() {
            if left == 0 {
                break;
            }
            let count = standing.count.min(left);
            left -= count;
            moving.push(Moving {
                pattern: standing.pattern,
                count,
            });
        }
        moving.reverse();
        moving
    }

    pub fn commands(&self, view: &View) -> Vec<Command> {
        self.patterns(view)
            .into_iter()
            .flat_map(|moving| {
                let Moving { pattern, count } = moving;
                [
                    Command::Want {
                        asteroid: self.from,
                        pattern,
                        count: view
                            .want_of(Posting::of(self.from, view.seat, pattern))
                            .saturating_sub(count),
                    },
                    Command::Want {
                        asteroid: self.to,
                        pattern,
                        count: (view.want_of(Posting::of(self.to, view.seat, pattern)) + count)
                            .min(MAX_WANT),
                    },
                ]
            })
            .collect()
    }
}

fn standing_units(view: &View, asteroid: AsteroidId) -> Vec<Moving> {
    let mut held: BTreeMap<EntityPattern, u32> = BTreeMap::new();
    for unit in view
        .present
        .iter()
        .filter(|unit| unit.seat == view.seat && unit.at.standing() == Some(asteroid))
        .filter(|unit| unit.home == asteroid)
        .filter(|unit| unit.pattern.kind() == Kind::Unit)
    {
        *held.entry(unit.pattern).or_insert(0) += 1;
    }
    let mut moving: Vec<Moving> = held
        .into_iter()
        .map(|(pattern, count)| Moving { pattern, count })
        .collect();
    moving.sort_by(|a, b| {
        b.pattern
            .cost()
            .total()
            .total_cmp(&a.pattern.cost().total())
            .then(a.pattern.cmp(&b.pattern))
    });
    moving
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::local::Local;

    fn asteroid(at: u32) -> AsteroidId {
        AsteroidId(at)
    }

    fn placed() -> Local {
        let mut local = Local::start(1);
        local.want(&[(asteroid(0), EntityPattern::Constructor, 1)]);
        local.want(&[(asteroid(2), EntityPattern::Shipyard, 1)]);
        local
    }
    #[test]
    fn a_drag_starts_out_moving_every_unit_but_no_structure() {
        let local = placed();

        assert_eq!(
            Sending::present(&local.view(), asteroid(0)),
            1,
            "the constructor moves"
        );
        assert_eq!(
            Sending::present(&local.view(), asteroid(2)),
            0,
            "the shipyard never moves"
        );
    }

    #[test]
    fn a_send_takes_the_source_want_down_and_the_destination_up() {
        let local = placed();
        let sending = Sending {
            from: asteroid(0),
            to: asteroid(1),
            count: 1,
        };

        assert_eq!(
            sending.commands(&local.view()),
            vec![
                Command::Want {
                    asteroid: asteroid(0),
                    pattern: EntityPattern::Constructor,
                    count: 0,
                },
                Command::Want {
                    asteroid: asteroid(1),
                    pattern: EntityPattern::Constructor,
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn a_drag_of_more_than_the_source_holds_moves_what_it_holds() {
        let local = placed();
        let sending = Sending {
            from: asteroid(0),
            to: asteroid(1),
            count: 9,
        };

        assert_eq!(
            sending.patterns(&local.view()),
            vec![Moving {
                pattern: EntityPattern::Constructor,
                count: 1
            }]
        );
    }

    #[test]
    fn a_send_from_a_place_holding_nothing_issues_nothing() {
        let local = placed();
        let sending = Sending {
            from: asteroid(7),
            to: asteroid(1),
            count: 3,
        };

        assert!(sending.commands(&local.view()).is_empty());
    }

    #[test]
    fn a_unit_the_seat_no_longer_wants_there_can_still_be_sent() {
        let mut local = placed();
        local.want(&[(asteroid(0), EntityPattern::Constructor, 0)]);
        let sending = Sending {
            from: asteroid(0),
            to: asteroid(1),
            count: 1,
        };

        assert_eq!(
            sending.patterns(&local.view()),
            vec![Moving {
                pattern: EntityPattern::Constructor,
                count: 1
            }],
            "a surplus unit is still on the run, so a drag can move it"
        );
    }
}
