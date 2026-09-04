use probe_protocol::{Codec, Message, Notice, Relayed, Request};
use probe_sim::{SeatId, Stamped, Tick};

use crate::net::transport::Transport;
use crate::net::websocket::WebSocket;

pub struct Connection {
    link: WebSocket,
    address: String,
    notices: Vec<Notice>,
    relayed: Vec<Relayed>,
}

impl Connection {
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn closed(&self) -> bool {
        self.link.closed()
    }

    pub fn notices(&mut self) -> Vec<Notice> {
        self.decode_pending();
        core::mem::take(&mut self.notices)
    }

    pub fn joining(address: &str) -> Connection {
        let mut socket = Connection {
            link: WebSocket::opening(address),
            address: address.to_string(),
            notices: Vec::new(),
            relayed: Vec::new(),
        };
        socket.request(Request::Join {
            version: probe_protocol::VERSION,
        });
        socket
    }

    pub fn leaving(&mut self) {
        self.request(Request::Leave);
    }

    pub fn request(&mut self, request: Request) {
        self.send_message(Message::Request(request));
    }

    fn decode_pending(&mut self) {
        for bytes in self.link.received() {
            match Message::decode(&bytes) {
                Ok(Message::Notice(notice)) => self.notices.push(notice),
                Ok(Message::Relayed(relayed)) => self.relayed.push(relayed),
                Ok(Message::Request(_)) | Err(_) => {}
            }
        }
    }

    fn send_message(&mut self, message: Message) {
        self.link.send(message.encoded());
    }
}

impl Transport for Connection {
    fn acknowledge(&mut self, seat: SeatId, up_to: Tick) {
        self.send_message(Message::Relayed(Relayed::Acknowledge { seat, up_to }));
    }

    fn received(&mut self) -> Vec<Relayed> {
        self.decode_pending();
        core::mem::take(&mut self.relayed)
    }

    fn report(&mut self, tick: Tick, hash: u64) {
        self.send_message(Message::Relayed(Relayed::Hash { tick, hash }));
    }

    fn send(&mut self, stamped: Stamped) {
        self.send_message(Message::Relayed(Relayed::Command(stamped)));
    }
}
