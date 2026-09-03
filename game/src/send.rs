//! The send: a drag from one ring to another, and the count edits it
//! issues.

use std::collections::BTreeMap;

use probe_sim::roster::{Kind, Roster};
use probe_sim::state::view::View;
use probe_sim::state::{Command, MAX_WANT};
use probe_sim::{Place, RowId};

/// A drag from one ring to another: the units it moves, and the edits that
/// move them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sending {
    pub from: Place,
    pub to: Place,
    /// How many units it moves, off the end of the source run. More than
    /// the source holds moves what it holds.
    pub count: u32,
}

impl Sending {
    /// Every unit of the seat's own at `place`, which is what a drag starts
    /// out moving.
    pub fn present(view: &View, place: Place, roster: &Roster) -> u32 {
        run(view, place, roster).iter().map(|(_, held)| held).sum()
    }

    /// The rows it moves and how many of each, taken off the end of the
    /// source run, so the cheapest rows go first.
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

    /// The two count edits per row it moves: the source's want down by what
    /// leaves, the destination's up by what arrives.
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

/// The units of the seat's own holding at `place`, by row, in run order:
/// rows by cost descending. It counts what the seat sees, which is what the
/// run draws, so a unit the seat no longer wants there can still be sent.
/// Structures never move, so they are not in it.
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

/// What the seat wants of `row` at `place` now; none where it wants
/// nothing.
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
    use probe_sim::belt::Belt;
    use probe_sim::roster::{CONSTRUCTOR, SHIPYARD};
    use probe_sim::session::Session;
    use probe_sim::state::{Issued, State};
    use probe_sim::{Band, RockId, SeatId, TICKS_PER_SECOND, TeamId, Tick};

    use super::*;

    const PLAYER: SeatId = SeatId(0);

    fn inner(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Inner,
        }
    }

    /// A match where the player has placed its reserve at rock zero: a
    /// shipyard, which never moves, and a constructor, which does.
    fn placed() -> Session {
        let mut session = Session::new(State::start(
            Tick(15 * 60 * TICKS_PER_SECOND as u64),
            Belt::GRAVITY,
            Belt::fixed(Belt::GRAVITY),
            &[TeamId(0)],
        ));
        let wants = [(SHIPYARD, 1), (CONSTRUCTOR, 1)].map(|(row, count)| Issued {
            seat: PLAYER,
            command: Command::Want {
                place: inner(0),
                row,
                count,
            },
        });
        assert!(session.advance(wants.into()).is_empty());
        session
    }

    fn view(session: &Session) -> View {
        View::of(session.state(), PLAYER, session.shots())
    }

    #[test]
    fn a_drag_starts_out_moving_every_unit_but_no_structure() {
        let session = placed();

        assert_eq!(
            Sending::present(&view(&session), inner(0), session.state().roster()),
            1,
            "the shipyard never moves"
        );
    }

    #[test]
    fn a_send_takes_the_source_want_down_and_the_destination_up() {
        let session = placed();
        let roster = session.state().roster();
        let sending = Sending {
            from: inner(0),
            to: inner(1),
            count: 1,
        };

        assert_eq!(
            sending.commands(&view(&session), roster),
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
        let session = placed();
        let roster = session.state().roster();
        let sending = Sending {
            from: inner(0),
            to: inner(1),
            count: 9,
        };

        assert_eq!(
            sending.rows(&view(&session), roster),
            vec![(CONSTRUCTOR, 1)]
        );
    }

    #[test]
    fn a_send_from_a_place_holding_nothing_issues_nothing() {
        let session = placed();
        let roster = session.state().roster();
        let sending = Sending {
            from: inner(7),
            to: inner(1),
            count: 3,
        };

        assert!(sending.commands(&view(&session), roster).is_empty());
    }

    #[test]
    fn a_unit_the_seat_no_longer_wants_there_can_still_be_sent() {
        let mut session = placed();
        let dropped = Issued {
            seat: PLAYER,
            command: Command::Want {
                place: inner(0),
                row: CONSTRUCTOR,
                count: 0,
            },
        };
        assert!(session.advance(vec![dropped]).is_empty());
        let sending = Sending {
            from: inner(0),
            to: inner(1),
            count: 1,
        };

        assert_eq!(
            sending.rows(&view(&session), session.state().roster()),
            vec![(CONSTRUCTOR, 1)],
            "a surplus unit is still on the run, so a drag can move it"
        );
    }
}
