//! The bytes of one socket, moved by whatever the target has: a runtime
//! and a client on the desktop, the browser's own WebSocket in the page.

#[cfg(target_arch = "wasm32")]
pub(crate) use browser::Link;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::Link;

/// The room at `address`, as `host:port`. Rooms are plain WebSockets: a
/// match carries no secret, and a room behind TLS is served through a
/// proxy rather than by the game.
pub(crate) fn url(address: &str) -> String {
    format!("ws://{address}/")
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
