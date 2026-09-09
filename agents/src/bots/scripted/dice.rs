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
