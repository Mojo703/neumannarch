use std::net::SocketAddr;

use probe_protocol::DEFAULT_PORT;
use probe_server::Hosted;

fn main() {
    let asked = std::env::args().nth(1);
    let address = match asked {
        Some(address) => match address.parse::<SocketAddr>() {
            Ok(address) => address,
            Err(why) => {
                eprintln!("{address} is not an address to serve on: {why}");
                return;
            }
        },
        None => SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)),
    };
    match Hosted::serving(address) {
        Ok(hosted) => {
            println!("a room at {}", hosted.address());
            hosted.wait();
        }
        Err(why) => eprintln!("no room at {address}: {why}"),
    }
}
