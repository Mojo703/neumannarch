use std::collections::BTreeMap;

use crate::ids::{EntityId, RockId, RowId, SeatId};
use crate::post::Post;
use crate::roster::Kind;
use crate::state::{Entity, Send, State};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub post: Post,
    pub row: RowId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opening {
    pub post: Post,
    pub row: RowId,
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
}

pub struct Fulfilment<'a> {
    state: &'a State,
    surplus: BTreeMap<(SeatId, RowId), Vec<Surplus>>,
}

#[derive(Clone, Copy, Debug)]
struct Surplus {
    entity: EntityId,
    rock: RockId,
}

impl<'a> Fulfilment<'a> {
    pub fn of(state: &'a State) -> Fulfilment<'a> {
        let mut surplus: BTreeMap<(SeatId, RowId), Vec<Surplus>> = BTreeMap::new();
        for (post, row, over) in surpluses(state) {
            if state[row].kind() == Kind::Unit {
                surplus
                    .entry((post.seat, row))
                    .or_default()
                    .extend(over.into_iter().map(|entity| Surplus {
                        entity,
                        rock: post.rock,
                    }));
            }
        }
        Fulfilment { state, surplus }
    }

    pub fn run(mut self) -> Assigned {
        let mut assigned = Assigned::default();
        let mut reserved: BTreeMap<(SeatId, RowId), u32> = BTreeMap::new();
        let mut moving: BTreeMap<(RockId, RockId, SeatId), Vec<EntityId>> = BTreeMap::new();
        let mut missing: Vec<(Post, RowId, u32)> = Vec::new();
        for (post, row, shortfall) in shortfalls(self.state) {
            let from_reserve = self.take_reserved(post, row, shortfall, &mut reserved);
            for _ in 0..from_reserve {
                assigned.placements.push(Placement { post, row });
            }
            let mut taking = 0;
            for surplus in self.nearest_surplus(post, row, shortfall - from_reserve) {
                moving
                    .entry((surplus.rock, post.rock, post.seat))
                    .or_default()
                    .push(surplus.entity);
                taking += 1;
            }
            missing.push((post, row, shortfall - from_reserve - taking));
        }
        let held_back = self.solve(&mut assigned, moving);
        for (post, row, still) in missing {
            let back = held_back.get(&(post, row)).copied().unwrap_or_default();
            self.reconcile(&mut assigned, post, row, still + back);
        }
        let leaving = self.leaving(&assigned.sends);
        for (post, row, over) in unwanted_frames(self.state, &leaving) {
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

    fn nearest_surplus(&mut self, post: Post, row: RowId, wanted: u32) -> Vec<Surplus> {
        let mut taking = Vec::new();
        let Some(surplus) = self.surplus.get_mut(&(post.seat, row)) else {
            return taking;
        };
        let here = self.state.rock_body(post.rock).pos;
        while taking.len() < wanted as usize {
            let nearest = surplus
                .iter()
                .enumerate()
                .filter(|(_, surplus)| surplus.rock != post.rock)
                .min_by(|(_, a), (_, b)| {
                    let (first, second) = (
                        self.state.rock_body(a.rock).pos.distance(here),
                        self.state.rock_body(b.rock).pos.distance(here),
                    );
                    first
                        .total_cmp(&second)
                        .then(a.rock.cmp(&b.rock))
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

    fn reconcile(&self, assigned: &mut Assigned, post: Post, row: RowId, wanted: u32) {
        let open = self.state.frames_of(post, row).count();
        match wanted {
            0 => self.cancel(assigned, post, row, open as u32),
            _ if open == 0 => assigned.openings.push(Opening { post, row }),
            _ => {}
        }
    }

    fn cancel(&self, assigned: &mut Assigned, post: Post, row: RowId, count: u32) {
        let mut open: Vec<(usize, f64)> = self
            .state
            .frames_of(post, row)
            .map(|(at, frame)| (at, frame.progress()))
            .collect();
        open.sort_by(|(at, first), (next, second)| first.total_cmp(second).then(at.cmp(next)));
        for (frame, progress) in open.into_iter().take(count as usize) {
            assigned.cancellations.push(Cancellation {
                frame,
                row,
                seat: post.seat,
                progress,
            });
        }
    }

    fn leaving(&self, sends: &[Send]) -> BTreeMap<(Post, RowId), u32> {
        let mut gone: BTreeMap<(Post, RowId), u32> = BTreeMap::new();
        for send in sends {
            for entity in send.members.iter().filter_map(|id| self.state.entity(*id)) {
                let post = Post {
                    rock: send.source,
                    seat: entity.seat(),
                };
                *gone.entry((post, entity.row())).or_default() += 1;
            }
        }
        gone
    }

    fn solve(
        &self,
        assigned: &mut Assigned,
        moving: BTreeMap<(RockId, RockId, SeatId), Vec<EntityId>>,
    ) -> BTreeMap<(Post, RowId), u32> {
        let mut held_back: BTreeMap<(Post, RowId), u32> = BTreeMap::new();
        for ((source, destination, seat), mut members) in moving {
            members.sort_unstable();
            match Send::joining(self.state, source, destination, seat, &members) {
                Some(send) => assigned.sends.push(send),
                None => {
                    for entity in members.iter().filter_map(|id| self.state.entity(*id)) {
                        *held_back
                            .entry((
                                Post {
                                    rock: destination,
                                    seat,
                                },
                                entity.row(),
                            ))
                            .or_default() += 1;
                    }
                }
            }
        }
        held_back
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
        let want = state
            .wants(post)
            .map_or(0, |wants| wants.get(row))
            .saturating_sub(state.frames_of(post, row).count() as u32);
        let mut held: Vec<EntityId> = state
            .entities_at(post.rock)
            .filter(|entity| entity.seat() == post.seat && entity.row() == row)
            .filter(|entity| entity.flight().is_none())
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
                    rock: entity.home(),
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

fn unwanted_frames(
    state: &State,
    leaving: &BTreeMap<(Post, RowId), u32>,
) -> Vec<(Post, RowId, u32)> {
    let mut open: BTreeMap<(Post, RowId), u32> = BTreeMap::new();
    for frame in state.frames() {
        *open.entry((frame.post(), frame.row())).or_default() += 1;
    }
    open.into_iter()
        .filter(|((post, row), _)| {
            let want = state.wants(*post).map_or(0, |wants| wants.get(*row));
            let gone = leaving.get(&(*post, *row)).copied().unwrap_or_default();
            want + gone <= state.count(*post, *row)
        })
        .map(|((post, row), count)| (post, row, count))
        .collect()
}
