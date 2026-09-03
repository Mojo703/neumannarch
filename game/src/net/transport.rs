//! What carries a match between machines.

use probe_protocol::Message;
use probe_sim::{SeatId, Stamped, Tick};

/// What a machine tells its peers about the match, and what they tell it.
///
/// A machine hands everything it issues to the transport at once and waits
/// for nothing; what arrives is inserted into its own session, which
/// rewinds as needed.
pub trait Transport {
    /// Passes on a command this machine issued.
    fn send(&mut self, stamped: Stamped);

    /// Passes on that no command of `seat` before `up_to` is unknown here.
    fn acknowledge(&mut self, seat: SeatId, up_to: Tick);

    /// Passes on that this machine is leaving the match.
    fn leave(&mut self);

    /// Passes on this machine's state hash at a settled tick.
    fn report(&mut self, tick: Tick, hash: u64);

    /// What has arrived since the last call, in arrival order.
    fn received(&mut self) -> Vec<Message>;
}
