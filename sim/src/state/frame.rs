use crate::TICKS_PER_SECOND;
use crate::ids::RowId;
use crate::materials::Material;
use crate::place::Post;
use crate::real::Real;
use crate::time::Tick;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    post: Post,
    row: RowId,
    progress: Real,
    fed: Tick,
    short: Option<Material>,
}

impl Frame {
    pub fn new(post: Post, row: RowId, progress: f64, at: Tick) -> Frame {
        Frame {
            post,
            row,
            progress: Real(progress),
            fed: at,
            short: None,
        }
    }

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

    pub fn progress(&self) -> f64 {
        self.progress.0
    }

    pub fn fraction(&self, cost: f64) -> f64 {
        match cost > 0.0 {
            true => (self.progress.0 / cost).clamp(0.0, 1.0),
            false => 1.0,
        }
    }

    pub(crate) fn build(&mut self, units: f64, at: Tick) {
        self.progress.0 += units;
        self.fed = at;
        self.short = None;
    }

    pub(crate) fn short_of(&mut self, material: Option<Material>) {
        self.short = material;
    }
}
