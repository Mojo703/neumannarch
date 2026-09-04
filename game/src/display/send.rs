use std::collections::BTreeMap;

use probe_sim::roster::{Kind, Roster};
use probe_sim::state::view::View;
use probe_sim::state::{Command, MAX_WANT};
use probe_sim::{Place, RowId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sending {
    pub from: Place,
    pub to: Place,
    pub count: u32,
}

impl Sending {
    pub fn present(view: &View, place: Place, roster: &Roster) -> u32 {
        run(view, place, roster).iter().map(|(_, held)| held).sum()
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
                        place: self.from,
                        row,
                        count: wanted(view, self.from, row).saturating_sub(count),
                    },
                    Command::Want {
                        place: self.to,
                        row,
                        count: (wanted(view, self.to, row) + count).min(MAX_WANT),
                    },
                ]
            })
            .collect()
    }
}

fn run(view: &View, place: Place, roster: &Roster) -> Vec<(RowId, u32)> {
    let mut held: BTreeMap<RowId, u32> = BTreeMap::new();
    for seen in view
        .seen
        .iter()
        .filter(|seen| seen.seat == view.seat && !seen.flying)
        .filter(|seen| seen.home == Some(place))
        .filter(|seen| roster[seen.row].kind() == Kind::Unit)
    {
        *held.entry(seen.row).or_insert(0) += 1;
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

fn wanted(view: &View, place: Place, row: RowId) -> u32 {
    view.compositions
        .iter()
        .filter(|composition| composition.place == place)
        .flat_map(|composition| &composition.rows)
        .find(|wanted| wanted.row == row)
        .map_or(0, |wanted| wanted.want)
}

#[cfg(test)]
mod tests {
    use probe_sim::roster::{CONSTRUCTOR, SHIPYARD};
    use probe_sim::{Band, RockId};

    use super::*;
    use crate::display::local::Local;

    fn inner(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Inner,
        }
    }

    fn placed() -> Local {
        let mut local = Local::start(1);
        local.want(&[(inner(0), SHIPYARD, 1), (inner(0), CONSTRUCTOR, 1)]);
        local
    }
    #[test]
    fn a_drag_starts_out_moving_every_unit_but_no_structure() {
        let local = placed();

        assert_eq!(
            Sending::present(&local.view(), inner(0), local.session().state().roster()),
            1,
            "the shipyard never moves"
        );
    }

    #[test]
    fn a_send_takes_the_source_want_down_and_the_destination_up() {
        let local = placed();
        let roster = local.session().state().roster();
        let sending = Sending {
            from: inner(0),
            to: inner(1),
            count: 1,
        };

        assert_eq!(
            sending.commands(&local.view(), roster),
            vec![
                Command::Want {
                    place: inner(0),
                    row: CONSTRUCTOR,
                    count: 0,
                },
                Command::Want {
                    place: inner(1),
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
            from: inner(0),
            to: inner(1),
            count: 9,
        };

        assert_eq!(sending.rows(&local.view(), roster), vec![(CONSTRUCTOR, 1)]);
    }

    #[test]
    fn a_send_from_a_place_holding_nothing_issues_nothing() {
        let local = placed();
        let roster = local.session().state().roster();
        let sending = Sending {
            from: inner(7),
            to: inner(1),
            count: 3,
        };

        assert!(sending.commands(&local.view(), roster).is_empty());
    }

    #[test]
    fn a_unit_the_seat_no_longer_wants_there_can_still_be_sent() {
        let mut local = placed();
        local.want(&[(inner(0), CONSTRUCTOR, 0)]);
        let sending = Sending {
            from: inner(0),
            to: inner(1),
            count: 1,
        };

        assert_eq!(
            sending.rows(&local.view(), local.session().state().roster()),
            vec![(CONSTRUCTOR, 1)],
            "a surplus unit is still on the run, so a drag can move it"
        );
    }
}
