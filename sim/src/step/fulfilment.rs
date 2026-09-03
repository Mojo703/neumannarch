//! The fulfilment rule: every post's wants met from the reserve, then from
//! a surplus elsewhere, then by frames opened for builders.

use std::collections::BTreeMap;

use crate::ids::{EntityId, RowId, SeatId};
use crate::place::{Place, Post};
use crate::roster::Kind;
use crate::state::{Entity, Flight, State};

/// One entity taken from the reserve and placed complete at a post.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub post: Post,
    pub row: RowId,
}

/// One send: the units re-homed to `flight`'s destination this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Send {
    pub flight: Flight,
    /// The units joining it, in id order.
    pub members: Vec<EntityId>,
}

/// A send that had no plan: the units stay home this tick and the phase
/// says so rather than dropping them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unplanned {
    pub from: Place,
    pub to: Place,
    pub seat: SeatId,
}

/// Frames to open at a post for one row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opening {
    pub post: Post,
    pub row: RowId,
    pub count: u32,
}

/// One frame to close, by its place in the order opened, with the cost
/// units it consumed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cancellation {
    pub frame: usize,
    pub row: RowId,
    pub seat: SeatId,
    /// Cost units already spent on it, which refund.
    pub progress: f64,
}

/// What fulfilment decided this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Assigned {
    pub placements: Vec<Placement>,
    pub sends: Vec<Send>,
    pub unplanned: Vec<Unplanned>,
    pub openings: Vec<Opening>,
    pub cancellations: Vec<Cancellation>,
    /// Every entity that is surplus with nowhere to go, in id order; the
    /// full set, so a mark lifts when a want comes back.
    pub marked: Vec<EntityId>,
}

/// The fulfilment phase over one tick's snapshot.
pub struct Fulfilment<'a> {
    state: &'a State,
    /// Per seat and row, the units that can leave their post, in id order.
    spare: BTreeMap<(SeatId, RowId), Vec<Spare>>,
    /// Surplus structures, which never move, in id order.
    stuck: Vec<EntityId>,
}

/// One unit that its post does not want, and where it is.
#[derive(Clone, Copy, Debug)]
struct Spare {
    entity: EntityId,
    place: Place,
}

impl<'a> Fulfilment<'a> {
    /// Reads `state` and finds every unit its post no longer wants.
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

    /// The tick's reserve placements, sends, frame openings and
    /// cancellations, and the surplus left with nowhere to go.
    pub fn run(mut self) -> Assigned {
        let mut assigned = Assigned::default();
        let mut reserved: BTreeMap<(SeatId, RowId), u32> = BTreeMap::new();
        let mut moving: BTreeMap<(Place, Place, SeatId), Vec<EntityId>> = BTreeMap::new();
        for (post, row, shortfall) in shortfalls(self.state) {
            let from_reserve = self.take_reserved(post, row, shortfall, &mut reserved);
            let mut filled = from_reserve;
            for entity in self.nearest_spare(post, row, shortfall - filled) {
                let place = entity.place;
                moving
                    .entry((place, post.place, post.seat))
                    .or_default()
                    .push(entity.entity);
                filled += 1;
            }
            for _ in 0..from_reserve {
                assigned.placements.push(Placement { post, row });
            }
            self.reconcile(&mut assigned, post, row, shortfall - filled);
        }
        self.plan(&mut assigned, moving);
        self.mark(&mut assigned);
        for (post, row, over) in unwanted_frames(self.state) {
            self.cancel(&mut assigned, post, row, over);
        }
        assigned
    }

    /// How many of `row` the seat's reserve gives this post, counting what
    /// earlier posts of the same seat already took.
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

    /// Up to `wanted` spare units of `row`, taken from the nearest post
    /// that has any, by the distance between rock bodies now, ties by the
    /// lower rock id. The highest-id units of a post go first.
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

    /// Brings the frames of `row` at `post` to `wanted`, opening or
    /// cancelling the difference. Cancelling takes the least-progressed
    /// first, which refunds least.
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

    /// Cancels `count` frames of `row` at `post`, least-progressed first.
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

    /// Where the frames of `row` at `post` sit in the order opened.
    fn frames_of(&self, post: Post, row: RowId) -> Vec<usize> {
        self.state
            .frames()
            .iter()
            .enumerate()
            .filter(|(_, frame)| frame.post() == post && frame.row() == row)
            .map(|(at, _)| at)
            .collect()
    }

    /// One flight per source and destination pair, or an `Unplanned` when
    /// no transfer fits, which leaves those units home this tick.
    fn plan(
        &self,
        assigned: &mut Assigned,
        moving: BTreeMap<(Place, Place, SeatId), Vec<EntityId>>,
    ) {
        for ((from, to, seat), mut members) in moving {
            members.sort_unstable();
            let rows: Vec<RowId> = members
                .iter()
                .filter_map(|id| self.state.entity(*id))
                .map(Entity::row)
                .collect();
            match Flight::plan(self.state, from, to, rows.into_iter()) {
                Some(flight) => assigned.sends.push(Send { flight, members }),
                None => assigned.unplanned.push(Unplanned { from, to, seat }),
            }
        }
    }

    /// Every unit and structure left with nowhere to go, which a builder
    /// scraps where it stands.
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

/// Every post and row wanting more than it has, in key order, with the
/// count missing. Units present and in flight to the place count; frames
/// never do.
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

/// Every post and row holding more than it wants, in key order, with the
/// entities that are over: the highest ids first, and never one in flight,
/// which is already committed.
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

/// Every post and row with an entity, in key order.
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

/// Every post and row with more frames than the post's wants can use, with
/// how many are over. A post whose want is gone keeps no frames.
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
