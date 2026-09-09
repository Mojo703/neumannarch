use crate::bots::scripted::ranking::*;

#[test]
fn the_greatest_score_wins_and_a_tie_falls_to_the_lowest_id() {
    let ranking = Ranking::by([3u8, 1, 2, 4], |id| match id {
        4 => None,
        3 | 1 => Some(9.0),
        _ => Some(2.0),
    });

    assert_eq!(ranking.best(), Some(1));
    assert_eq!(ranking.order(), vec![1, 3, 2]);
}
