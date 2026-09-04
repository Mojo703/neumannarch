use std::collections::{BTreeMap, BTreeSet};

use crate::TICKS_PER_SECOND;
use crate::ids::{EntityId, RowId, SeatId};
use crate::place::{Place, Post};
use crate::roster::Kind;
use crate::state::{Entity, Schedule, State};
use crate::time::Tick;

const SEARCH_STEP: u64 = TICKS_PER_SECOND as u64;

const SEARCH_BOUND: u64 = 600 * TICKS_PER_SECOND as u64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub post: Post,
    pub row: RowId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Send {
    pub source: Place,
    pub destination: Place,
    pub schedules: BTreeMap<RowId, Schedule>,
    pub members: Vec<EntityId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opening {
    pub post: Post,
    pub row: RowId,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cancellation {
    pub frame: usize,
    pub row: RowId,
    pub seat: SeatId,
    pub progress: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assigned {
    pub placements: Vec<Placement>,
    pub sends: Vec<Send>,
    pub openings: Vec<Opening>,
    pub cancellations: Vec<Cancellation>,
    pub marked: Vec<EntityId>,
}

impl Send {
    pub(crate) fn solved(
        state: &State,
        source: Place,
        destination: Place,
        members: &[EntityId],
    ) -> Option<Send> {
        let gravity = state.gravity();
        let depart = state.tick().next();
        let from = state.anchor(source).at(depart, gravity);
        let rows: BTreeSet<RowId> = members
            .iter()
            .filter_map(|id| state.entity(*id))
            .map(Entity::row)
            .collect();
        (SEARCH_STEP..=SEARCH_BOUND)
            .step_by(SEARCH_STEP as usize)
            .find_map(|step| {
                let arrive = Tick(depart.0 + step);
                let to = state.anchor(destination).at(arrive, gravity);
                let schedules: BTreeMap<RowId, Schedule> = rows
                    .iter()
                    .filter_map(|row| {
                        Schedule::between(from, to, depart, arrive, state[*row].accel.0, gravity)
                            .map(|schedule| (*row, schedule))
                    })
                    .collect();
                (schedules.len() == rows.len()).then(|| Send {
                    source,
                    destination,
                    schedules,
                    members: members.to_vec(),
                })
            })
    }
}

pub struct Fulfilment<'a> {
    state: &'a State,
    spare: BTreeMap<(SeatId, RowId), Vec<Spare>>,
    stuck: Vec<EntityId>,
}

#[derive(Clone, Copy, Debug)]
struct Spare {
    entity: EntityId,
    place: Place,
}

impl<'a> Fulfilment<'a> {
    pub fn of(state: &'a State) -> Fulfilment<'a> {
        let mut spare: BTreeMap<(SeatId, RowId), Vec<Spare>> = BTreeMap::new();
        let mut stuck = Vec::new();
        for (post, row, over) in surpluses(state) {
            if state[row].kind() == Kind::Structure {
                stuck.extend(over);
            } else {
                spare
                    .entry((post.seat, row))
                    .or_default()
                    .extend(over.into_iter().map(|entity| Spare {
                        entity,
                        place: post.place,
                    }));
            }
        }
        Fulfilment {
            state,
            spare,
            stuck,
        }
    }

    pub fn run(mut self) -> Assigned {
        let mut assigned = Assigned::default();
        let mut reserved: BTreeMap<(SeatId, RowId), u32> = BTreeMap::new();
        let mut moving: BTreeMap<(Place, Place, SeatId), Vec<EntityId>> = BTreeMap::new();
        let mut missing: Vec<(Post, RowId, u32)> = Vec::new();
        for (post, row, shortfall) in shortfalls(self.state) {
            let from_reserve = self.take_reserved(post, row, shortfall, &mut reserved);
            for _ in 0..from_reserve {
                assigned.placements.push(Placement { post, row });
            }
            let mut taking = 0;
            for spare in self.nearest_spare(post, row, shortfall - from_reserve) {
                moving
                    .entry((spare.place, post.place, post.seat))
                    .or_default()
                    .push(spare.entity);
                taking += 1;
            }
            missing.push((post, row, shortfall - from_reserve - taking));
        }
        let held_back = self.solve(&mut assigned, moving);
        for (post, row, still) in missing {
            let back = held_back.get(&(post, row)).copied().unwrap_or_default();
            self.reconcile(&mut assigned, post, row, still + back);
        }
        self.mark(&mut assigned);
        for (post, row, over) in unwanted_frames(self.state) {
            self.cancel(&mut assigned, post, row, over);
        }
        assigned
    }

    fn take_reserved(
        &self,
        post: Post,
        row: RowId,
        shortfall: u32,
        taken: &mut BTreeMap<(SeatId, RowId), u32>,
    ) -> u32 {
        let held = self.state[post.seat].reserved(row);
        let spent = taken.entry((post.seat, row)).or_default();
        let giving = shortfall.min(held.saturating_sub(*spent));
        *spent += giving;
        giving
    }

    fn nearest_spare(&mut self, post: Post, row: RowId, wanted: u32) -> Vec<Spare> {
        let mut taking = Vec::new();
        let Some(spare) = self.spare.get_mut(&(post.seat, row)) else {
            return taking;
        };
        let here = self.state.rock_body(post.place.rock).pos;
        while taking.len() < wanted as usize {
            let nearest = spare
                .iter()
                .enumerate()
                .filter(|(_, spare)| spare.place != post.place)
                .min_by(|(_, a), (_, b)| {
                    let (first, second) = (
                        self.state.rock_body(a.place.rock).pos.distance(here),
                        self.state.rock_body(b.place.rock).pos.distance(here),
                    );
                    first
                        .total_cmp(&second)
                        .then(a.place.rock.cmp(&b.place.rock))
                        .then(b.entity.cmp(&a.entity))
                })
                .map(|(at, _)| at);
            match nearest {
                Some(at) => taking.push(spare.remove(at)),
                None => break,
            }
        }
        taking
    }

    fn reconcile(&self, assigned: &mut Assigned, post: Post, row: RowId, wanted: u32) {
        let open = self.frames_of(post, row);
        if open.len() < wanted as usize {
            assigned.openings.push(Opening {
                post,
                row,
                count: wanted - open.len() as u32,
            });
        } else {
            self.cancel(assigned, post, row, open.len() as u32 - wanted);
        }
    }

    fn cancel(&self, assigned: &mut Assigned, post: Post, row: RowId, count: u32) {
        let mut open = self.frames_of(post, row);
        open.sort_by(|a, b| {
            self.state.frames()[*a]
                .progress()
                .total_cmp(&self.state.frames()[*b].progress())
                .then(a.cmp(b))
        });
        for frame in open.into_iter().take(count as usize) {
            assigned.cancellations.push(Cancellation {
                frame,
                row,
                seat: post.seat,
                progress: self.state.frames()[frame].progress(),
            });
        }
    }

    fn frames_of(&self, post: Post, row: RowId) -> Vec<usize> {
        self.state
            .frames()
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame.post() == post && frame.row() == row)
            .map(|(at, _)| at)
            .collect()
    }

    fn solve(
        &self,
        assigned: &mut Assigned,
        moving: BTreeMap<(Place, Place, SeatId), Vec<EntityId>>,
    ) -> BTreeMap<(Post, RowId), u32> {
        let mut held_back: BTreeMap<(Post, RowId), u32> = BTreeMap::new();
        for ((from, to, seat), mut members) in moving {
            members.sort_unstable();
            match Send::solved(self.state, from, to, &members) {
                Some(send) => assigned.sends.push(send),
                None => {
                    for row in members.iter().filter_map(|id| self.state.entity(*id)) {
                        *held_back
                            .entry((Post { place: to, seat }, row.row()))
                            .or_default() += 1;
                    }
                }
            }
        }
        held_back
    }

    fn mark(&self, assigned: &mut Assigned) {
        assigned.marked = self
            .spare
            .values()
            .flatten()
            .map(|spare| spare.entity)
            .chain(self.stuck.iter().copied())
            .collect();
        assigned.marked.sort_unstable();
    }
}

fn shortfalls(state: &State) -> Vec<(Post, RowId, u32)> {
    state
        .posts()
        .flat_map(|(post, wants)| {
            wants.iter().filter_map(move |(row, want)| {
                want.checked_sub(state.count(post, row))
                    .filter(|missing| *missing > 0)
                    .map(|missing| (post, row, missing))
            })
        })
        .collect()
}

fn surpluses(state: &State) -> Vec<(Post, RowId, Vec<EntityId>)> {
    let mut over: Vec<(Post, RowId, Vec<EntityId>)> = Vec::new();
    for (post, row) in held_rows(state) {
        let want = state.wants(post).map_or(0, |wants| wants.get(row));
        let mut held: Vec<EntityId> = state
            .entities_at(post.place)
            .filter(|entity| entity.seat() == post.seat && entity.row() == row)
            .filter(|entity| !entity.is_flying())
            .map(Entity::id)
            .collect();
        held.sort_unstable_by(|a, b| b.cmp(a));
        let spare = state.count(post, row).saturating_sub(want) as usize;
        held.truncate(spare);
        if !held.is_empty() {
            over.push((post, row, held));
        }
    }
    over
}

fn held_rows(state: &State) -> Vec<(Post, RowId)> {
    let mut rows: Vec<(Post, RowId)> = state
        .entities()
        .map(|entity| {
            (
                Post {
                    place: entity.home(),
                    seat: entity.seat(),
                },
                entity.row(),
            )
        })
        .collect();
    rows.sort_unstable();
    rows.dedup();
    rows
}

fn unwanted_frames(state: &State) -> Vec<(Post, RowId, u32)> {
    let mut open: BTreeMap<(Post, RowId), u32> = BTreeMap::new();
    for frame in state.frames() {
        *open.entry((frame.post(), frame.row())).or_default() += 1;
    }
    open.into_iter()
        .filter(|((post, row), _)| {
            let want = state.wants(*post).map_or(0, |wants| wants.get(*row));
            want <= state.count(*post, *row)
        })
        .map(|((post, row), count)| (post, row, count))
        .collect()
}
