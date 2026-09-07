use crate::ids::{AsteroidId, RowId, SeatId};
use crate::post::Post;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Posting {
    post: Post,
    row: RowId,
}

impl Posting {
    pub fn new(post: Post, row: RowId) -> Posting {
        Posting { post, row }
    }

    pub fn of(asteroid: AsteroidId, seat: SeatId, row: RowId) -> Posting {
        Posting::new(Post { asteroid, seat }, row)
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

    pub fn row(self) -> RowId {
        self.row
    }

    pub fn seated(self, seat: SeatId) -> Posting {
        Posting::of(self.asteroid(), seat, self.row)
    }
}
