//! Work in progress toward one entity.

use crate::ids::RowId;
use crate::place::Post;
use crate::real::Real;

/// One shortfall being built at a post: the work done so far, in cost
/// units, toward one entity of `row`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    post: Post,
    row: RowId,
    progress: Real,
}

impl Frame {
    /// A frame at `post` for `row` with `progress` cost units done.
    pub fn new(post: Post, row: RowId, progress: f64) -> Frame {
        Frame {
            post,
            row,
            progress: Real(progress),
        }
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

    /// Adds `units` cost units of work.
    pub(crate) fn build(&mut self, units: f64) {
        self.progress.0 += units;
    }
}
