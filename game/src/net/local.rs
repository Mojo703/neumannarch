//! The transport of a match with no peers.

use probe_protocol::Message;
use probe_sim::{SeatId, Stamped, Tick};

use crate::net::transport::Transport;

/// A match played on this machine alone. It carries nothing anywhere and
/// nothing arrives; it exists so the loop has one shape whether or not the
/// match has peers.
#[derive(Clone, Copy, Debug, Default)]
pub struct Local;

impl Transport for Local {
    fn send(&mut self, _stamped: Stamped) {}

    fn acknowledge(&mut self, _seat: SeatId, _up_to: Tick) {}

    fn leave(&mut self) {}

    fn report(&mut self, _tick: Tick, _hash: u64) {}

    fn received(&mut self) -> Vec<Message> {
        Vec::new()
    }
}
