use neumannarch_protocol::Relayed;
use neumannarch_sim::{SeatId, Stamped, Tick};

pub trait Transport {
    fn send(&mut self, stamped: Stamped);

    fn acknowledge(&mut self, seat: SeatId, up_to: Tick);

    fn report(&mut self, tick: Tick, hash: u64);

    fn received(&mut self) -> Vec<Relayed>;
}
