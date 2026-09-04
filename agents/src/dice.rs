const GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dice(u64);

impl Dice {
    pub fn new(seed: u64) -> Dice {
        Dice(seed)
    }

    pub fn roll(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(GAMMA);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, bound: usize) -> Option<usize> {
        let bound = u64::try_from(bound).ok().filter(|bound| *bound > 0)?;
        usize::try_from(self.roll() % bound).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
