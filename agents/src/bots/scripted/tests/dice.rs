use crate::bots::scripted::dice::*;

#[test]
fn the_same_seed_gives_the_same_sequence() {
    let run = |seed| {
        let mut dice = Dice::new(seed);
        (0..8).map(|_| dice.roll()).collect::<Vec<u64>>()
    };
    assert_eq!(run(7), run(7));
    assert_ne!(run(7), run(8));
}

#[test]
fn a_roll_below_a_bound_stays_under_it() {
    let mut dice = Dice::new(3);
    for _ in 0..64 {
        assert!(dice.below(5).expect("a positive bound") < 5);
    }
    assert_eq!(dice.below(0), None);
}
