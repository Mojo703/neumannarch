use crate::net::listener::NoListener;

pub struct Listener(Nowhere);

enum Nowhere {}

impl Listener {
    pub fn address(&self) -> String {
        match self.0 {}
    }

    pub fn opened() -> Result<Listener, NoListener> {
        Err(NoListener::NotBuilt)
    }
}
