//! Who speaks for each seat of a match, what this machine says to its
//! peers about it, and the socket it says it over.

pub mod controller;
pub mod hosting;
pub mod local;
pub mod machine;
pub mod pace;
pub mod room;
pub mod socket;
pub mod transport;

mod link;
