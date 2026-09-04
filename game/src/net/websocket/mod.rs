#[cfg(target_arch = "wasm32")]
pub(crate) use browser::WebSocket;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::WebSocket;

pub(crate) fn url(address: &str) -> String {
    format!("ws://{address}/")
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod native;
