use crate::TICKS_PER_SECOND;
use crate::materials::Material;
use crate::pattern::EntityPattern;
use crate::post::Post;
use crate::real::Real;
use crate::time::Time;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    post: Post,
    pattern: EntityPattern,
    progress: Real,
    fed: Time,
    short: Option<Material>,
}

impl Frame {
    pub fn new(post: Post, pattern: EntityPattern, progress: f64, at: Time) -> Frame {
        Frame {
            post,
            pattern,
            progress: Real(progress),
            fed: at,
            short: None,
        }
    }

    pub fn starved_material(&self, now: Time) -> Option<Material> {
        self.short
            .filter(|_| now.0.saturating_sub(self.fed.0) >= TICKS_PER_SECOND as u64)
    }

    pub fn post(&self) -> Post {
        self.post
    }

    pub(crate) fn pattern(&self) -> EntityPattern {
        self.pattern
    }

    pub(crate) fn progress(&self) -> f64 {
        self.progress.0
    }

    pub(crate) fn fraction(&self) -> f64 {
        let cost = self.pattern.cost().total();
        match cost > 0.0 {
            true => (self.progress.0 / cost).clamp(0.0, 1.0),
            false => 1.0,
        }
    }

    pub fn build(&mut self, units: f64, at: Time) {
        self.progress.0 += units;
        self.fed = at;
        self.short = None;
    }

    pub fn short_of(&mut self, material: Option<Material>) {
        self.short = material;
    }
}
