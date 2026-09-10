use crate::ids::{AsteroidId, SeatId};
use crate::pattern::EntityPattern;
use crate::post::Post;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Posting {
    post: Post,
    pattern: EntityPattern,
}

impl Posting {
    pub fn new(post: Post, pattern: EntityPattern) -> Posting {
        Posting { post, pattern }
    }

    pub fn of(asteroid: AsteroidId, seat: SeatId, pattern: EntityPattern) -> Posting {
        Posting::new(Post { asteroid, seat }, pattern)
    }

    pub fn post(self) -> Post {
        self.post
    }

    pub fn asteroid(self) -> AsteroidId {
        self.post.asteroid
    }

    pub fn seat(self) -> SeatId {
        self.post.seat
    }

    pub fn pattern(self) -> EntityPattern {
        self.pattern
    }

    pub fn seated(self, seat: SeatId) -> Posting {
        Posting::of(self.asteroid(), seat, self.pattern)
    }
}
