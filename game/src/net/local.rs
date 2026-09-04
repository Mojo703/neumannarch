use neumannarch_protocol::Relayed;
use neumannarch_sim::{SeatId, Stamped, Tick};

use crate::net::transport::Transport;

#[derive(Clone, Copy, Debug, Default)]
pub struct Local;

impl Transport for Local {
    fn send(&mut self, _stamped: Stamped) {}

    fn acknowledge(&mut self, _seat: SeatId, _up_to: Tick) {}

    fn report(&mut self, _tick: Tick, _hash: u64) {}

    fn received(&mut self) -> Vec<Relayed> {
        Vec::new()
    }
}
