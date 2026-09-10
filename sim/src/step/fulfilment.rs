use std::collections::BTreeMap;

use crate::ids::{AsteroidId, EntityId, RowId, SeatId};
use crate::materials::Materials;
use crate::posting::Posting;
use crate::roster::Kind;
use crate::state::{Frame, Rolls, State};
use crate::step::reserve::Placements;
use crate::time::{RunningSpan, Time};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cancellation {
    pub(crate) posting: Posting,
    pub(crate) frame: usize,
    pub(crate) progress: f64,
}

impl Cancellation {
    pub(crate) fn refund(&self, state: &State) -> Materials {
        let cost = state[self.posting.row()].cost;
        let total = cost.total();
        match total > 0.0 {
            true => cost * (self.progress / total),
            false => Materials::ZERO,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Assigned {
    pub(crate) placements: Placements,
    pub(crate) sent_to: BTreeMap<EntityId, AsteroidId>,
    pub(crate) still_short: BTreeMap<Posting, u32>,
    pub(crate) openings: Vec<Frame>,
    pub(crate) cancellations: Vec<Cancellation>,
}

impl Assigned {
    fn sent_away(&self, state: &State) -> BTreeMap<Posting, u32> {
        let mut gone: BTreeMap<Posting, u32> = BTreeMap::new();
        for entity in self.sent_to.keys().map(|id| state.entity(*id)) {
            *gone
                .entry(Posting::of(entity.home(), entity.seat(), entity.row()))
                .or_default() += 1;
        }
        gone
    }
}

pub(crate) struct Fulfilment<'a> {
    state: &'a State,
    rolls: &'a Rolls<'a>,
    shortfalls: &'a BTreeMap<Posting, u32>,
    filled: &'a Placements,
    opened: Time,
    homed: BTreeMap<Posting, u32>,
    frames: BTreeMap<Posting, Vec<usize>>,
    surplus: BTreeMap<SeatId, BTreeMap<RowId, BTreeMap<AsteroidId, Vec<EntityId>>>>,
}

impl<'a> Fulfilment<'a> {
    pub(crate) fn of(
        state: &'a State,
        rolls: &'a Rolls<'a>,
        shortfalls: &'a BTreeMap<Posting, u32>,
        filled: &'a Placements,
        over: RunningSpan,
    ) -> Fulfilment<'a> {
        let homed = state.homed();
        let frames = framed(state);
        let mut surplus: BTreeMap<SeatId, BTreeMap<RowId, BTreeMap<AsteroidId, Vec<EntityId>>>> =
            BTreeMap::new();
        for (posting, mut over) in surpluses(state, &homed, &frames) {
            if state[posting.row()].kind() == Kind::Unit {
                over.reverse();
                surplus
                    .entry(posting.seat())
                    .or_default()
                    .entry(posting.row())
                    .or_default()
                    .insert(posting.asteroid(), over);
            }
        }
        Fulfilment {
            state,
            rolls,
            shortfalls,
            filled,
            opened: over.ends_at(state.time()),
            homed,
            frames,
            surplus,
        }
    }

    pub(crate) fn run(mut self) -> Assigned {
        let mut assigned = Assigned::default();
        let mut still_short: BTreeMap<Posting, u32> = BTreeMap::new();
        for (posting, shortfall) in self.shortfalls {
            let short = shortfall - self.filled.filling(*posting);
            let sent = self.nearest_surplus(*posting, short);
            for entity in &sent {
                assigned.sent_to.insert(*entity, posting.asteroid());
            }
            still_short.insert(*posting, short - sent.len() as u32);
        }
        for (posting, short) in &still_short {
            self.reconcile(&mut assigned, *posting, *short);
        }
        for (posting, over) in self.unwanted_frames(&assigned.sent_away(self.state)) {
            self.cancel(&mut assigned, posting, over);
        }
        Assigned {
            still_short,
            ..assigned
        }
    }

    fn count(&self, posting: Posting) -> u32 {
        self.homed.get(&posting).copied().unwrap_or_default()
    }

    fn nearest_surplus(&mut self, posting: Posting, asked: u32) -> Vec<EntityId> {
        let mut taking = Vec::new();
        let rolls = self.rolls;
        let Some(surplus) = self
            .surplus
            .get_mut(&posting.seat())
            .and_then(|rows| rows.get_mut(&posting.row()))
            .filter(|_| asked > 0)
        else {
            return taking;
        };
        let here = rolls[posting.asteroid()].body().pos;
        let mut nearest: Vec<(f64, AsteroidId, &mut Vec<EntityId>)> = surplus
            .iter_mut()
            .filter(|(asteroid, _)| **asteroid != posting.asteroid())
            .map(|(asteroid, held)| (rolls[*asteroid].body().pos.distance(here), *asteroid, held))
            .collect();
        nearest.sort_unstable_by(|(one, first, _), (other, second, _)| {
            one.total_cmp(other).then(first.cmp(second))
        });
        for (_, _, held) in &mut nearest {
            while taking.len() < asked as usize {
                match held.pop() {
                    Some(entity) => taking.push(entity),
                    None => break,
                }
            }
            if taking.len() == asked as usize {
                break;
            }
        }
        surplus.retain(|_, held| !held.is_empty());
        taking
    }

    fn reconcile(&self, assigned: &mut Assigned, posting: Posting, asked: u32) {
        let open = self.open_frames(posting).len();
        match asked {
            0 => self.cancel(assigned, posting, open as u32),
            _ if open == 0 => {
                assigned
                    .openings
                    .push(Frame::new(posting.post(), posting.row(), 0.0, self.opened))
            }
            _ => {}
        }
    }

    fn cancel(&self, assigned: &mut Assigned, posting: Posting, count: u32) {
        assigned
            .cancellations
            .extend(self.open_frames(posting).into_iter().take(count as usize));
    }

    fn unwanted_frames(&self, leaving: &BTreeMap<Posting, u32>) -> BTreeMap<Posting, u32> {
        self.frames
            .iter()
            .filter(|(posting, _)| {
                let want = self
                    .state
                    .wants(posting.post())
                    .map_or(0, |wants| wants.get(posting.row()));
                let gone = leaving.get(*posting).copied().unwrap_or_default();
                want + gone <= self.count(**posting)
            })
            .map(|(posting, open)| (*posting, open.len() as u32))
            .collect()
    }

    fn open_frames(&self, posting: Posting) -> Vec<Cancellation> {
        self.frames
            .get(&posting)
            .into_iter()
            .flatten()
            .map(|at| Cancellation {
                posting,
                frame: *at,
                progress: self.state.frames()[*at].progress(),
            })
            .collect()
    }
}

fn framed(state: &State) -> BTreeMap<Posting, Vec<usize>> {
    let mut framed: BTreeMap<Posting, Vec<usize>> = BTreeMap::new();
    for (at, frame) in state.frames().iter().enumerate() {
        framed
            .entry(Posting::new(frame.post(), frame.row()))
            .or_default()
            .push(at);
    }
    framed
}

fn surpluses(
    state: &State,
    homed: &BTreeMap<Posting, u32>,
    frames: &BTreeMap<Posting, Vec<usize>>,
) -> BTreeMap<Posting, Vec<EntityId>> {
    let mut standing: BTreeMap<Posting, Vec<EntityId>> = BTreeMap::new();
    for entity in state
        .entities()
        .filter(|entity| entity.standing().is_some())
    {
        standing
            .entry(Posting::of(entity.home(), entity.seat(), entity.row()))
            .or_default()
            .push(entity.id());
    }
    standing.retain(|posting, held| {
        let open = frames.get(posting).map_or(0, Vec::len) as u32;
        let covered = state
            .wants(posting.post())
            .map_or(0, |wants| wants.get(posting.row()))
            .saturating_sub(open);
        let over = homed
            .get(posting)
            .copied()
            .unwrap_or_default()
            .saturating_sub(covered) as usize;
        held.sort_unstable_by(|first, second| second.cmp(first));
        held.truncate(over);
        !held.is_empty()
    });
    standing
}
