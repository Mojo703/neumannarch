//! Work in progress toward one entity.

use crate::TICKS_PER_SECOND;
use crate::ids::RowId;
use crate::materials::Material;
use crate::place::Post;
use crate::real::Real;
use crate::time::Tick;

/// One shortfall being built at a post: the work done so far, in cost
/// units, toward one entity of `row`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    post: Post,
    row: RowId,
    progress: Real,
    /// The last tick materials went into it.
    fed: Tick,
    /// The material the last spend it took nothing on wanted.
    short: Option<Material>,
}

impl Frame {
    /// A frame at `post` for `row` with `progress` cost units done, opened
    /// at `at`.
    pub fn new(post: Post, row: RowId, progress: f64, at: Tick) -> Frame {
        Frame {
            post,
            row,
            progress: Real(progress),
            fed: at,
            short: None,
        }
    }

    /// The material this frame has spent nothing on for a second for want
    /// of, at `now`.
    pub fn starved_material(&self, now: Tick) -> Option<Material> {
        self.short
            .filter(|_| now.0.saturating_sub(self.fed.0) >= TICKS_PER_SECOND as u64)
    }

    pub fn post(&self) -> Post {
        self.post
    }

    pub fn row(&self) -> RowId {
        self.row
    }

    /// Work done so far, in cost units.
    pub fn progress(&self) -> f64 {
        self.progress.0
    }

    /// Work done as a fraction of `cost`, the row's cost in cost units; a
    /// frame of a row costing nothing is done.
    pub fn fraction(&self, cost: f64) -> f64 {
        match cost > 0.0 {
            true => (self.progress.0 / cost).clamp(0.0, 1.0),
            false => 1.0,
        }
    }

    /// Adds `units` cost units of work, done at `at`.
    pub(crate) fn build(&mut self, units: f64, at: Tick) {
        self.progress.0 += units;
        self.fed = at;
        self.short = None;
    }

    /// Records that a spend took nothing for want of `material`.
    pub(crate) fn short_of(&mut self, material: Option<Material>) {
        self.short = material;
    }
}
