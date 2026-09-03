//! One socket in the browser: the page's own WebSocket, and the queue its
//! callbacks fill.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{BinaryType, Event, MessageEvent, WebSocket};

/// One socket to a room. Opening does not wait: bytes said before the
/// socket opens are sent when it does, and a socket that never opens reads
/// as closed.
pub(crate) struct Link {
    socket: Option<WebSocket>,
    /// Messages the socket's callback has taken, oldest first.
    hears: Rc<RefCell<VecDeque<Vec<u8>>>>,
    /// Whether the socket has closed. A failed socket closes too, which the
    /// page guarantees, so its failure needs no callback of its own.
    shut: Rc<Cell<bool>>,
    /// Bytes said before the socket opened.
    waiting: Vec<Vec<u8>>,
    /// The callbacks the socket holds, which live as long as it does.
    messaged: Option<Closure<dyn FnMut(MessageEvent)>>,
    shutting: Option<Closure<dyn FnMut(Event)>>,
}

impl Link {
    /// Whether the socket has closed, which a room that was never reached
    /// also reads as.
    pub(crate) fn closed(&self) -> bool {
        self.socket.is_none() || self.shut.get()
    }

    /// Every message off the socket since the last call, in arrival order.
    pub(crate) fn heard(&mut self) -> Vec<Vec<u8>> {
        self.flush();
        self.hears.borrow_mut().drain(..).collect()
    }

    /// Opens a socket to the room at `address`, as `host:port`.
    pub(crate) fn opening(address: &str) -> Link {
        let hears: Rc<RefCell<VecDeque<Vec<u8>>>> = Rc::new(RefCell::new(VecDeque::new()));
        let shut = Rc::new(Cell::new(false));
        let Ok(socket) = WebSocket::new(&super::url(address)) else {
            shut.set(true);
            return Link {
                socket: None,
                hears,
                shut,
                waiting: Vec::new(),
                messaged: None,
                shutting: None,
            };
        };
        socket.set_binary_type(BinaryType::Arraybuffer);

        let taken = Rc::clone(&hears);
        let messaged = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() {
                taken
                    .borrow_mut()
                    .push_back(js_sys::Uint8Array::new(&buffer).to_vec());
            }
        });
        socket.set_onmessage(Some(messaged.as_ref().unchecked_ref()));

        let ended = Rc::clone(&shut);
        let shutting = Closure::<dyn FnMut(Event)>::new(move |_: Event| ended.set(true));
        socket.set_onclose(Some(shutting.as_ref().unchecked_ref()));

        Link {
            socket: Some(socket),
            hears,
            shut,
            waiting: Vec::new(),
            messaged: Some(messaged),
            shutting: Some(shutting),
        }
    }

    /// Passes `bytes` to the room, once the socket is open.
    pub(crate) fn say(&mut self, bytes: Vec<u8>) {
        self.waiting.push(bytes);
        self.flush();
    }

    /// Sends everything said so far, where the socket is open.
    fn flush(&mut self) {
        let Some(socket) = self.socket.as_ref().filter(|_| !self.shut.get()) else {
            return;
        };
        if socket.ready_state() != WebSocket::OPEN {
            return;
        }
        for bytes in self.waiting.drain(..) {
            if socket.send_with_u8_array(&bytes).is_err() {
                self.shut.set(true);
                return;
            }
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let Some(socket) = self.socket.take() else {
            return;
        };
        socket.set_onmessage(None);
        socket.set_onclose(None);
        drop(self.messaged.take());
        drop(self.shutting.take());
        // A socket already closed is what this asks for anyway.
        let _ = socket.close();
    }
}
