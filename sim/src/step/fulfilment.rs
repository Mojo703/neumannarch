use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{AsteroidId, EntityId, RowId, SeatId};
use crate::materials::Materials;
use crate::posting::Posting;
use crate::roster::Kind;
use crate::state::{Route, Send, State};

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
    pub(crate) placements: Vec<Posting>,
    pub(crate) sends: Vec<Send>,
    pub(crate) openings: Vec<Posting>,
    pub(crate) cancellations: Vec<Cancellation>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ShortfallAssignment {
    pub(crate) from_reserve: Vec<Posting>,
    pub(crate) sent_units: BTreeMap<Route, Vec<EntityId>>,
    pub(crate) still_short: BTreeMap<Posting, u32>,
}

#[derive(Debug, Default)]
pub(crate) struct SendSchedules {
    departing: Vec<Send>,
    staying: BTreeSet<Route>,
}

impl SendSchedules {
    pub(crate) fn nothing_held_back() -> SendSchedules {
        SendSchedules::default()
    }
}

pub(crate) struct Fulfilment<'a> {
    state: &'a State,
    posted: BTreeMap<Posting, u32>,
    frames: BTreeMap<Posting, Vec<usize>>,
    surplus: BTreeMap<SeatId, BTreeMap<RowId, Vec<Surplus>>>,
}

#[derive(Clone, Copy, Debug)]
struct Surplus {
    entity: EntityId,
    asteroid: AsteroidId,
}

