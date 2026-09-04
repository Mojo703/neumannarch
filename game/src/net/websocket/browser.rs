use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{BinaryType, Event, MessageEvent};

pub(crate) struct WebSocket {
    socket: Option<web_sys::WebSocket>,
    received: Rc<RefCell<VecDeque<Vec<u8>>>>,
    shut: Rc<Cell<bool>>,
    waiting: Vec<Vec<u8>>,
    messaged: Option<Closure<dyn FnMut(MessageEvent)>>,
    shutting: Option<Closure<dyn FnMut(Event)>>,
}

impl WebSocket {
    pub(crate) fn closed(&self) -> bool {
        self.socket.is_none() || self.shut.get()
    }

    pub(crate) fn received(&mut self) -> Vec<Vec<u8>> {
        self.flush();
        self.received.borrow_mut().drain(..).collect()
    }

    pub(crate) fn opening(address: &str) -> WebSocket {
        let received: Rc<RefCell<VecDeque<Vec<u8>>>> = Rc::new(RefCell::new(VecDeque::new()));
        let shut = Rc::new(Cell::new(false));
        let Ok(socket) = web_sys::WebSocket::new(&super::url(address)) else {
            shut.set(true);
            return WebSocket {
                socket: None,
                received,
                shut,
                waiting: Vec::new(),
                messaged: None,
                shutting: None,
            };
        };
        socket.set_binary_type(BinaryType::Arraybuffer);

        let taken = Rc::clone(&received);
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

        WebSocket {
            socket: Some(socket),
            received,
            shut,
            waiting: Vec::new(),
            messaged: Some(messaged),
            shutting: Some(shutting),
        }
    }

    pub(crate) fn send(&mut self, bytes: Vec<u8>) {
        self.waiting.push(bytes);
        self.flush();
    }

    fn flush(&mut self) {
        let Some(socket) = self.socket.as_ref().filter(|_| !self.shut.get()) else {
            return;
        };
        if socket.ready_state() != web_sys::WebSocket::OPEN {
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

impl Drop for WebSocket {
    fn drop(&mut self) {
        let Some(socket) = self.socket.take() else {
            return;
        };
        socket.set_onmessage(None);
        socket.set_onclose(None);
        drop(self.messaged.take());
        drop(self.shutting.take());

        let _ = socket.close();
    }
}
