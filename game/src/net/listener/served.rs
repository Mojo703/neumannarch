use std::net::SocketAddr;

use crate::net::listener::NoListener;

pub struct Listener(neumannarch_server::Hosted);

impl Listener {
    pub fn address(&self) -> String {
        format!("127.0.0.1:{}", self.0.address().port())
    }

    pub fn opened() -> Result<Listener, NoListener> {
        Listener::serving(SocketAddr::from((
            [0, 0, 0, 0],
            neumannarch_protocol::DEFAULT_PORT,
        )))
    }

    pub fn serving(address: SocketAddr) -> Result<Listener, NoListener> {
        neumannarch_server::Hosted::serving(address)
            .map(Listener)
            .map_err(|_| NoListener::PortHeld)
    }
}
