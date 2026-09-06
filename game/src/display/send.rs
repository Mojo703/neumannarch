use std::collections::BTreeMap;

use neumannarch_sim::roster::{Kind, Roster};
use neumannarch_sim::state::view::View;
use neumannarch_sim::state::{Command, MAX_WANT};
use neumannarch_sim::{AsteroidId, RowId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sending {
    pub from: AsteroidId,
    pub to: AsteroidId,
    pub count: u32,
}

impl Sending {
    pub fn present(view: &View, asteroid: AsteroidId, roster: &Roster) -> u32 {
        run(view, asteroid, roster)
            .iter()
            .map(|(_, held)| held)
            .sum()
    }

    pub fn rows(&self, view: &View, roster: &Roster) -> Vec<(RowId, u32)> {
        let mut left = self.count;
        let mut moving: Vec<(RowId, u32)> = Vec::new();
        for (row, held) in run(view, self.from, roster).into_iter().rev() {
            if left == 0 {
                break;
            }
            let taken = held.min(left);
            left -= taken;
            moving.push((row, taken));
        }
        moving.reverse();
        moving
    }

    pub fn commands(&self, view: &View, roster: &Roster) -> Vec<Command> {
        self.rows(view, roster)
            .into_iter()
            .flat_map(|(row, count)| {
                [
                    Command::Want {
                        asteroid: self.from,
                        row,
                        count: wanted(view, self.from, row).saturating_sub(count),
                    },
                    Command::Want {
                        asteroid: self.to,
                        row,
                        count: (wanted(view, self.to, row) + count).min(MAX_WANT),
                    },
                ]
            })
            .collect()
    }
}

fn run(view: &View, asteroid: AsteroidId, roster: &Roster) -> Vec<(RowId, u32)> {
    let mut held: BTreeMap<RowId, u32> = BTreeMap::new();
    for unit in view
        .present
        .iter()
        .filter(|unit| unit.seat == view.seat && unit.at.standing() == Some(asteroid))
        .filter(|unit| unit.home == asteroid)
        .filter(|unit| roster[unit.row].kind() == Kind::Unit)
    {
        *held.entry(unit.row).or_insert(0) += 1;
    }
    let mut rows: Vec<(RowId, u32)> = held.into_iter().collect();
    rows.sort_by(|(a, _), (b, _)| {
        roster[*b]
            .cost
            .total()
            .total_cmp(&roster[*a].cost.total())
            .then(a.cmp(b))
    });
    rows
}

fn wanted(view: &View, asteroid: AsteroidId, row: RowId) -> u32 {
    view.plan_of(asteroid, row).map_or(0, |plan| plan.want)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::local::Local;
    use neumannarch_sim::roster::{CONSTRUCTOR, SHIPYARD};

    fn asteroid(at: u32) -> AsteroidId {
        AsteroidId(at)
    }

    fn placed() -> Local {
        let mut local = Local::start(1);
        local.want(&[(asteroid(0), CONSTRUCTOR, 1)]);
        local.want(&[(asteroid(2), SHIPYARD, 1)]);
        local
    }
    #[test]
    fn a_drag_starts_out_moving_every_unit_but_no_structure() {
        let local = placed();
        let roster = local.session().state().roster();

        assert_eq!(
            Sending::present(&local.view(), asteroid(0), roster),
            1,
            "the constructor moves"
        );
        assert_eq!(
            Sending::present(&local.view(), asteroid(2), roster),
            0,
            "the shipyard never moves"
        );
    }

    #[test]
    fn a_send_takes_the_source_want_down_and_the_destination_up() {
        let local = placed();
        let roster = local.session().state().roster();
        let sending = Sending {
            from: asteroid(0),
            to: asteroid(1),
            count: 1,
        };

        assert_eq!(
            sending.commands(&local.view(), roster),
            vec![
                Command::Want {
                    asteroid: asteroid(0),
                    row: CONSTRUCTOR,
                    count: 0,
                },
                Command::Want {
                    asteroid: asteroid(1),
                    row: CONSTRUCTOR,
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn a_drag_of_more_than_the_source_holds_moves_what_it_holds() {
        let local = placed();
        let roster = local.session().state().roster();
        let sending = Sending {
            from: asteroid(0),
            to: asteroid(1),
            count: 9,
        };

        assert_eq!(sending.rows(&local.view(), roster), vec![(CONSTRUCTOR, 1)]);
    }

    #[test]
    fn a_send_from_a_place_holding_nothing_issues_nothing() {
        let local = placed();
        let roster = local.session().state().roster();
        let sending = Sending {
            from: asteroid(7),
            to: asteroid(1),
            count: 3,
        };

        assert!(sending.commands(&local.view(), roster).is_empty());
    }

    #[test]
    fn a_unit_the_seat_no_longer_wants_there_can_still_be_sent() {
        let mut local = placed();
        local.want(&[(asteroid(0), CONSTRUCTOR, 0)]);
        let sending = Sending {
            from: asteroid(0),
            to: asteroid(1),
            count: 1,
        };

        assert_eq!(
            sending.rows(&local.view(), local.session().state().roster()),
            vec![(CONSTRUCTOR, 1)],
            "a surplus unit is still on the run, so a drag can move it"
        );
    }
}