impl<'a> Fulfilment<'a> {
    pub(crate) fn of(state: &'a State) -> Fulfilment<'a> {
        let posted = posted(state);
        let frames = framed(state);
        let mut surplus: BTreeMap<SeatId, BTreeMap<RowId, Vec<Surplus>>> = BTreeMap::new();
        for (posting, over) in surpluses(state, &posted, &frames) {
            if state[posting.row()].kind() == Kind::Unit {
                surplus
                    .entry(posting.seat())
                    .or_default()
                    .entry(posting.row())
                    .or_default()
                    .extend(over.into_iter().map(|entity| Surplus {
                        entity,
                        asteroid: posting.asteroid(),
                    }));
            }
        }
        Fulfilment {
            state,
            posted,
            frames,
            surplus,
        }
    }

    fn count(&self, posting: Posting) -> u32 {
        self.posted.get(&posting).copied().unwrap_or_default()
    }

    pub(crate) fn run(mut self) -> Assigned {
        let assignment = self.assign();
        let schedules = self.schedule(&assignment);
        self.settle(&assignment, schedules)
    }

    pub(crate) fn assign(&mut self) -> ShortfallAssignment {
        let mut assignment = ShortfallAssignment::default();
        let mut reserved: BTreeMap<SeatId, BTreeMap<RowId, u32>> = BTreeMap::new();
        for (posting, shortfall) in shortfalls(self.state, &self.posted) {
            let from_reserve = self.take_reserved(posting, shortfall, &mut reserved);
            for _ in 0..from_reserve {
                assignment.from_reserve.push(posting);
            }
            let mut taking = 0;
            for surplus in self.nearest_surplus(posting, shortfall - from_reserve) {
                assignment
                    .sent_units
                    .entry(Route {
                        source: surplus.asteroid,
                        destination: posting.asteroid(),
                        seat: posting.seat(),
                    })
                    .or_default()
                    .push(surplus.entity);
                taking += 1;
            }
            assignment
                .still_short
                .insert(posting, shortfall - from_reserve - taking);
        }
        assignment
    }

    pub(crate) fn schedule(&self, assignment: &ShortfallAssignment) -> SendSchedules {
        let mut schedules = SendSchedules::default();
        for (route, members) in &assignment.sent_units {
            let mut members = members.clone();
            members.sort_unstable();
            match Send::joining(self.state, *route, &members) {
                Some(send) => schedules.departing.push(send),
                None => {
                    schedules.staying.insert(*route);
                }
            }
        }
        schedules
    }

    pub(crate) fn settle(
        &self,
        assignment: &ShortfallAssignment,
        schedules: SendSchedules,
    ) -> Assigned {
        let held_back = self.held_back(assignment, &schedules.staying);
        let leaving = self.leaving(assignment, &schedules.staying);
        let mut assigned = Assigned {
            placements: assignment.from_reserve.clone(),
            sends: schedules.departing,
            openings: Vec::new(),
            cancellations: Vec::new(),
        };
        for (posting, still) in &assignment.still_short {
            let back = held_back.get(posting).copied().unwrap_or_default();
            self.reconcile(&mut assigned, *posting, still + back);
        }
        for (posting, over) in self.unwanted_frames(&leaving) {
            self.cancel(&mut assigned, posting, over);
        }
        assigned
    }

    fn take_reserved(
        &self,
        posting: Posting,
        shortfall: u32,
        taken: &mut BTreeMap<SeatId, BTreeMap<RowId, u32>>,
    ) -> u32 {
        let held = self.state[posting.seat()].reserved(posting.row());
        let spent = taken
            .entry(posting.seat())
            .or_default()
            .entry(posting.row())
            .or_default();
        let giving = shortfall.min(held.saturating_sub(*spent));
        *spent += giving;
        giving
    }

    fn nearest_surplus(&mut self, posting: Posting, asked: u32) -> Vec<Surplus> {
        let mut taking = Vec::new();
        let Some(surplus) = self
            .surplus
            .get_mut(&posting.seat())
            .and_then(|rows| rows.get_mut(&posting.row()))
        else {
            return taking;
        };
        let here = self.state.asteroid_body(posting.asteroid()).pos;
        while taking.len() < asked as usize {
            let nearest = surplus
                .iter()
                .enumerate()
                .filter(|(_, surplus)| surplus.asteroid != posting.asteroid())
                .min_by(|(_, a), (_, b)| {
                    let (first, second) = (
                        self.state.asteroid_body(a.asteroid).pos.distance(here),
                        self.state.asteroid_body(b.asteroid).pos.distance(here),
                    );
                    first
                        .total_cmp(&second)
                        .then(a.asteroid.cmp(&b.asteroid))
                        .then(b.entity.cmp(&a.entity))
                })
                .map(|(at, _)| at);
            match nearest {
                Some(at) => taking.push(surplus.remove(at)),
                None => break,
            }
        }
        taking
    }

    fn reconcile(&self, assigned: &mut Assigned, posting: Posting, asked: u32) {
        let open = self.open_frames(posting).len();
        match asked {
            0 => self.cancel(assigned, posting, open as u32),
            _ if open == 0 => assigned.openings.push(posting),
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
        let mut open: Vec<Cancellation> = self
            .frames
            .get(&posting)
            .into_iter()
            .flatten()
            .map(|at| Cancellation {
                posting,
                frame: *at,
                progress: self.state.frames()[*at].progress(),
            })
            .collect();
        open.sort_by(|first, second| {
            first
                .progress
                .total_cmp(&second.progress)
                .then(first.frame.cmp(&second.frame))
        });
        open
    }

    fn held_back(
        &self,
        assignment: &ShortfallAssignment,
        staying: &BTreeSet<Route>,
    ) -> BTreeMap<Posting, u32> {
        let mut back: BTreeMap<Posting, u32> = BTreeMap::new();
        for (route, members) in &assignment.sent_units {
            if staying.contains(route) {
                self.tally(route.destination, route.seat, members, &mut back);
            }
        }
        back
    }

    fn leaving(
        &self,
        assignment: &ShortfallAssignment,
        staying: &BTreeSet<Route>,
    ) -> BTreeMap<Posting, u32> {
        let mut gone: BTreeMap<Posting, u32> = BTreeMap::new();
        for (route, members) in &assignment.sent_units {
            if !staying.contains(route) {
                self.tally(route.source, route.seat, members, &mut gone);
            }
        }
        gone
    }

    fn tally(
        &self,
        asteroid: AsteroidId,
        seat: SeatId,
        members: &[EntityId],
        into: &mut BTreeMap<Posting, u32>,
    ) {
        for entity in members.iter().map(|id| self.state.entity(*id)) {
            *into
                .entry(Posting::of(asteroid, seat, entity.row()))
                .or_default() += 1;
        }
    }
}

fn shortfalls(state: &State, posted: &BTreeMap<Posting, u32>) -> BTreeMap<Posting, u32> {
    state
        .posts()
        .flat_map(|(post, wants)| {
            wants.iter().filter_map(move |(row, want)| {
                let posting = Posting::new(post, row);
                want.checked_sub(posted.get(&posting).copied().unwrap_or_default())
                    .filter(|missing| *missing > 0)
                    .map(|missing| (posting, missing))
            })
        })
        .collect()
}

fn posted(state: &State) -> BTreeMap<Posting, u32> {
    let mut posted: BTreeMap<Posting, u32> = BTreeMap::new();
    for entity in state.entities() {
        *posted
            .entry(Posting::of(entity.home(), entity.seat(), entity.row()))
            .or_default() += 1;
    }
    posted
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
    posted: &BTreeMap<Posting, u32>,
    frames: &BTreeMap<Posting, Vec<usize>>,
) -> BTreeMap<Posting, Vec<EntityId>> {
    let mut standing: BTreeMap<Posting, Vec<EntityId>> = BTreeMap::new();
    for entity in state.entities().filter(|entity| entity.flight().is_none()) {
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
        let over = posted
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
