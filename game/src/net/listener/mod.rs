#[cfg(not(all(feature = "host", not(target_arch = "wasm32"))))]
pub use nowhere::Listener;
#[cfg(all(feature = "host", not(target_arch = "wasm32")))]
pub use served::Listener;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoListener {
    NotBuilt,
    PortHeld,
}

impl NoListener {
    pub fn reason(self) -> &'static str {
        match self {
            NoListener::NotBuilt => "Cannot host here",
            NoListener::PortHeld => "Port in use",
        }
    }
}

#[cfg(not(all(feature = "host", not(target_arch = "wasm32"))))]
mod nowhere;
#[cfg(all(feature = "host", not(target_arch = "wasm32")))]
mod served;
